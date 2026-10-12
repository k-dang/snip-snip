//! snipctl: drives a real snip-snip instance with OS input and captures proof.
//! See ../SKILL.md. Positions given to `snip` are screen units (physical
//! pixels on Windows, points on macOS); `drag` and `click` take image pixels.

mod fixture;
mod input;
mod platform;
mod run;
mod screen;
#[cfg(windows)]
mod verify_shell;

use std::{fs, path::Path, process::Command, time::Duration};

use anyhow::{Context as _, Result, bail};

use input::Input;
use run::{Run, wait_for};
use screen::{Layout, OVERLAY_TITLE};

const USAGE: &str = "\
snipctl <command> [args]

  launch                start target/release/snip-snip in a new run dir; waits for ready
  verify-shell [CHORD]  Windows shell scenario, evidence + cleanup; default ctrl+alt+k
  doctor                read-only health check of the current run
  target                show the fixture (blue|green halves) at screen 100,100 size 800x500
  snip X Y W H [CHORD]  capture with CHORD (default Alt+Shift+S), drag, wait for editor
  shell-menu ITEM       Windows tray menu: new-snip, settings, quit
  shell-key COMBO...    focus Settings if open, then send real keyboard input
  cancel-snip           Alt+Shift+S, then Esc on the overlay; asserts no editor opens
  layout                print where the editor draws the image
  key COMBO...          focus the editor and press chords: r  mod+z  mod+shift+z  left  home  esc
                        macOS code:N presses virtual-key code N, e.g. alt+shift+code:1
  type TEXT...          focus the editor and type; letters, digits, spaces as keys, others as Unicode
  drag X1 Y1 X2 Y2 [KEY] focus the editor and drag in Fit image pixels, optionally holding KEY
  drag-screen X1 Y1 X2 Y2 [KEY] drag in absolute screen units (also works after zoom/pan)
  drag-preview X1 Y1 X2 Y2 NAME drag in Fit image pixels, screenshot before releasing the mouse
  click X Y             focus the editor and click a point in image pixels
  double-click X Y      focus the editor and double-click in Fit image pixels
  click-screen X Y      focus the editor and click in absolute screen units
  click-desktop X Y     click an absolute desktop point without focusing the editor
  hover-screen X Y      focus the editor and move the cursor without clicking
  cursor                Windows: read the native cursor shape
  resize-editor W H     Windows: resize the owned editor's client area
  probe-shortcut CHORD  Windows: check global availability, releasing a successful registration
  paint-launch EXE      Windows: start an owned Paint process for clipboard paste checks
  paint-key COMBO...    Windows: send keys to owned Paint's current foreground window
  paint-type TEXT       Windows: type text into owned Paint's current foreground window
  paint-click X Y       Windows: click a screen point in owned Paint
  paint-shot NAME       Windows: capture owned Paint and list its windows
  scroll-screen X Y NOTCHES [KEY] scroll vertically at a screen point, optionally holding KEY
  shot NAME             save the editor window (or the primary screen) to <run>/NAME.png
  clipboard NAME        save the clipboard image to <run>/NAME.png
  save NAME             mod+s, enter <run>/NAME.png in the save dialog, wait for the file
  cancel-save           open the native save dialog, cancel it, assert the editor stays open
  inspect NAME|PATH     print a PNG's size and fixture/ink color counts
  close                 Esc the editor (finishing text first if editing)
  cleanup               stop the app and fixture this run started; evidence stays

`mod` is Ctrl on Windows and Cmd on macOS.";

fn main() {
    platform::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(err) = dispatch(&args) {
        eprintln!("snipctl: {err:#}");
        std::process::exit(1);
    }
}

