use crate::{Action, OutputDirection, river::river_window_management::river_seat_v1::Modifiers};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

/// Leaves at least one number of 1 to 9 free for dynamic workspaces.
const MAX_HOME_WORKSPACES: usize = crate::workspaces::MAX_NUMBER - 1;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Monitors in workspace order: the first gets workspace 1, the next 2, ...
    pub workspace_outputs: Vec<String>,
    pub gaming_output: Option<String>,
    pub async_outputs: Vec<String>,
    pub wallpaper_directory: String,
    pub idle: crate::session::IdleConfig,
    pub keyboard: crate::keyboard::KeyboardConfig,
    pub terminal: Vec<String>,
    pub launcher: Vec<String>,
    pub program_bindings: BTreeMap<String, ProgramBinding>,
    pub bindings: Bindings,
    pub appearance: crate::appearance::Appearance,
    pub float_dialogs: bool,
    pub game_app_id_prefixes: Vec<String>,
    pub vrr: crate::vrr::VrrConfig,
    pub rules: Vec<crate::rules::Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspace_outputs: Vec::new(),
            async_outputs: Vec::new(),
            wallpaper_directory: std::env::var("HOME")
                .map(|home| format!("{home}/Bilder/Wallpaper"))
                .unwrap_or_else(|_| "/usr/share/backgrounds".into()),
            idle: crate::session::IdleConfig::default(),
            keyboard: crate::keyboard::KeyboardConfig::default(),
            terminal: vec!["kitty".into()],
            program_bindings: BTreeMap::new(),
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
            gaming_output: None,
            game_app_id_prefixes: Vec::new(),
            vrr: crate::vrr::VrrConfig::default(),
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramBinding {
    pub keys: Vec<String>,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    reload: Vec<String>,
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
    workspace_previous: Vec<String>,
    workspace_next: Vec<String>,
    move_to_workspace_previous: Vec<String>,
    move_to_workspace_next: Vec<String>,
    new_workspace: Vec<String>,
    move_to_new_workspace: Vec<String>,
    toggle_floating: Vec<String>,
    toggle_scratchpad: Vec<String>,
    move_to_scratchpad: Vec<String>,
    focus_output_left: Vec<String>,
    focus_output_right: Vec<String>,
    focus_output_up: Vec<String>,
    focus_output_down: Vec<String>,
    move_to_output_left: Vec<String>,
    move_to_output_right: Vec<String>,
    move_to_output_up: Vec<String>,
    move_to_output_down: Vec<String>,
    pointer_modifiers: String,
    workspace_modifiers: String,
    move_to_workspace_modifiers: String,
}

