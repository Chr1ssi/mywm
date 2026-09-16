use crate::{Action, river::river_window_management::river_seat_v1::Modifiers};
use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub workspaces: usize,
    pub terminal: Vec<String>,
    pub bindings: Bindings,
    pub float_dialogs: bool,
    pub rules: Vec<crate::rules::Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspaces: 9,
            terminal: vec!["kitty".into()],
            bindings: Bindings::default(),
            float_dialogs: true,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    terminal: Vec<String>,
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
            toggle_floating: keys(&["Super+v"]),
            pointer_modifiers: "Super".into(),
            terminal: keys(&["Super+Return"]),
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
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
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
        for (index, rule) in config.rules.iter().enumerate() {
            rule.validate(config.workspaces)
                .map_err(|error| format!("rules[{}]: {error}", index + 1))?;
        }
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
                println!("Configuration: {}", path.display());
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
            (&self.bindings.terminal, Action::Terminal),
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
    fn defaults_and_partial_configuration() {
        let defaults = Config::parse("").unwrap();
        assert_eq!(defaults.workspaces, 9);
        assert_eq!(defaults.keybindings().unwrap().len(), 30);
        let config =
            Config::parse("workspaces = 3\nterminal = ['kitty', '--single-instance']").unwrap();
        assert_eq!(config.keybindings().unwrap().len(), 18);
        assert_eq!(config.terminal[1], "--single-instance");
        Config::parse(include_str!("../config/mywm.toml")).unwrap();
    }

    #[test]
    fn invalid_config_is_rejected_before_connecting_to_river() {
        for text in [
            "workspaces = 0",
            "workspaces = 10",
            "terminal = []",
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