fn dispatch(args: &[String]) -> Result<()> {
    let (command, rest) = args
        .split_first()
        .map(|(c, r)| (c.as_str(), r))
        .unwrap_or(("help", &[]));
    match command {
        "verify-shell" => {
            #[cfg(windows)]
            {
                if rest.len() > 1 {
                    bail!("expected at most one test chord");
                }
                verify_shell::verify(rest.first().map(String::as_str).unwrap_or("ctrl+alt+k"))
            }
            #[cfg(not(windows))]
            bail!("verify-shell currently requires Windows tray automation")
        }
        "launch" => launch(),
        "doctor" => doctor(),
        "target" => target(),
        "fixture-window" => fixture::run(),
        "snip" => {
            if !(4..=5).contains(&rest.len()) {
                bail!("expected X Y W H [CHORD]");
            }
            snip(
                numbers(&rest[..4], 4)?,
                rest.get(4).map(String::as_str).unwrap_or("alt+shift+s"),
            )
        }
        #[cfg(windows)]
        "shell-menu" => shell_menu(name(rest)?),
        "shell-key" => shell_key(rest),
        "cancel-snip" => cancel_snip(),
        "layout" => layout(),
        "key" => key(rest),
        "type" => type_text(&rest.join(" ")),
        "drag" | "drag-screen" => {
            if !(4..=5).contains(&rest.len()) {
                bail!("expected X1 Y1 X2 Y2 [KEY]");
            }
            drag(
                numbers(&rest[..4], 4)?,
                rest.get(4).map(String::as_str),
                command == "drag-screen",
            )
        }
        "click" => click(numbers(rest, 2)?),
        "drag-preview" => {
            if rest.len() != 5 {
                bail!("expected X1 Y1 X2 Y2 NAME");
            }
            drag_preview(numbers(&rest[..4], 4)?, &rest[4])
        }
        "double-click" => double_click(numbers(rest, 2)?),
        "click-screen" | "hover-screen" => screen_pointer(numbers(rest, 2)?, command == "click-screen"),
        "click-desktop" => click_desktop(numbers(rest, 2)?),
        #[cfg(windows)]
        "cursor" => run::current()?.step(&format!("native cursor: {}", platform::cursor_name()?)),
        #[cfg(windows)]
        "probe-shortcut" => {
            let chord = name(rest)?;
            let hotkey = chord.parse::<global_hotkey::hotkey::HotKey>()?;
            let manager = global_hotkey::GlobalHotKeyManager::new()?;
            let result = match manager.register(hotkey) {
                Ok(()) => {
                    manager.unregister(hotkey)?;
                    "available (probe released)".to_owned()
                }
                Err(err) => format!("unavailable: {err:?}"),
            };
            run::current()?.step(&format!("global shortcut {chord}: {result}"))
        }
        #[cfg(windows)]
        "resize-editor" => {
            let n = numbers(rest, 2)?;
            if n.iter().any(|v| !v.is_finite() || *v < 1.) {
                bail!("positive size required");
            }
            let run = run::current()?;
            let window = focus_editor(&run, &mut Input::new()?)?;
            platform::resize_editor(&window, n[0] as i32, n[1] as i32)?;
            std::thread::sleep(Duration::from_millis(300));
            run.step(&format!("resize editor client to {}x{}", n[0], n[1]))
        }
        #[cfg(windows)]
        "paint-launch" => paint_launch(name(rest)?),
        #[cfg(windows)]
        "paint-key" | "paint-type" => paint_input(rest, command == "paint-type"),
        #[cfg(windows)]
        "paint-click" => {
            let n = numbers(rest, 2)?;
            let run = run::current()?;
            let (pid, window) = paint_window(&run)?;
            let mut input = Input::new()?;
            platform::focus(&window, pid, &mut input)?;
            input.click((n[0].round() as i32, n[1].round() as i32))?;
            run.step(&format!("Paint click screen {},{}", n[0], n[1]))
        }
        #[cfg(windows)]
        "paint-shot" => paint_shot(name(rest)?),
        "scroll-screen" => {
            if !(3..=4).contains(&rest.len()) {
                bail!("expected X Y NOTCHES [KEY]");
            }
            scroll_screen(
                numbers(&rest[..2], 2)?,
                rest[2].parse()?,
                rest.get(3).map(String::as_str),
            )
        }
        "shot" => shot(name(rest)?),
        "clipboard" => clipboard(name(rest)?),
        "save" => save(name(rest)?),
        "cancel-save" => cancel_save(),
        "inspect" => inspect(name(rest)?),
        "close" => close(),
        "cleanup" => cleanup(),
        _ => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

fn numbers(args: &[String], n: usize) -> Result<Vec<f32>> {
    if args.len() != n {
        bail!("expected {n} numbers, got {args:?}");
    }
    args.iter()
        .map(|a| a.parse::<f32>().with_context(|| format!("not a number: {a}")))
        .collect()
}

fn name(args: &[String]) -> Result<&str> {
    args.first().map(String::as_str).context("missing NAME")
}

/// Our running app's pid; every command but `launch` acts on it.
fn app(run: &Run) -> Result<u32> {
    run.owned("app")
        .context("our snip-snip is not running (see doctor)")
}

fn launch() -> Result<()> {
    let exe = check_launch()?;
    let run = run::start()?;
    launch_app(&run, &exe)
}

#[cfg(windows)]
fn launch_in(run: &Run) -> Result<()> {
    launch_app(run, &check_launch()?)
}

fn check_launch() -> Result<std::path::PathBuf> {
    let exe = run::app_exe();
    if !exe.exists() {
        bail!(
            "missing {}; run: cargo build --release -p snip-gpui",
            exe.display()
        );
    }
    let others = run::snip_snip_pids();
    if !others.is_empty() {
        bail!(
            "snip-snip already running (pid {others:?}); it owns the capture shortcut. Ask the user before stopping it."
        );
    }
    Ok(exe)
}

fn launch_app(run: &Run, exe: &Path) -> Result<()> {
    let pid = run.spawn("app", &mut Command::new(exe))?;
    wait_for("the ready line", Duration::from_secs(10), || {
        let log = run.app_log();
        if log.contains("snip-snip running") {
            return Ok(Some(()));
        }
        if run.owned("app").is_none() || log.contains("snip-snip: ") {
            bail!("app failed to start:\n{log}");
        }
        Ok(None)
    })?;
    run.step(&format!("launched pid {pid}; run dir {}", run.dir.display()))
}

fn doctor() -> Result<()> {
    let run = run::current()?;
    let pid = run.owned("app");
    let exe = run::app_exe();
    let log = run.app_log();
    let others: Vec<u32> = run::snip_snip_pids()
        .into_iter()
        .filter(|p| Some(*p) != pid)
        .collect();
    let newest_source = newest_mtime(&run::repo().join("crates"))?;
    let fresh = fs::metadata(&exe)
        .and_then(|m| m.modified())
        .is_ok_and(|built| built >= newest_source);
    let ours = pid
        .and_then(run::process)
        .and_then(|p| p.exe)
        .is_some_and(|p| same_file(&p, &exe));
    let windows = match pid {
        Some(pid) => screen::windows_of(pid)?
            .iter()
            .filter_map(|w| w.title().ok())
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };
    let unlocked = platform::desktop_unlocked();
    let checks: [(&str, bool, String); 7] = [
        ("our app running", pid.is_some(), format!("{pid:?}")),
        ("binary is this repo", ours, exe.display().to_string()),
        (
            "ready line logged",
            log.contains("snip-snip running"),
            String::new(),
        ),
        (
            "no errors logged",
            !log.contains("snip-snip: ") && !log.contains("failed"),
            String::new(),
        ),
        ("no other snip-snip", others.is_empty(), format!("{others:?}")),
        (
            "desktop unlocked",
            unlocked != Some(false),
            format!("{unlocked:?}"),
        ),
        (
            "binary newer than src",
            fresh,
            if fresh { String::new() } else { "rebuild".into() },
        ),
    ];
    println!("run dir                {}", run.dir.display());
    for (label, ok, detail) in &checks {
        println!("{label:<22} {} {detail}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("windows                {windows:?}");
    if checks.iter().any(|(_, ok, _)| !ok) {
        bail!("doctor: not safe to drive");
    }
    println!("doctor: OK");
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    a.canonicalize()
        .ok()
        .zip(b.canonicalize().ok())
        .is_some_and(|(a, b)| a == b)
}

fn newest_mtime(dir: &Path) -> Result<std::time::SystemTime> {
    let mut newest = std::time::UNIX_EPOCH;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let modified = if entry.file_type()?.is_dir() {
            newest_mtime(&entry.path())?
        } else {
            entry.metadata()?.modified()?
        };
        newest = newest.max(modified);
    }
    Ok(newest)
}

fn target() -> Result<()> {
    let run = run::current()?;
    if run.owned("fixture").is_some() {
        bail!("fixture already shown");
    }
    let pid = run.spawn(
        "fixture",
        Command::new(std::env::current_exe()?).arg("fixture-window"),
    )?;
    wait_for("the fixture window", Duration::from_secs(10), || {
        screen::titled(pid, fixture::TITLE)
    })?;
    std::thread::sleep(Duration::from_millis(300));
    let (x, y, w, h) = fixture::BOUNDS;
    run.step(&format!(
        "fixture shown at {x},{y} {w}x{h} (left half blue #2E86DE, right half green #20BF6B)"
    ))
}

/// Presses the hotkey and waits for the overlay's first frame.
fn open_overlay(run: &Run, input: &mut Input, chord: &str) -> Result<()> {
    let frames = run.count_in_log("overlay first frame");
    input.chord(chord)?;
    wait_for("the overlay", Duration::from_secs(10), || {
        Ok((run.count_in_log("overlay first frame") > frames).then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(200));
    Ok(())
}

fn snip(n: Vec<f32>, chord: &str) -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    if screen::titled(pid, screen::EDITOR_TITLE)?.is_some() {
        bail!("an editor is already open; close it first");
    }
    let [x, y, w, h] = [n[0] as i32, n[1] as i32, n[2] as i32, n[3] as i32];
    let mut input = Input::new()?;
    open_overlay(&run, &mut input, chord)?;
    // The overlay's selection includes the pixels under both drag ends.
    input.drag((x, y), (x + w - 1, y + h - 1))?;
    wait_for("the editor window", Duration::from_secs(10), || {
        screen::titled(pid, screen::EDITOR_TITLE)
    })?;
    let scale = xcap::Monitor::from_point(x, y)?.scale_factor()?;
    let (_, image_per_unit) = platform::units(scale);
    // Drag endpoints include one screenshot pixel, not an entire Retina point.
    let image = (
        ((w - 1) as f32 * image_per_unit) as u32 + 1,
        ((h - 1) as f32 * image_per_unit) as u32 + 1,
    );
    fs::write(run.path("image.size"), format!("{}x{}", image.0, image.1))?;
    input.park()?;
    // The topmost fixture would cover the editor; the snip has consumed it.
    let fixture = run.stop("fixture")?;
    std::thread::sleep(Duration::from_millis(300));
    let closed = if fixture.is_some() { "; fixture closed" } else { "" };
    run.step(&format!(
        "snipped screen rect {x},{y} {w}x{h} -> image {}x{}; editor open{closed}",
        image.0, image.1
    ))
}

fn cancel_snip() -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    let mut input = Input::new()?;
    open_overlay(&run, &mut input, "alt+shift+s")?;
    input.chord("esc")?;
    wait_for("the overlay to close", Duration::from_secs(5), || {
        Ok(screen::titled(pid, OVERLAY_TITLE)?.is_none().then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(500));
    if screen::titled(pid, screen::EDITOR_TITLE)?.is_some() {
        bail!("an editor opened after Esc");
    }
    run.step("overlay opened and closed on Esc; no editor")
}

#[cfg(windows)]
fn shell_menu(item: &str) -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    let label = match item {
        "new-snip" => "New snip",
        "settings" => "Settings",
        "quit" => "Quit",
        _ => bail!("expected new-snip, settings, or quit"),
    };
    let frames = run.count_in_log("overlay first frame");
    platform::tray_menu(pid, label, &mut Input::new()?)?;
    match item {
        "new-snip" => {
            wait_for("the menu capture overlay", Duration::from_secs(10), || {
                Ok((run.count_in_log("overlay first frame") > frames).then_some(()))
            })?;
            std::thread::sleep(Duration::from_millis(200));
        }
        "settings" => {
            wait_for("Settings", Duration::from_secs(5), || {
                screen::titled(pid, "snip-snip Settings")
            })?;
        }
        "quit" => {
            wait_for("the app to quit", Duration::from_secs(5), || {
                Ok(run.owned("app").is_none().then_some(()))
            })?;
        }
        _ => unreachable!(),
    }
    run.step(&format!("selected tray menu {label}"))
}

fn shell_key(chords: &[String]) -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    let mut input = Input::new()?;
    if let Some(settings) = screen::titled(pid, "snip-snip Settings")? {
        platform::focus(&settings, pid, &mut input)?;
    }
    for chord in chords {
        input.chord(chord)?;
    }
    run.step(&format!("sent shell keys {}", chords.join(" ")))
}

/// Focuses the editor so keys reach it, returning its window.
fn focus_editor(run: &Run, input: &mut Input) -> Result<xcap::Window> {
    let pid = app(run)?;
    let window = screen::editor(pid)?;
    platform::focus(&window, pid, input)?;
    Ok(window)
}

fn layout() -> Result<()> {
    let run = run::current()?;
    let window = screen::editor(app(&run)?)?;
    let l = Layout::of(&run, &window)?;
    println!(
        "image {}x{} drawn at screen {:?}, {} screen units per image pixel",
        l.image.0, l.image.1, l.origin, l.fit
    );
    let content = platform::content_rect(&window)?;
    let capture = window.capture_image()?;
    println!(
        "window screen {},{} {}x{}, capture {}x{}, content {},{} {}x{}",
        window.x()?,
        window.y()?,
        window.width()?,
        window.height()?,
        capture.width(),
        capture.height(),
        content.x,
        content.y,
        content.w,
        content.h
    );
    Ok(())
}

#[cfg(windows)]
fn paint_launch(exe: &str) -> Result<()> {
    let system = sysinfo::System::new_all();
    if system
        .processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case("mspaint.exe"))
    {
        bail!("Paint is already running; use a fresh owned session");
    }
    let run = run::current()?;
    let pid = run.spawn("paint", &mut Command::new(exe))?;
    wait_for("the owned Paint window", Duration::from_secs(15), || {
        Ok((!screen::windows_of(pid)?.is_empty()).then_some(()))
    })?;
    run.step(&format!("launched owned Paint pid {pid}"))
}

#[cfg(windows)]
fn paint_window(run: &Run) -> Result<(u32, xcap::Window)> {
    let pid = run.owned("paint").context("no owned Paint process")?;
    let windows = screen::windows_of(pid)?;
    if let Some(dialog) = windows.iter().find(|w| w.title().is_ok_and(|t| t == "Save as")) {
        return Ok((pid, dialog.clone()));
    }
    for window in &windows {
        if platform::is_focused(window)? {
            return Ok((pid, window.clone()));
        }
    }
    let window = windows
        .into_iter()
        .max_by_key(|w| w.width().unwrap_or(0) * w.height().unwrap_or(0))
        .context("no owned Paint window")?;
    Ok((pid, window))
}

#[cfg(windows)]
fn paint_input(args: &[String], text: bool) -> Result<()> {
    let run = run::current()?;
    let (pid, window) = paint_window(&run)?;
    let mut input = Input::new()?;
    platform::focus(&window, pid, &mut input)?;
    if text {
        input.text(&args.join(" "))?;
    } else {
        for key in args {
            input.chord(key)?;
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    run.step(&format!("Paint {} {args:?}", if text { "type" } else { "key" }))
}

#[cfg(windows)]
fn paint_shot(name: &str) -> Result<()> {
    let run = run::current()?;
    let (pid, window) = paint_window(&run)?;
    let path = run.path(&format!("{name}.png"));
    window.capture_image()?.save(&path)?;
    let titles: Vec<_> = screen::windows_of(pid)?
        .iter()
        .filter_map(|w| w.title().ok())
        .collect();
    run.step(&format!(
        "Paint shot {}, windows={titles:?}, window screen {},{} {}x{}",
        path.display(),
        window.x()?,
        window.y()?,
        window.width()?,
        window.height()?
    ))
}

fn key(combos: &[String]) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    for combo in combos {
        input.chord(combo)?;
    }
    std::thread::sleep(Duration::from_millis(200));
    run.step(&format!("key {}", combos.join(" ")))
}

fn type_text(text: &str) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    input.type_keys(text)?;
    std::thread::sleep(Duration::from_millis(200));
    run.step(&format!("type {text:?}"))
}

fn drag(n: Vec<f32>, held: Option<&str>, absolute: bool) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    let window = focus_editor(&run, &mut input)?;
    let (a, b) = if absolute {
        (
            (n[0].round() as i32, n[1].round() as i32),
            (n[2].round() as i32, n[3].round() as i32),
        )
    } else {
        let layout = Layout::of(&run, &window)?;
        (layout.to_screen((n[0], n[1])), layout.to_screen((n[2], n[3])))
    };
    if let Some(key) = held {
        input.held(key, |input| input.drag(a, b))?;
    } else {
        input.drag(a, b)?;
    }
    input.park()?;
    run.step(&format!(
        "drag ({},{})->({},{}) = screen {a:?}->{b:?}, held={held:?}, absolute={absolute}",
        n[0], n[1], n[2], n[3]
    ))
}

fn double_click(n: Vec<f32>) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    let layout = Layout::of(&run, &focus_editor(&run, &mut input)?)?;
    let at = layout.to_screen((n[0], n[1]));
    input.double_click(at)?;
    input.park()?;
    run.step(&format!("double-click image ({},{}) = screen {at:?}", n[0], n[1]))
}

