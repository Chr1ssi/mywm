//! River output events.
use crate::*;

impl Dispatch<RiverOutputV1, ()> for State {
    fn event(
        state: &mut Self,
        output: &RiverOutputV1,
        event: river::river_window_management::river_output_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if matches!(
            event,
            river::river_window_management::river_output_v1::Event::Removed
        ) {
            if let Some(index) = state.outputs.iter().position(|o| o.river_output == *output) {
                cancel_drag(state);
                let removed = state.outputs.remove(index);
                if let Some(layer_output) = removed.layer_output {
                    layer_output.destroy();
                }
                let fallback = (!state.outputs.is_empty()).then_some(0);
                let focus_was_removed = state.windows.iter().any(|w| {
                    w.output == Some(index) && Some(w.river_window.id()) == state.focused_window
                });
                for window in &mut state.windows {
                    window.output = if window.output == Some(index) {
                        fallback
                    } else {
                        window
                            .output
                            .and_then(|i| scrolling::index_after_remove(i, index))
                    };
                }
                state.focused_output = state
                    .focused_output
                    .and_then(|i| scrolling::index_after_remove(i, index))
                    .or(fallback);
                if let Some(target) = fallback {
                    state.outputs[target].workspaces.absorb(removed.workspaces);
                    if focus_was_removed {
                        focus_output(state, target);
                    }
                } else {
                    state.detached_workspaces = Some(removed.workspaces);
                    state.focused_window = None;
                    state.focus_dirty = true;
                }
            }
            output.destroy();
            return;
        }
        if state.drag.is_some() {
            cancel_drag(state);
        }
        let Some(state_output) = state
            .outputs
            .iter_mut()
            .find(|item| item.river_output.id() == output.id())
        else {
            return;
        };

        match event {
            river::river_window_management::river_output_v1::Event::WlOutput { name } => {
                state_output.wl_global = Some(name);
            }
            river::river_window_management::river_output_v1::Event::Position { x, y } => {
                state_output.position = Some((x, y));

                println!("Output {} position: ({x}, {y})", output.id());
            }

            river::river_window_management::river_output_v1::Event::Dimensions {
                width,
                height,
            } => {
                state_output.dimensions = Some((width, height));

                println!("Output {} dimensions: {width}x{height}", output.id());
            }

            _ => {}
        }
    }
}
