use crate::{Action, river::river_window_management::river_seat_v1::Modifiers};
use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub workspace_outputs: std::collections::BTreeMap<String, Vec<usize>>,
    pub wallpaper_directory: String,
    pub idle: crate::session::IdleConfig,
    pub keyboard: crate::keyboard::KeyboardConfig,
    pub workspaces: usize,
    pub terminal: Vec<String>,
    pub launcher: Vec<String>,
    pub bindings: Bindings,
    pub appearance: crate::appearance::Appearance,
    pub float_dialogs: bool,
    pub rules: Vec<crate::rules::Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspace_outputs: Default::default(),
            wallpaper_directory: std::env::var("HOME")
                .map(|home| format!("{home}/Bilder/Wallpaper"))
                .unwrap_or_else(|_| "/usr/share/backgrounds".into()),
            idle: crate::session::IdleConfig::default(),
            keyboard: crate::keyboard::KeyboardConfig::default(),
            workspaces: 9,
            terminal: vec!["kitty".into()],
            launcher: vec![
                "qs".into(),
                "--path".into(),
                crate::shell::qml("shell.qml")
                    .to_string_lossy()
                    .into_owned(),
                "--no-duplicate".into(),
            ],
            bindings: Bindings::default(),
            appearance: crate::appearance::Appearance::default(),
            float_dialogs: true,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    wallpaper: Vec<String>,
    lock: Vec<String>,
    terminal: Vec<String>,
    launcher: Vec<String>,
    close: Vec<String>,
    exit: Vec<String>,
    focus_left: Vec<String>,
    focus_right: Vec<String>,
    move_left: Vec<String>,
    move_right: Vec<String>,
    toggle_floating: Vec<String>,
    pointer_modifiers: String,
    workspace_modifiers: String,
    move_to_workspace_modifiers: String,
}

impl Default for Bindings {
    fn default() -> Self {
        let keys = |values: &[&str]| values.iter().map(|v| (*v).into()).collect();
        Self {
            wallpaper: keys(&["Super+Shift+w"]),
            lock: keys(&["Super+Escape"]),
            toggle_floating: keys(&["Super+v"]),
            pointer_modifiers: "Super".into(),
            terminal: keys(&["Super+Return"]),
            launcher: keys(&["Super+Space"]),
            close: keys(&["Super+q"]),
            exit: keys(&["Super+m"]),
            focus_left: keys(&["Super+h", "Super+Left"]),
            focus_right: keys(&["Super+l", "Super+Right"]),
            move_left: keys(&["Super+Shift+h", "Super+Shift+Left"]),
            move_right: keys(&["Super+Shift+l", "Super+Shift+Right"]),
            workspace_modifiers: "Super".into(),
            move_to_workspace_modifiers: "Super+Shift".into(),
        }
    }
}

impl Config {
    pub fn apply_theme(&self, command: &mut std::process::Command) {
        let a = &self.appearance;
        for (name, color) in [
            ("BACKGROUND", a.background),
            ("SURFACE", a.surface),
            ("TEXT", a.text),
            ("MUTED", a.muted_text),
            ("ACCENT", a.active_border),
            ("BORDER", a.inactive_border),
        ] {
            command.env(format!("MYWM_COLOR_{name}"), color.css());
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        if !config.workspace_outputs.is_empty() {
            let mut seen = HashSet::new();
            for (output, numbers) in &config.workspace_outputs {
                if output.trim().is_empty() || numbers.is_empty() {
                    return Err(
                        "workspace_outputs requires nonempty monitor names and workspace lists"
                            .into(),
                    );
                }
                for number in numbers {
                    if !(1..=config.workspaces).contains(number) || !seen.insert(*number) {
                        return Err(
                            "workspace_outputs must assign each workspace exactly once".into()
                        );
                    }
                }
            }
            if seen.len() != config.workspaces {
                return Err("workspace_outputs must assign every workspace".into());
            }
        }
        if !(1..=9).contains(&config.workspaces) {
            return Err("workspaces must be between 1 and 9".into());
        }
        if config
            .terminal
            .first()
            .is_none_or(|program| program.trim().is_empty())
        {
            return Err("terminal must contain a program, e.g. [\"kitty\"]".into());
        }
        if config
            .launcher
            .first()
            .is_none_or(|program| program.trim().is_empty())
        {
            return Err("launcher must contain a program".into());
        }
        for (index, rule) in config.rules.iter().enumerate() {
            rule.validate(config.workspaces)
                .map_err(|error| format!("rules[{}]: {error}", index + 1))?;
        }
        if !std::path::Path::new(&config.wallpaper_directory).is_absolute() {
            return Err("wallpaper_directory must be an absolute path".into());
        }
        config.idle.validate()?;
        config.appearance.validate()?;
        config.keybindings()?;
        config.pointer_modifiers()?;
        Ok(config)
    }

    pub fn load() -> Result<Self> {
        let explicit = std::env::var_os("MYWM_CONFIG").map(PathBuf::from);
        let path = explicit.clone().or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
                .map(|p| p.join("mywm/config.toml"))
        });
        let Some(path) = path else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let config = Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                eprintln!("Configuration: {}", path.display());
                Ok(config)
            }
            Err(e) if explicit.is_none() && e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(e) => Err(format!("{}: {e}", path.display()).into()),
        }
    }

    pub fn pointer_modifiers(&self) -> Result<Modifiers> {
        Ok(parse_key(&format!("{}+a", self.bindings.pointer_modifiers))?.1)
    }

    pub fn keybindings(&self) -> Result<Vec<(u32, Modifiers, Action)>> {
        let mut bindings = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |key: &str, action| -> Result<()> {
            let (symbol, modifiers) = parse_key(key)?;
            if !seen.insert((symbol, modifiers.bits())) {
                return Err(format!("Duplicate keybinding: {key}").into());
            }
            bindings.push((symbol, modifiers, action));
            Ok(())
        };
        for (keys, action) in [
            (&self.bindings.wallpaper, Action::Wallpaper),
            (&self.bindings.lock, Action::Lock),
            (&self.bindings.terminal, Action::Terminal),
            (&self.bindings.launcher, Action::Launcher),
            (&self.bindings.close, Action::Close),
            (&self.bindings.exit, Action::Exit),
            (&self.bindings.toggle_floating, Action::ToggleFloating),
            (&self.bindings.focus_left, Action::Focus(-1)),
            (&self.bindings.focus_right, Action::Focus(1)),
            (&self.bindings.move_left, Action::Move(-1)),
            (&self.bindings.move_right, Action::Move(1)),
        ] {
            for key in keys {
                add(key, action)?;
            }
        }
        for workspace in 0..self.workspaces {
            add(
                &format!("{}+{}", self.bindings.workspace_modifiers, workspace + 1),
                Action::Workspace(workspace),
            )?;
            add(
                &format!(
                    "{}+{}",
                    self.bindings.move_to_workspace_modifiers,
                    workspace + 1
                ),
                Action::MoveToWorkspace(workspace),
            )?;
        }
        Ok(bindings)
    }
}

