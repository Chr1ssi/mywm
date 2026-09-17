use crate::{
    State,
    river::{
        river_input_management::{
            river_input_device_v1::{self, RiverInputDeviceV1},
            river_input_manager_v1::{self, RiverInputManagerV1},
        },
        river_xkb_config::{
            river_xkb_config_v1::{self, RiverXkbConfigV1},
            river_xkb_keyboard_v1::{self, RiverXkbKeyboardV1},
            river_xkb_keymap_v1::{self, RiverXkbKeymapV1},
        },
    },
};
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::{fd::AsFd, unix::fs::OpenOptionsExt},
    process::Command,
};
use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_registry::WlRegistry};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyboardConfig {
    pub layout: String,
    pub variant: String,
    pub options: String,
}
impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            layout: "de".into(),
            variant: String::new(),
            options: String::new(),
        }
    }
}

pub struct KeyboardState {
    file: File,
    keymap: Option<RiverXkbKeymapV1>,
    ready: bool,
    keyboards: Vec<RiverXkbKeyboardV1>,
}
impl KeyboardState {
    pub fn new(config: &KeyboardConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if config.layout.trim().is_empty() {
            return Err("keyboard.layout must not be empty".into());
        }
        let output = Command::new("xkbcli")
            .args([
                "compile-keymap",
                "--output-format",
                "1",
                "--layout",
                &config.layout,
                "--variant",
                &config.variant,
                "--options",
                &config.options,
            ])
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "Invalid keyboard configuration: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        // Unlink immediately: the descriptor survives until River has read the map.
        let path = std::env::temp_dir().join(format!("mywm-keymap-{}", std::process::id()));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        std::fs::remove_file(path)?;
        file.write_all(&output.stdout)?;
        file.write_all(&[0])?;
        Ok(Self {
            file,
            keymap: None,
            ready: false,
            keyboards: Vec::new(),
        })
    }
}

pub fn bind(
    state: &mut State,
    registry: &WlRegistry,
    name: u32,
    interface: &str,
    version: u32,
    qh: &QueueHandle<State>,
) {
    if interface == "river_input_manager_v1" {
        registry.bind::<RiverInputManagerV1, _, _>(name, version.min(2), qh, ());
    } else if interface == "river_xkb_config_v1" {
        let manager = registry.bind::<RiverXkbConfigV1, _, _>(name, version.min(2), qh, ());
        state.keyboard.keymap = Some(manager.create_keymap(
            state.keyboard.file.as_fd(),
            river_xkb_config_v1::KeymapFormat::TextV1,
            qh,
            (),
        ));
    }
}
impl Dispatch<RiverInputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        object: &RiverInputManagerV1,
        event: river_input_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let river_input_manager_v1::Event::Finished = event {
            object.destroy();
        }
    }
    wayland_client::event_created_child!(State, RiverInputManagerV1, [1 => (RiverInputDeviceV1, ())]);
}
impl Dispatch<RiverInputDeviceV1, ()> for State {
    fn event(
        _: &mut Self,
        object: &RiverInputDeviceV1,
        event: river_input_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let river_input_device_v1::Event::Removed = event {
            object.destroy();
        }
    }
}
impl Dispatch<RiverXkbConfigV1, ()> for State {
    fn event(
        state: &mut Self,
        object: &RiverXkbConfigV1,
        event: river_xkb_config_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            river_xkb_config_v1::Event::XkbKeyboard { id } => {
                if state.keyboard.ready {
                    id.set_keymap(state.keyboard.keymap.as_ref().unwrap());
                }
                state.keyboard.keyboards.push(id);
            }
            river_xkb_config_v1::Event::Finished => object.destroy(),
        }
    }
    wayland_client::event_created_child!(State, RiverXkbConfigV1, [1 => (RiverXkbKeyboardV1, ())]);
}
impl Dispatch<RiverXkbKeymapV1, ()> for State {
    fn event(
        state: &mut Self,
        object: &RiverXkbKeymapV1,
        event: river_xkb_keymap_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            river_xkb_keymap_v1::Event::Success => {
                state.keyboard.ready = true;
                for keyboard in &state.keyboard.keyboards {
                    keyboard.set_keymap(object);
                }
                println!("Keyboard layout: {}", state.config.keyboard.layout);
            }
            river_xkb_keymap_v1::Event::Failure { error_msg } => {
                eprintln!("River rejected keyboard keymap: {error_msg}")
            }
        }
    }
}
impl Dispatch<RiverXkbKeyboardV1, ()> for State {
    fn event(
        state: &mut Self,
        object: &RiverXkbKeyboardV1,
        event: river_xkb_keyboard_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let river_xkb_keyboard_v1::Event::Removed = event {
            state.keyboard.keyboards.retain(|k| k != object);
            object.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::FileExt;
    #[test]
    fn german_keymap_compiles_and_invalid_layout_fails() {
        let state = KeyboardState::new(&KeyboardConfig::default()).unwrap();
        let mut bytes = vec![0; state.file.metadata().unwrap().len() as usize];
        state.file.read_exact_at(&mut bytes, 0).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("German"));
        assert!(text.contains("odiaeresis"));
        assert!(text.contains("ssharp"));
        assert!(
            KeyboardState::new(&KeyboardConfig {
                layout: "mywm_invalid_layout".into(),
                ..KeyboardConfig::default()
            })
            .is_err()
        );
    }
}
