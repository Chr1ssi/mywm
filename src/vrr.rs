use crate::State;
use serde::Deserialize;
use wayland_client::Proxy;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VrrConfig {
    pub enabled: bool,
    pub output: String,
    pub command: Vec<String>,
}

impl Default for VrrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            output: String::new(),
            command: vec!["wlr-randr".into()],
        }
    }
}

impl VrrConfig {
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.enabled && self.output.trim().is_empty() {
            return Err("vrr.output must name an output when VRR is enabled".into());
        }
        if self.enabled
            && self
                .command
                .first()
                .is_none_or(|program| program.trim().is_empty())
        {
            return Err("vrr.command must contain a program when VRR is enabled".into());
        }
        Ok(())
    }
}

fn output_connected(state: &State) -> bool {
    state
        .wl_outputs
        .values()
        .any(|(_, name)| name.as_deref() == Some(state.config.vrr.output.as_str()))
}

fn desired(state: &State) -> bool {
    if !state.config.vrr.enabled || state.session_locked {
        return false;
    }
    state.windows.iter().any(|window| {
        window.fullscreen_output.is_some()
            && crate::is_game_window(state, &window.river_window.id())
            && window.output.is_some_and(|index| {
                state.outputs[index]
                    .wl_global
                    .and_then(|global| state.wl_outputs.get(&global))
                    .and_then(|(_, name)| name.as_deref())
                    == Some(state.config.vrr.output.as_str())
            })
    })
}

fn set(state: &mut State, enabled: bool) {
    let config = &state.config.vrr;
    let Some(program) = config.command.first() else {
        return;
    };
    let status = std::process::Command::new(program)
        .args(&config.command[1..])
        .args([
            "--output",
            &config.output,
            "--adaptive-sync",
            if enabled { "enabled" } else { "disabled" },
        ])
        .status();
    match status {
        Ok(status) if status.success() => {
            state.vrr_enabled = Some(enabled);
            eprintln!(
                "VRR {} on {}",
                if enabled { "enabled" } else { "disabled" },
                config.output
            );
        }
        Ok(status) => eprintln!("Cannot change VRR on {}: {status}", config.output),
        Err(error) => eprintln!("Cannot start VRR command '{program}': {error}"),
    }
}

pub fn disable(state: &mut State) {
    if state.vrr_enabled == Some(true) {
        set(state, false);
    }
}

/// Keep adaptive sync off on the desktop and enable it only while a known game
/// is visibly fullscreen on the configured output.
pub fn apply(state: &mut State) {
    let enabled = desired(state);
    if state.vrr_enabled == Some(enabled) {
        return;
    }
    if state.config.vrr.enabled && !output_connected(state) {
        return;
    }
    if !state.config.vrr.enabled && state.vrr_enabled.is_none() {
        state.vrr_enabled = Some(false);
        return;
    }
    set(state, enabled);
}
