use crate::config::Config;
use std::{os::unix::process::CommandExt, path::PathBuf, process::Command};

pub fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let state_home = std::env::var_os("XDG_STATE_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .ok_or("HOME or XDG_STATE_HOME is required for wallpaper persistence")?;
    let state_dir = state_home.join("mywm");
    std::fs::create_dir_all(&state_dir)?;
    let mut command = Command::new("qs");
    command
        .arg("--path")
        .arg(crate::shell::qml("wallpaper.qml"))
        .arg("--no-duplicate")
        .env("MYWM_WALLPAPER_HELPER", std::env::current_exe()?)
        .env("MYWM_WALLPAPER_DIRECTORY", &config.wallpaper_directory)
        .env("MYWM_WALLPAPER_STATE", state_dir.join("wallpaper.json"));
    config.apply_theme(&mut command);
    Err(command.exec().into())
}
pub fn picker(x: i32, y: i32) -> Command {
    let mut command = Command::new("qs");
    command
        .args(["ipc", "--path"])
        .arg(crate::shell::qml("wallpaper.qml"))
        .args([
            "call",
            "wallpaper",
            "openPicker",
            &x.to_string(),
            &y.to_string(),
        ]);
    command
}

pub fn list(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&config.wallpaper_directory)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "webp" | "bmp"
                    )
                })
        {
            paths.push(path);
        }
    }
    paths.sort();
    use std::io::Write;
    let mut output = std::io::stdout().lock();
    for path in paths {
        if let Some(path) = path.to_str() {
            writeln!(output, "{}", file_url(path))?;
        }
    }
    Ok(())
}
fn file_url(path: &str) -> String {
    let mut url = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            url.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(url, "%{byte:02X}").unwrap();
        }
    }
    url
}
#[cfg(test)]
mod tests {
    #[test]
    fn picker_command_uses_separate_coordinate_arguments() {
        let command = super::picker(-1280, 720);
        let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
        let qml = crate::shell::qml("wallpaper.qml");
        assert_eq!(
            args,
            [
                "ipc",
                "--path",
                qml.to_str().unwrap(),
                "call",
                "wallpaper",
                "openPicker",
                "-1280",
                "720"
            ]
        );
    }
    #[test]
    fn paths_are_encoded_without_interpreting_url_delimiters() {
        assert_eq!(
            super::file_url("/tmp/Bild #1% ä.png"),
            "file:///tmp/Bild%20%231%25%20%C3%A4.png"
        );
    }
}