fn drag_preview(n: Vec<f32>, name: &str) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    let window = focus_editor(&run, &mut input)?;
    let layout = Layout::of(&run, &window)?;
    let (a, b) = (layout.to_screen((n[0], n[1])), layout.to_screen((n[2], n[3])));
    let path = run.path(&format!("{name}.png"));
    input.drag_with_preview(a, b, || screen::shot(app(&run)?, &path))?;
    input.park()?;
    run.step(&format!(
        "drag screen {a:?}->{b:?}, held-button preview {}",
        path.display()
    ))
}

fn screen_pointer(n: Vec<f32>, click: bool) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    let at = (n[0].round() as i32, n[1].round() as i32);
    if click {
        input.click(at)?;
        input.park()?;
    } else {
        // Approach and settle within the same hit region, as a real pointer
        // movement does. Windows applies WM_SETCURSOR before GPUI handles
        // the first move's newly selected cursor.
        input.move_to((at.0 - 1, at.1))?;
        std::thread::sleep(Duration::from_millis(100));
        input.move_to(at)?;
        std::thread::sleep(Duration::from_millis(200));
    }
    run.step(&format!(
        "{} screen {at:?}",
        if click { "click" } else { "hover" }
    ))
}

fn click_desktop(n: Vec<f32>) -> Result<()> {
    let run = run::current()?;
    let at = (n[0].round() as i32, n[1].round() as i32);
    Input::new()?.click(at)?;
    run.step(&format!("desktop click {at:?}"))
}

