//! River seat events (focus, pointer, layer shell).
use crate::*;

impl Dispatch<RiverSeatV1, ()> for State {
    fn event(
        state: &mut Self,
        seat: &RiverSeatV1,
        event: river::river_window_management::river_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if matches!(
            event,
            river::river_window_management::river_seat_v1::Event::Removed
        ) {
            if state.seat.as_ref() == Some(seat) {
                for binding in state.bindings.drain(..) {
                    binding.destroy();
                }
                for binding in state.pointer_bindings.drain(..) {
                    binding.destroy();
                }
                cancel_drag(state);
                if let Some(layer_seat) = state.layer_seat.take() {
                    layer_seat.destroy();
                }
                state.layer_focus = LayerFocus::None;
                state.layer_focus_granted = false;
                state.seat = None;
                state.pointer_position = None;
                state.pointer_window = None;
            }
            seat.destroy();
            return;
        }
        if state.seat.as_ref().is_some_and(|active| active != seat) {
            return;
        }
        state.seat = Some(seat.clone());
        layer_shell::setup(state, qh);

        match event {
            river::river_window_management::river_seat_v1::Event::PointerEnter { window } => {
                state.pointer_window = Some(window.id());
            }
            river::river_window_management::river_seat_v1::Event::PointerLeave => {
                state.pointer_window = None;
            }
            river::river_window_management::river_seat_v1::Event::OpDelta { dx, dy } => {
                if state.drag.is_some() {
                    state.drag_delta = Some((dx, dy));
                }
            }
            river::river_window_management::river_seat_v1::Event::OpRelease => {
                state.end_drag = true;
            }
            river::river_window_management::river_seat_v1::Event::PointerPosition { x, y } => {
                state.pointer_position = Some((x, y));

                update_focused_output(state, x, y);
            }

            river::river_window_management::river_seat_v1::Event::WindowInteraction { window } => {
                let Some(window_index) = state
                    .windows
                    .iter()
                    .position(|item| item.river_window.id() == window.id())
                else {
                    println!("Window interaction for unknown window");
                    return;
                };

                if state.scratchpad_visible && state.scratchpad.windows.contains(&window.id()) {
                    state.scratchpad.focused = Some(window.id());
                    focus_scratchpad(state);
                    return;
                }

                if let Some(output) = state.windows[window_index].output
                    && state.outputs[output].workspaces.focus(&window.id())
                {
                    focus_output(state, output);
                }
            }

            _ => {}
        }
    }
}
