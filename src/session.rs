use crate::config::Config;
use serde::Deserialize;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    process::{Child, Command},
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IdleConfig {
    pub lock_after_seconds: u32,
    pub monitor_off_after_seconds: u32,
}
impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            lock_after_seconds: 300,
            monitor_off_after_seconds: 600,
        }
    }
}
impl IdleConfig {
    pub fn validate(&self) -> Result<()> {
        if self.lock_after_seconds > 86400 || self.monitor_off_after_seconds > 86400 {
            return Err("idle timeouts must be between 0 and 86400 seconds".into());
        }
        if self.monitor_off_after_seconds > 0
            && (self.lock_after_seconds == 0
                || self.monitor_off_after_seconds <= self.lock_after_seconds)
        {
            return Err("monitor-off timeout must be later than the enabled lock timeout".into());
        }
        Ok(())
    }
}

pub fn locker(config: &Config) -> Command {
    let mut command = Command::new("swaylock");
    command.args([
        "--config",
        "/dev/null",
        "--show-keyboard-layout",
        "--indicator-idle-visible",
    ]);
    let a = &config.appearance;
    for (flag, color) in [
        ("--color", a.background),
        ("--inside-color", a.surface),
        ("--ring-color", a.active_border),
        ("--text-color", a.text),
        ("--key-hl-color", a.text),
        ("--bs-hl-color", a.muted_text),
        ("--layout-bg-color", a.surface),
        ("--layout-text-color", a.text),
        ("--layout-border-color", a.inactive_border),
    ] {
        command.args([flag, color.css().trim_start_matches('#')]);
    }
    for state in ["clear", "caps-lock", "ver", "wrong"] {
        command.args([
            format!("--inside-{state}-color"),
            a.surface.css()[1..].into(),
        ]);
        command.args([
            format!("--ring-{state}-color"),
            a.active_border.css()[1..].into(),
        ]);
        command.args([format!("--text-{state}-color"), a.text.css()[1..].into()]);
    }
    command.arg("--line-uses-ring");
    command
}

pub fn start_lock(config: &Config, child: &mut Option<Child>, locked: bool) {
    if let Some(process) = child {
        match process.try_wait() {
            Ok(None) => return,
            Ok(Some(_)) => *child = None,
            Err(error) => {
                eprintln!("Cannot check screen locker: {error}");
                return;
            }
        }
    }
    if locked {
        return;
    }
    match locker(config).spawn() {
        Ok(process) => *child = Some(process),
        Err(error) => eprintln!("Cannot lock session: {error}"),
    }
}

/// Exit successfully only after River acknowledges the actual session lock.
pub fn lock_and_wait() -> Result<()> {
    let path = std::env::var_os("MYWM_SOCKET")
        .ok_or("MYWM_SOCKET is not set; start River with scripts/river-init")?;
    let mut socket = UnixStream::connect(path)?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    socket.write_all(b"v1 lock\n")?;
    let mut reader = BufReader::new(socket);
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("River did not confirm the session lock")?;
        reader.get_ref().set_read_timeout(Some(remaining))?;
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err("WM disconnected before confirming the session lock".into());
        }
        if line.trim() == "v1 locked 1" {
            return Ok(());
        }
        if line.starts_with("v1 error") {
            return Err(line.into());
        }
    }
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn idle_args(config: &IdleConfig, executable: &str) -> Vec<String> {
    let lock = format!("{} --lock", quote(executable));
    let on = "wlopm --on '*'";
    let mut args = vec!["-w".into(), "-C".into(), "/dev/null".into()];
    if config.lock_after_seconds > 0 {
        args.extend([
            "timeout".into(),
            config.lock_after_seconds.to_string(),
            lock.clone(),
        ]);
    }
    if config.monitor_off_after_seconds > 0 {
        args.extend([
            "timeout".into(),
            config.monitor_off_after_seconds.to_string(),
            format!("{lock} && wlopm --off '*'"),
            "resume".into(),
            on.into(),
        ]);
    }
    args.extend([
        "before-sleep".into(),
        lock.clone(),
        "after-resume".into(),
        on.into(),
        "lock".into(),
        lock,
    ]);
    args
}
pub fn idle(config: &IdleConfig) -> Result<()> {
    let executable = std::env::current_exe()?;
    let executable = executable.to_str().ok_or("Executable path is not UTF-8")?;
    Err(Command::new("swayidle")
        .args(idle_args(config, executable))
        .exec()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_commands_quote_paths_and_wait_for_lock_before_power_off() {
        let args = idle_args(&IdleConfig::default(), "/tmp/a'b/my wm");
        assert!(args.contains(&"'/tmp/a'\\''b/my wm' --lock && wlopm --off '*'".into()));
        assert!(args.contains(&"before-sleep".into()));
        let disabled = IdleConfig {
            lock_after_seconds: 0,
            monitor_off_after_seconds: 0,
        };
        disabled.validate().unwrap();
        assert!(!idle_args(&disabled, "mywm").contains(&"timeout".into()));
        assert!(
            IdleConfig {
                lock_after_seconds: 0,
                monitor_off_after_seconds: 600
            }
            .validate()
            .is_err()
        );
        assert!(
            IdleConfig {
                lock_after_seconds: 300,
                monitor_off_after_seconds: 100
            }
            .validate()
            .is_err()
        );
    }
}