impl Default for Bindings {
    fn default() -> Self {
        let keys = |values: &[&str]| values.iter().map(|v| (*v).into()).collect();
        Self {
            reload: keys(&["Super+Shift+r"]),
            wallpaper: keys(&["Super+Shift+w"]),
            lock: keys(&["Super+Escape"]),
            toggle_floating: keys(&["Super+v"]),
            toggle_scratchpad: keys(&["Super+grave"]),
            move_to_scratchpad: keys(&["Super+Shift+grave"]),
            focus_output_left: keys(&["Super+Alt+Left"]),
            focus_output_right: keys(&["Super+Alt+Right"]),
            focus_output_up: keys(&["Super+Alt+Up"]),
            focus_output_down: keys(&["Super+Alt+Down"]),
            move_to_output_left: keys(&["Super+Shift+Left"]),
            move_to_output_right: keys(&["Super+Shift+Right"]),
            move_to_output_up: keys(&["Super+Shift+Up"]),
            move_to_output_down: keys(&["Super+Shift+Down"]),
            pointer_modifiers: "Super".into(),
            terminal: keys(&["Super+Return"]),
            launcher: keys(&["Super+Space"]),
            close: keys(&["Super+q"]),
            exit: keys(&["Super+m"]),
            focus_left: keys(&["Super+h", "Super+Left"]),
            focus_right: keys(&["Super+l", "Super+Right"]),
            move_left: keys(&["Super+Shift+h"]),
            move_right: keys(&["Super+Shift+l"]),
            workspace_previous: keys(&["Super+Ctrl+Left", "Super+Ctrl+Up"]),
            workspace_next: keys(&["Super+Ctrl+Right", "Super+Ctrl+Down"]),
            move_to_workspace_previous: keys(&["Super+Ctrl+Shift+Up"]),
            move_to_workspace_next: keys(&["Super+Ctrl+Shift+Down"]),
            new_workspace: keys(&["Super+n"]),
            move_to_new_workspace: keys(&["Super+Shift+n"]),
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
        if let Ok(path) = crate::theme::state_path() {
            command.env("MYWM_THEME_STATE", path);
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        let mut seen = HashSet::new();
        for output in &config.workspace_outputs {
            if output.trim().is_empty() || !seen.insert(output) {
                return Err(
                    "workspace_outputs must list distinct, nonempty monitor names".into(),
                );
            }
        }
        if config.workspace_outputs.len() > MAX_HOME_WORKSPACES {
            return Err(format!(
                "workspace_outputs may list at most {MAX_HOME_WORKSPACES} monitors"
            )
            .into());
        }
        if config
            .gaming_output
            .as_ref()
            .is_some_and(|output| output.trim().is_empty())
        {
            return Err("gaming_output must not be empty".into());
        }
        if config
            .async_outputs
            .iter()
            .any(|output| output.trim().is_empty())
        {
            return Err("async_outputs must not contain empty output names".into());
        }
        if config
            .game_app_id_prefixes
            .iter()
            .any(|prefix| prefix.trim().is_empty())
        {
            return Err("game_app_id_prefixes must not contain empty values".into());
        }
        config.vrr.validate()?;
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
        for (name, binding) in &config.program_bindings {
            if name.trim().is_empty() {
                return Err("program_bindings names must not be empty".into());
            }
            if binding.keys.is_empty() {
                return Err(format!("program_bindings.{name}.keys must not be empty").into());
            }
            validate_command(
                &binding.command,
                &format!("program_bindings.{name}.command"),
            )?;
        }
        for (index, rule) in config.rules.iter().enumerate() {
            rule.validate()
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
            let mut config = Self::default();
            crate::theme::apply_to_appearance(&mut config.appearance);
            return Ok(config);
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut config =
                    Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                crate::theme::apply_to_appearance(&mut config.appearance);
                eprintln!("Configuration: {}", path.display());
                Ok(config)
            }
            Err(e) if explicit.is_none() && e.kind() == std::io::ErrorKind::NotFound => {
                let mut config = Self::default();
                crate::theme::apply_to_appearance(&mut config.appearance);
                Ok(config)
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
            (&self.bindings.reload, Action::Reload),
            (&self.bindings.wallpaper, Action::Wallpaper),
            (&self.bindings.lock, Action::Lock),
            (&self.bindings.terminal, Action::Terminal),
            (&self.bindings.launcher, Action::Launcher),
            (&self.bindings.close, Action::Close),
            (&self.bindings.exit, Action::Exit),
            (&self.bindings.toggle_floating, Action::ToggleFloating),
            (&self.bindings.toggle_scratchpad, Action::ToggleScratchpad),
            (&self.bindings.move_to_scratchpad, Action::MoveToScratchpad),
            (
                &self.bindings.focus_output_left,
                Action::FocusOutput(OutputDirection::Left),
            ),
            (
                &self.bindings.focus_output_right,
                Action::FocusOutput(OutputDirection::Right),
            ),
            (
                &self.bindings.focus_output_up,
                Action::FocusOutput(OutputDirection::Up),
            ),
            (
                &self.bindings.focus_output_down,
                Action::FocusOutput(OutputDirection::Down),
            ),
            (
                &self.bindings.move_to_output_left,
                Action::MoveToOutput(OutputDirection::Left),
            ),
            (
                &self.bindings.move_to_output_right,
                Action::MoveToOutput(OutputDirection::Right),
            ),
            (
                &self.bindings.move_to_output_up,
                Action::MoveToOutput(OutputDirection::Up),
            ),
            (
                &self.bindings.move_to_output_down,
                Action::MoveToOutput(OutputDirection::Down),
            ),
            (&self.bindings.focus_left, Action::Focus(-1)),
            (&self.bindings.focus_right, Action::Focus(1)),
            (&self.bindings.move_left, Action::Move(-1)),
            (&self.bindings.move_right, Action::Move(1)),
            (
                &self.bindings.workspace_previous,
                Action::WorkspaceRelative(-1),
            ),
            (&self.bindings.workspace_next, Action::WorkspaceRelative(1)),
            (&self.bindings.new_workspace, Action::NewWorkspace),
            (
                &self.bindings.move_to_new_workspace,
                Action::MoveToNewWorkspace,
            ),
            (
                &self.bindings.move_to_workspace_previous,
                Action::MoveToWorkspaceRelative(-1),
            ),
            (
                &self.bindings.move_to_workspace_next,
                Action::MoveToWorkspaceRelative(1),
            ),
        ] {
            for key in keys {
                add(key, action)?;
            }
        }
        for workspace in 1..=crate::workspaces::MAX_NUMBER {
            add(
                &format!("{}+{}", self.bindings.workspace_modifiers, workspace),
                Action::Workspace(workspace),
            )?;
            add(
                &format!(
                    "{}+{}",
                    self.bindings.move_to_workspace_modifiers, workspace
                ),
                Action::MoveToWorkspace(workspace),
            )?;
        }
        for (index, binding) in self.program_bindings.values().enumerate() {
            for key in &binding.keys {
                add(key, Action::Program(index))?;
            }
        }
        Ok(bindings)
    }

    pub fn apply_reloadable(&mut self, new: Self) {
        self.wallpaper_directory = new.wallpaper_directory;
        self.terminal = new.terminal;
        self.launcher = new.launcher;
        self.program_bindings = new.program_bindings;
        self.bindings = new.bindings;
        self.appearance = new.appearance;
        self.float_dialogs = new.float_dialogs;
        self.gaming_output = new.gaming_output;
        self.game_app_id_prefixes = new.game_app_id_prefixes;
        self.vrr = new.vrr;
        self.rules = new.rules;
    }
}

fn validate_command(command: &[String], name: &str) -> Result<()> {
    if command
        .first()
        .is_none_or(|program| program.trim().is_empty())
    {
        return Err(format!("{name} must contain a program").into());
    }
    Ok(())
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
        "grave" => 0x60,
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
    fn workspace_outputs_list_distinct_monitors() {
        let config = Config::parse("workspace_outputs = ['DP-3', 'HDMI-A-1']").unwrap();
        assert_eq!(config.workspace_outputs, ["DP-3", "HDMI-A-1"]);
        Config::parse("gaming_output = 'DP-3'").unwrap();
        for invalid in [
            "workspace_outputs = ['DP-1', 'DP-1']",
            "workspace_outputs = ['']",
            "workspace_outputs = ['a','b','c','d','e','f','g','h','i']",
            "gaming_output = ''",
        ] {
            assert!(Config::parse(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn defaults_and_partial_configuration() {
        let defaults = Config::parse("").unwrap();
        assert_eq!(defaults.keybindings().unwrap().len(), 50);
        assert!(defaults.program_bindings.is_empty());
        let config = Config::parse("terminal = ['kitty', '--single-instance']").unwrap();
        assert_eq!(config.terminal[1], "--single-instance");
        // The shipped example has every setting commented out; it must parse both
        // as-is (defaults) and with all example settings enabled.
        let example = include_str!("../config/mywm.toml");
        Config::parse(example).unwrap();
        let enabled = example
            .lines()
            .map(|line| match line.strip_prefix('#') {
                Some(rest) if rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '[') => {
                    rest
                }
                _ => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        Config::parse(&enabled).unwrap();
    }

    #[test]
    fn invalid_config_is_rejected_before_connecting_to_river() {
        for text in [
            "workspaces = 3",
            "gaming_workspace = 3",
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
            "workspace_outputs =",
            "[bindings]\npointer_modifiers = 'Bogus'",
            "[program_bindings.browser]\nkeys = []\ncommand = ['firefox']",
            "[program_bindings.browser]\nkeys = ['Super+b']\ncommand = []",
            "[program_bindings.browser]\nkeys = ['Super+q']\ncommand = ['firefox']",
            "game_app_id_prefixes = ['']",
            "[vrr]\nenabled = true",
            "[vrr]\nenabled = true\noutput = ''",
            "[vrr]\nenabled = true\noutput = 'DP-3'\ncommand = []",
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

    #[test]
    fn program_bindings_add_commands_and_detect_duplicates() {
        let config = Config::parse(
            "[program_bindings.browser]\nkeys = ['Super+b', 'Super+Shift+b']\ncommand = ['firefox', '--private-window']",
        )
        .unwrap();
        assert_eq!(config.keybindings().unwrap().len(), 52);
        let binding = config.program_bindings.get("browser").unwrap();
        assert_eq!(binding.command, ["firefox", "--private-window"]);
    }
}
