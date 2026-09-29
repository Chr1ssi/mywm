//! Keyboard and pointer binding events.
use crate::*;

impl Dispatch<RiverPointerBindingV1, PointerAction> for State {
    fn event(
        state: &mut State,
        _binding: &RiverPointerBindingV1,
        event: river::river_window_management::river_pointer_binding_v1::Event,
        action: &PointerAction,
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        if matches!(
            event,
            river::river_window_management::river_pointer_binding_v1::Event::Pressed
        ) && let Some(window) = &state.pointer_window
        {
            state.pending_drag = Some((window.clone(), *action));
        }
    }
}

impl Dispatch<RiverXkbBindingsV1, ()> for State {
    fn event(
        state: &mut State,
        _bindings: &RiverXkbBindingsV1,
        _event: river::river_xkb_bindings::river_xkb_bindings_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        state.xkb_bindings = Some(_bindings.clone());
    }

    // event_created_child! ...
}

impl Dispatch<RiverXkbBindingV1, Action> for State {
    fn event(
        state: &mut State,
        _binding: &RiverXkbBindingV1,
        event: river::river_xkb_bindings::river_xkb_binding_v1::Event,
        action: &Action,
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        if matches!(
            event,
            river::river_xkb_bindings::river_xkb_binding_v1::Event::Pressed
        ) {
            state.actions.push(*action);
        }
    }
}

impl Dispatch<RiverXkbBindingsSeatV1, ()> for State {
    fn event(
        _state: &mut State,
        _seat: &RiverXkbBindingsSeatV1,
        _event: river::river_xkb_bindings::river_xkb_bindings_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
    }
}
