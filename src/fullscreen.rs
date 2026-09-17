use crate::State;
use wayland_client::Proxy;

/// Preserve the normal layout underneath fullscreen, including floating geometry.
pub fn apply(state: &mut State) {
    for window in &mut state.windows {
        let output = window.output.and_then(|index| state.outputs.get(index));
        let target = output.filter(|output| {
            window.fullscreen
                && output.workspaces.current().focused.as_ref() == Some(&window.river_window.id())
                && output.position.is_some()
                && output.dimensions.is_some()
        });
        let target_id = target.map(|output| output.river_output.id());
        if window.fullscreen {
            window.river_window.inform_fullscreen();
        } else {
            window.river_window.inform_not_fullscreen();
        }
        if window.fullscreen_output != target_id {
            if let Some(output) = target {
                window.river_window.fullscreen(&output.river_output);
            } else if window.fullscreen_output.is_some() {
                window.river_window.exit_fullscreen();
            }
            window.fullscreen_output = target_id;
        }
        if let Some(output) = target {
            let (x, y) = output.position.unwrap();
            let (width, height) = output.dimensions.unwrap();
            window.geometry = Some((x, y, width, height));
        }
    }
}
