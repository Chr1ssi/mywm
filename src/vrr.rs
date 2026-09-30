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

fn output_name(state: &State, index: usize) -> Option<&str> {
    state.outputs[index]
        .wl_global
        .and_then(|global| state.wl_outputs.get(&global))
        .and_then(|(_, name)| name.as_deref())
}

fn game_fullscreen_on(state: &State, output: usize) -> bool {
    !state.session_locked
        && state.windows.iter().any(|window| {
            window.fullscreen_output.is_some()
                && window.output == Some(output)
                && crate::is_game_window(state, &window.river_window.id())
        })
}

/// Tearing (async presentation) is allowed on the configured outputs only
/// while a game is visibly fullscreen there; everything else stays on vsync.
pub fn apply_presentation(state: &mut State) {
    use crate::river::river_window_management::river_output_v1::PresentationMode;
    let wanted: Vec<Option<bool>> = (0..state.outputs.len())
        .map(|index| {
            let name = output_name(state, index)?;
            let allowed = state.config.async_outputs.iter().any(|o| o == name);
            Some(allowed && game_fullscreen_on(state, index))
        })
        .collect();
    for (output, wanted) in state.outputs.iter_mut().zip(wanted) {
        let Some(wanted) = wanted else { continue };
        if output.presentation_async == Some(wanted) || output.river_output.version() < 4 {
            continue;
        }
        output.river_output.set_presentation_mode(if wanted {
            PresentationMode::Async
        } else {
            PresentationMode::Vsync
        });
        output.presentation_async = Some(wanted);
    }
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

pub fn worker(config: &VrrConfig) -> Option<std::sync::mpsc::Sender<bool>> {
    if !config.enabled {
        return None;
    }
    let command = config.command.clone();
    let output = config.output.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(enabled) = receiver.recv() {
            let status = std::process::Command::new(&command[0])
                .args(&command[1..])
                .args([
                    "--output",
                    &output,
                    "--adaptive-sync",
                    if enabled { "enabled" } else { "disabled" },
                ])
                .status();
            match status {
                Ok(status) if status.success() => eprintln!(
                    "VRR {} on {output}",
                    if enabled { "enabled" } else { "disabled" }
                ),
                Ok(status) => eprintln!("Cannot change VRR on {output}: {status}"),
                Err(error) => eprintln!("Cannot start VRR command '{}': {error}", command[0]),
            }
        }
    });
    Some(sender)
}

fn set(state: &mut State, enabled: bool) {
    let Some(sender) = &state.vrr_sender else {
        return;
    };
    if sender.send(enabled).is_ok() {
        state.vrr_enabled = Some(enabled);
    } else {
        eprintln!("VRR worker stopped unexpectedly");
        state.vrr_sender = None;
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
    if state.vrr_enabled.is_none() && !enabled {
        state.vrr_enabled = Some(false);
        return;
    }
    set(state, enabled);
}

#[cfg(test)]
mod tests {
    use super::VrrConfig;

    #[test]
    fn disabled_config_needs_no_output() {
        assert!(VrrConfig::default().validate().is_ok());
    }

    #[test]
    fn enabled_config_requires_output_and_command() {
        let mut config = VrrConfig {
            enabled: true,
            ..VrrConfig::default()
        };
        assert!(config.validate().is_err());

        config.output = "DP-1".into();
        assert!(config.validate().is_ok());

        config.command = vec![" ".into()];
        assert!(config.validate().is_err());
        config.command.clear();
        assert!(config.validate().is_err());
    }
}