fn parse_key(key: &str) -> Result<(u32, Modifiers)> {
    let parts: Vec<_> = key.split('+').map(str::trim).collect();
    let mut modifiers = Modifiers::empty();
    for modifier in &parts[..parts.len() - 1] {
        let flag = match modifier.to_ascii_lowercase().as_str() {
            "super" => Modifiers::Mod4,
            "shift" => Modifiers::Shift,
            "ctrl" | "control" => Modifiers::Ctrl,
            "alt" => Modifiers::Mod1,
            _ => return Err(format!("Unknown modifier in keybinding: {key}").into()),
        };
        if modifiers.contains(flag) {
            return Err(format!("Repeated modifier in keybinding: {key}").into());
        }
        modifiers |= flag;
    }
    let name = parts.last().unwrap().to_ascii_lowercase();
    let symbol = match name.as_str() {
        "return" | "enter" => 0xff0d,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "space" => 0x20,
        "tab" => 0xff09,
        "escape" | "esc" => 0xff1b,
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphanumeric() => {
            value.as_bytes()[0] as u32
        }
        _ => return Err(format!("Unsupported key in keybinding: {key}").into()),
    };
    Ok((symbol, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_outputs_must_assign_every_global_number_once() {
        Config::parse("workspaces = 3\n[workspace_outputs]\nDP-1 = [1]\nDP-3 = [2, 3]").unwrap();
        for mapping in [
            "DP-1 = [1, 2]",
            "DP-1 = [1, 2]\nDP-3 = [2, 3]",
            "DP-1 = [0, 1, 2]",
            "DP-1 = [1, 2, 4]",
            "DP-1 = []",
        ] {
            assert!(
                Config::parse(&format!("workspaces = 3\n[workspace_outputs]\n{mapping}")).is_err()
            );
        }
    }

    #[test]
    fn defaults_and_partial_configuration() {
        let defaults = Config::parse("").unwrap();
        assert_eq!(defaults.workspaces, 9);
        assert_eq!(defaults.keybindings().unwrap().len(), 33);
        let config =
            Config::parse("workspaces = 3\nterminal = ['kitty', '--single-instance']").unwrap();
        assert_eq!(config.keybindings().unwrap().len(), 21);
        assert_eq!(config.terminal[1], "--single-instance");
        Config::parse(include_str!("../config/mywm.toml")).unwrap();
    }

    #[test]
    fn invalid_config_is_rejected_before_connecting_to_river() {
        for text in [
            "workspaces = 0",
            "workspaces = 10",
            "terminal = []",
            "launcher = []",
            "launcher = ['']",
            "terminal = ['']",
            "workpace = 3",
            "[bindings]\nunknown = []",
            "[bindings]\nexit = ['Super+q']",
            "[bindings]\nexit = ['Super+Bogus']",
            "[bindings]\nexit = ['Super+1']",
            "[bindings]\nexit = ['Super+Super+m']",
            "workspaces =",
            "[bindings]\npointer_modifiers = 'Bogus'",
        ] {
            assert!(Config::parse(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn key_aliases_and_modifiers() {
        assert_eq!(
            parse_key("Super+Return").unwrap(),
            parse_key("super+enter").unwrap()
        );
        let (_, modifiers) = parse_key("Super+Shift+2").unwrap();
        assert_eq!(modifiers, Modifiers::Mod4 | Modifiers::Shift);
        assert_eq!(parse_key("Ctrl+Alt+h").unwrap().0, 0x68);
    }
}