fn scroll_screen(n: Vec<f32>, notches: i32, held: Option<&str>) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    let at = (n[0].round() as i32, n[1].round() as i32);
    if let Some(key) = held {
        input.held(key, |input| input.scroll(at, notches))?;
    } else {
        input.scroll(at, notches)?;
    }
    input.park()?;
    run.step(&format!("scroll screen {at:?}, notches={notches}, held={held:?}"))
}

fn click(n: Vec<f32>) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    let layout = Layout::of(&run, &focus_editor(&run, &mut input)?)?;
    let at = layout.to_screen((n[0], n[1]));
    input.click(at)?;
    input.park()?;
    run.step(&format!("click image ({},{}) = screen {at:?}", n[0], n[1]))
}

fn shot(name: &str) -> Result<()> {
    let run = run::current()?;
    let path = run.path(&format!("{name}.png"));
    screen::shot(app(&run)?, &path)?;
    run.step(&format!("shot {}", path.display()))
}

fn clipboard(name: &str) -> Result<()> {
    let run = run::current()?;
    let path = run.path(&format!("{name}.png"));
    screen::save_clipboard(&path)?;
    let formats = platform::clipboard_formats()?;
    run.step(&format!(
        "clipboard image -> {} (formats: {})",
        path.display(),
        formats.join(", ")
    ))
}

