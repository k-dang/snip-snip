//! The run: a directory under `<repo>/target/verify/` holding the evidence of
//! one verification session, plus the processes that session started.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// The snip-snip checkout this tool was built from.
pub fn repo() -> PathBuf {
    // .claude/skills/verify-snip-snip/snipctl is four levels below the root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repo root")
        .into()
}

pub fn app_exe() -> PathBuf {
    repo()
        .join("target")
        .join("release")
        .join(format!("snip-snip{}", std::env::consts::EXE_SUFFIX))
}

fn verify_root() -> PathBuf {
    repo().join("target").join("verify")
}

pub struct Run {
    pub dir: PathBuf,
}

/// Creates a fresh run directory and makes it current.
pub fn start() -> Result<Run> {
    let name = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .to_string();
    let dir = verify_root().join(&name);
    fs::create_dir_all(&dir)?;
    fs::write(verify_root().join("current"), &name)?;
    Ok(Run { dir })
}

/// The run started by the latest `launch`.
pub fn current() -> Result<Run> {
    let name =
        fs::read_to_string(verify_root().join("current")).context("no run; start one with `launch`")?;
    Ok(Run {
        dir: verify_root().join(name.trim()),
    })
}

impl Run {
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Prints `line` and appends it, timestamped, to `steps.log`, so the
    /// evidence records each action next to the state it produced.
    pub fn step(&self, line: &str) -> Result<()> {
        println!("{line}");
        let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path("steps.log"))?;
        writeln!(log, "{millis} {line}")?;
        Ok(())
    }

    pub fn app_log(&self) -> String {
        fs::read_to_string(self.path("app.log")).unwrap_or_default()
    }

    pub fn count_in_log(&self, needle: &str) -> usize {
        self.app_log().matches(needle).count()
    }

    /// Starts `command` detached from this process, with output in
    /// `<name>.log`, and records it as `<name>.pid` so only it is ever stopped.
    pub fn spawn(&self, name: &str, command: &mut Command) -> Result<u32> {
        let log = fs::File::create(self.path(&format!("{name}.log")))?;
        command.stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
        detach(command);
        let child = command.spawn().with_context(|| format!("starting {name}"))?;
        let pid = child.id();
        let started = process(pid).context("child exited at once")?.start_time;
        fs::write(self.path(&format!("{name}.pid")), format!("{pid} {started}"))?;
        Ok(pid)
    }

    /// The pid recorded for `name`, if that same process is still running.
    pub fn owned(&self, name: &str) -> Option<u32> {
        let record = fs::read_to_string(self.path(&format!("{name}.pid"))).ok()?;
        let (pid, started) = record.trim().split_once(' ')?;
        let pid: u32 = pid.parse().ok()?;
        (process(pid)?.start_time.to_string() == started).then_some(pid)
    }

    /// Stops a process this run started (never one found by name).
    pub fn stop(&self, name: &str) -> Result<Option<u32>> {
        let Some(pid) = self.owned(name) else {
            return Ok(None);
        };
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        if let Some(p) = system.process(Pid::from_u32(pid)) {
            p.kill();
        }
        wait_for("the process to exit", Duration::from_secs(5), || {
            Ok(process(pid).is_none().then_some(()))
        })?;
        Ok(Some(pid))
    }
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

pub struct ProcessInfo {
    start_time: u64,
    pub exe: Option<PathBuf>,
}

pub fn process(pid: u32) -> Option<ProcessInfo> {
    let mut system = System::new();
    let pid = Pid::from_u32(pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::everything(),
    );
    let p = system.process(pid)?;
    Some(ProcessInfo {
        start_time: p.start_time(),
        exe: p.exe().map(Path::to_path_buf),
    })
}

/// Every running snip-snip, ours or not.
pub fn snip_snip_pids() -> Vec<u32> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let name = format!("snip-snip{}", std::env::consts::EXE_SUFFIX);
    system
        .processes_by_exact_name(name.as_ref())
        .map(|p| p.pid().as_u32())
        .collect()
}

/// Polls `check` every 100 ms until it yields a value or `timeout` passes.
pub fn wait_for<T>(what: &str, timeout: Duration, mut check: impl FnMut() -> Result<Option<T>>) -> Result<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = check()? {
            return Ok(value);
        }
        if Instant::now() > deadline {
            bail!("timed out waiting for {what}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
