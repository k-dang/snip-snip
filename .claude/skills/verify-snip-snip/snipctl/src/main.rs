//! snipctl: drives a real snip-snip instance with OS input and captures proof.
//! See ../SKILL.md. Positions given to `snip` are screen units (physical
//! pixels on Windows, points on macOS); `drag` and `click` take image pixels.

mod fixture;
mod input;
mod platform;
mod run;
mod screen;

use std::{fs, path::Path, process::Command, time::Duration};

use anyhow::{Context as _, Result, bail};

use input::Input;
use run::{Run, wait_for};
use screen::{Layout, OVERLAY_TITLE};

const USAGE: &str = "\
snipctl <command> [args]

  launch                start target/release/snip-snip in a new run dir; waits for ready
  doctor                read-only health check of the current run
  target                show the fixture (blue|green halves) at screen 100,100 size 800x500
  snip X Y W H          Alt+Shift+S, drag that screen rect on the overlay, wait for the editor
  cancel-snip           Alt+Shift+S, then Esc on the overlay; asserts no editor opens
  layout                print where the editor draws the image
  key COMBO...          focus the editor and press chords: r  mod+z  mod+shift+z  left  home  esc
  type TEXT...          focus the editor and type; letters, digits, spaces as keys, others as Unicode
  drag X1 Y1 X2 Y2      focus the editor and drag between two points in image pixels
  click X Y             focus the editor and click a point in image pixels
  shot NAME             save the editor window (or the primary screen) to <run>/NAME.png
  clipboard NAME        save the clipboard image to <run>/NAME.png
  save NAME             mod+s, enter <run>/NAME.png in the save dialog, wait for the file
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
        "launch" => launch(),
        "doctor" => doctor(),
        "target" => target(),
        "fixture-window" => fixture::run(),
        "snip" => snip(numbers(rest, 4)?),
        "cancel-snip" => cancel_snip(),
        "layout" => layout(),
        "key" => key(rest),
        "type" => type_text(&rest.join(" ")),
        "drag" => drag(numbers(rest, 4)?),
        "click" => click(numbers(rest, 2)?),
        "shot" => shot(name(rest)?),
        "clipboard" => clipboard(name(rest)?),
        "save" => save(name(rest)?),
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
            "snip-snip already running (pid {others:?}); it owns Alt+Shift+S. Ask the user before stopping it."
        );
    }
    let run = run::start()?;
    let pid = run.spawn("app", &mut Command::new(&exe))?;
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
fn open_overlay(run: &Run, input: &mut Input) -> Result<()> {
    let frames = run.count_in_log("overlay first frame");
    input.chord("alt+shift+s")?;
    wait_for("the overlay", Duration::from_secs(10), || {
        Ok((run.count_in_log("overlay first frame") > frames).then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(200));
    Ok(())
}

fn snip(n: Vec<f32>) -> Result<()> {
    let run = run::current()?;
    let pid = app(&run)?;
    if screen::titled(pid, screen::EDITOR_TITLE)?.is_some() {
        bail!("an editor is already open; close it first");
    }
    let [x, y, w, h] = [n[0] as i32, n[1] as i32, n[2] as i32, n[3] as i32];
    let mut input = Input::new()?;
    open_overlay(&run, &mut input)?;
    // The overlay's selection includes the pixels under both drag ends.
    input.drag((x, y), (x + w - 1, y + h - 1))?;
    wait_for("the editor window", Duration::from_secs(10), || {
        screen::titled(pid, screen::EDITOR_TITLE)
    })?;
    let scale = xcap::Monitor::from_point(x, y)?.scale_factor()?;
    let (_, image_per_unit) = platform::units(scale);
    let image = (
        (w as f32 * image_per_unit) as u32,
        (h as f32 * image_per_unit) as u32,
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
    open_overlay(&run, &mut input)?;
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

/// Focuses the editor so keys reach it, returning its window.
fn focus_editor(run: &Run, input: &mut Input) -> Result<xcap::Window> {
    let pid = app(run)?;
    let window = screen::editor(pid)?;
    platform::focus(&window, pid, input)?;
    Ok(window)
}

fn layout() -> Result<()> {
    let run = run::current()?;
    let l = Layout::of(&run, &screen::editor(app(&run)?)?)?;
    println!(
        "image {}x{} drawn at screen {:?}, {} screen units per image pixel",
        l.image.0, l.image.1, l.origin, l.fit
    );
    Ok(())
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

fn drag(n: Vec<f32>) -> Result<()> {
    let run = run::current()?;
    let mut input = Input::new()?;
    let layout = Layout::of(&run, &focus_editor(&run, &mut input)?)?;
    let (a, b) = (layout.to_screen((n[0], n[1])), layout.to_screen((n[2], n[3])));
    input.drag(a, b)?;
    input.park()?;
    run.step(&format!(
        "drag image ({},{})->({},{}) = screen {a:?}->{b:?}",
        n[0], n[1], n[2], n[3]
    ))
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
    for name in ["app", "fixture"] {
        if let Some(pid) = run.stop(name)? {
            println!("stopped {name} pid {pid}");
        }
    }
    println!("evidence kept in {}", run.dir.display());
    Ok(())
}