fn save(name: &str) -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    let path = run.path(&format!("{name}.png"));
    if path.exists() {
        bail!("{} exists; pick a new name", path.display());
    }
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    input.chord("mod+s")?;
    platform::save_as(pid, &path, &mut input)?;
    wait_for("the saved file", Duration::from_secs(10), || {
        Ok(fs::metadata(&path).is_ok_and(|m| m.len() > 0).then_some(()))
    })?;
    run.step(&format!("saved via dialog -> {}", path.display()))
}

fn cancel_save() -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    input.chord("mod+s")?;
    platform::cancel_save(pid, &mut input)?;
    screen::editor(pid).context("canceling the save dialog closed the editor")?;
    run.step("canceled native save dialog; editor remains open")
}

fn inspect(name: &str) -> Result<()> {
    let path = Path::new(name);
    let path = if path.exists() {
        path.to_path_buf()
    } else {
        run::current()?.path(&format!("{name}.png"))
    };
    println!("{}", screen::inspect(&path)?);
    Ok(())
}

fn close() -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    let mut input = Input::new()?;
    focus_editor(&run, &mut input)?;
    input.chord("esc")?;
    std::thread::sleep(Duration::from_millis(200));
    if screen::titled(pid, screen::EDITOR_TITLE)?.is_some() {
        input.chord("esc")?;
    }
    wait_for("the editor to close", Duration::from_secs(5), || {
        Ok(screen::titled(pid, screen::EDITOR_TITLE)?.is_none().then_some(()))
    })?;
    run.step("editor closed")
}

fn cleanup() -> Result<()> {
    let run = run::current()?;
    for name in ["app", "fixture", "paint"] {
        if let Some(pid) = run.stop(name)? {
            println!("stopped {name} pid {pid}");
        }
    }
    println!("evidence kept in {}", run.dir.display());
    Ok(())
}
