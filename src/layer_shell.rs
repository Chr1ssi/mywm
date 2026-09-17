use crate::river::river_layer_shell::{
    river_layer_shell_output_v1::{self, RiverLayerShellOutputV1},
    river_layer_shell_seat_v1::{self, RiverLayerShellSeatV1},
    river_layer_shell_v1::RiverLayerShellV1,
};
use crate::{Output, State, floating::Rect};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, backend::ObjectId};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum LayerFocus {
    #[default]
    None,
    Exclusive,
    NonExclusive,
}

impl Output {
    pub fn work_area(&self) -> Option<Rect> {
        let (x, y) = self.position?;
        let (width, height) = self.dimensions?;
        usable_area(
            Rect {
                x,
                y,
                width,
                height,
            },
            self.non_exclusive_area,
        )
    }
}

/// Layer-shell coordinates are global, including on rotated/offset monitors.
fn usable_area(output: Rect, reserved: Option<Rect>) -> Option<Rect> {
    let hint = reserved.unwrap_or(output);
    let x = output.x.max(hint.x);
    let y = output.y.max(hint.y);
    let right = output
        .x
        .saturating_add(output.width)
        .min(hint.x.saturating_add(hint.width));
    let bottom = output
        .y
        .saturating_add(output.height)
        .min(hint.y.saturating_add(hint.height));
    (right > x && bottom > y).then_some(Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    })
}

pub fn setup(state: &mut State, qh: &QueueHandle<State>) {
    let Some(layer_shell) = &state.layer_shell else {
        return;
    };
    for output in &mut state.outputs {
        if output.layer_output.is_none() {
            output.layer_output =
                Some(layer_shell.get_output(&output.river_output, qh, output.river_output.id()));
        }
    }
    if state.layer_seat.is_none()
        && let Some(seat) = &state.seat
    {
        state.layer_seat = Some(layer_shell.get_seat(seat, qh, ()));
    }
}

wayland_client::delegate_noop!(State: ignore RiverLayerShellV1);

impl Dispatch<RiverLayerShellOutputV1, ObjectId> for State {
    fn event(
        state: &mut Self,
        _object: &RiverLayerShellOutputV1,
        event: river_layer_shell_output_v1::Event,
        output_id: &ObjectId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let river_layer_shell_output_v1::Event::NonExclusiveArea {
            x,
            y,
            width,
            height,
        } = event
            && let Some(output) = state
                .outputs
                .iter_mut()
                .find(|o| o.river_output.id() == *output_id)
        {
            let area = Some(Rect {
                x,
                y,
                width,
                height,
            });
            if output.non_exclusive_area != area {
                output.non_exclusive_area = area;
                if state.layer_focus == LayerFocus::None {
                    state.focus_dirty = true;
                }
                // Stop a drag based on a now outdated coordinate origin.
                crate::cancel_drag(state);
            }
        }
    }
}

impl Dispatch<RiverLayerShellSeatV1, ()> for State {
    fn event(
        state: &mut Self,
        object: &RiverLayerShellSeatV1,
        event: river_layer_shell_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if state.layer_seat.as_ref() != Some(object) {
            return;
        }
        use river_layer_shell_seat_v1::Event;
        match event {
            Event::FocusExclusive => {
                state.layer_focus = LayerFocus::Exclusive;
                state.layer_focus_granted = true;
                crate::cancel_drag(state);
            }
            Event::FocusNonExclusive => {
                state.layer_focus = LayerFocus::NonExclusive;
                state.layer_focus_granted = true;
                crate::cancel_drag(state);
            }
            Event::FocusNone => {
                state.layer_focus = LayerFocus::None;
                state.layer_focus_granted = false;
                state.focus_dirty = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn respects_global_work_area_on_offset_and_portrait_outputs() {
        let output = Rect {
            x: 2560,
            y: -1080,
            width: 1440,
            height: 2560,
        };
        assert_eq!(usable_area(output, None), Some(output));
        let hint = Rect {
            x: 2560,
            y: -1040,
            width: 1440,
            height: 2520,
        };
        assert_eq!(usable_area(output, Some(hint)), Some(hint));
        assert_eq!(
            usable_area(
                output,
                Some(Rect {
                    x: 0,
                    y: -1080,
                    width: 9999,
                    height: 9999
                })
            ),
            Some(output)
        );
    }
    #[test]
    fn fully_reserved_outputs_have_no_window_area() {
        let output = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            usable_area(
                output,
                Some(Rect {
                    height: 0,
                    ..output
                })
            ),
            None
        );
        assert_eq!(usable_area(output, Some(Rect { x: 3000, ..output })), None);
    }
}
