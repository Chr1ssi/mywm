mod config;
mod floating;
mod river;
mod rules;
mod scrolling;
mod workspaces;

use river::river_window_management::{
    river_node_v1::RiverNodeV1, river_output_v1::RiverOutputV1, river_seat_v1::RiverSeatV1,
    river_window_manager_v1::RiverWindowManagerV1, river_window_v1::RiverWindowV1,
};

use config::Config;
use floating::{DragKind, Rect};
use river::river_window_management::{
    river_pointer_binding_v1::RiverPointerBindingV1, river_window_v1::Edges,
};
use river::river_xkb_bindings::river_xkb_binding_v1::RiverXkbBindingV1;
use river::river_xkb_bindings::river_xkb_bindings_seat_v1::RiverXkbBindingsSeatV1;
use river::river_xkb_bindings::river_xkb_bindings_v1::RiverXkbBindingsV1;
use wayland_client::backend::ObjectId;
use workspaces::Workspaces;

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, protocol::wl_registry};

struct Window {
    river_window: RiverWindowV1,
    node: Option<RiverNodeV1>,
    geometry: Option<(i32, i32, i32, i32)>,
    output: Option<usize>,
    floating: bool,
    floating_rect: Option<Rect>,
    app_id: Option<String>,
    parent: Option<ObjectId>,
}

struct Output {
    river_output: RiverOutputV1,
    position: Option<(i32, i32)>,
    dimensions: Option<(i32, i32)>,
    workspaces: Workspaces<ObjectId>,
}

struct State {
    windows: Vec<Window>,
    outputs: Vec<Output>,
    focused_output: Option<usize>,
    pointer_position: Option<(i32, i32)>,
    pointer_window: Option<ObjectId>,
    pointer_bindings: Vec<RiverPointerBindingV1>,
    pending_drag: Option<(ObjectId, PointerAction)>,
    drag: Option<Drag>,
    drag_delta: Option<(i32, i32)>,
    end_drag: bool,
    seat: Option<RiverSeatV1>,
    focused_window: Option<ObjectId>,
    focus_dirty: bool,
    config: Config,
    detached_workspaces: Option<Workspaces<ObjectId>>,

    bindings: Vec<RiverXkbBindingV1>,
    actions: Vec<Action>,
    exit_requested: bool,
    xkb_bindings: Option<RiverXkbBindingsV1>,
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Terminal,
    Exit,
    Focus(isize),
    Move(isize),
    Close,
    ToggleFloating,
    Workspace(usize),
    MoveToWorkspace(usize),
}

#[derive(Clone, Copy, Debug)]
enum PointerAction {
    Move,
    Resize,
    ResizeEdges(Edges),
}

struct Drag {
    window: ObjectId,
    initial: Rect,
    kind: DragKind,
}

fn cancel_drag(state: &mut State) {
    state.pending_drag = None;
    state.end_drag = true;
}

fn update_drag(state: &mut State) {
    if let Some(drag) = &state.drag {
        if let Some(window) = state
            .windows
            .iter_mut()
            .find(|w| w.river_window.id() == drag.window)
            && let Some(output) = window.output.and_then(|i| state.outputs.get(i))
            && output.workspaces.current().windows.contains(&drag.window)
            && window.floating
            && let Some((width, height)) = output.dimensions
        {
            if let Some((dx, dy)) = state.drag_delta.take() {
                window.floating_rect = Some(drag.initial.dragged(drag.kind, dx, dy, width, height));
            }
        } else {
            state.end_drag = true;
        }
    }
    if state.end_drag {
        if let Some(drag) = state.drag.take() {
            if let Some(seat) = &state.seat {
                seat.op_end();
            }
            if matches!(drag.kind, DragKind::Resize(_))
                && let Some(window) = state
                    .windows
                    .iter()
                    .find(|w| w.river_window.id() == drag.window)
            {
                window.river_window.inform_resize_end();
            }
        }
        state.drag_delta = None;
        state.end_drag = false;
    }
    let Some((id, action)) = state.pending_drag.take() else {
        return;
    };
    if state.drag.is_some() || state.seat.is_none() {
        return;
    }
    let Some(index) = state
        .windows
        .iter()
        .position(|w| w.river_window.id() == id && w.floating)
    else {
        return;
    };
    let Some(output_index) = state.windows[index].output else {
        return;
    };
    let output = &state.outputs[output_index];
    if !output.workspaces.current().windows.contains(&id) {
        return;
    }
    let (Some((ox, oy)), Some((width, height))) = (output.position, output.dimensions) else {
        return;
    };
    let initial = state.windows[index]
        .floating_rect
        .unwrap_or_else(|| Rect::centered(width, height))
        .constrained(width, height);
    let kind = match action {
        PointerAction::Move => DragKind::Move,
        PointerAction::ResizeEdges(edges) => DragKind::Resize(edges),
        PointerAction::Resize => {
            let (x, y) = state.pointer_position.unwrap_or((
                ox + initial.x + initial.width,
                oy + initial.y + initial.height,
            ));
            let horizontal = if x - ox < initial.x + initial.width / 2 {
                Edges::Left
            } else {
                Edges::Right
            };
            let vertical = if y - oy < initial.y + initial.height / 2 {
                Edges::Top
            } else {
                Edges::Bottom
            };
            DragKind::Resize(horizontal | vertical)
        }
    };
    state.outputs[output_index].workspaces.focus(&id);
    focus_output(state, output_index);
    state.seat.as_ref().unwrap().op_start_pointer();
    if matches!(kind, DragKind::Resize(_)) {
        state
            .windows
            .iter()
            .find(|w| w.river_window.id() == id)
            .unwrap()
            .river_window
            .inform_resize_start();
    }
    state.drag = Some(Drag {
        window: id,
        initial,
        kind,
    });
}

fn assign_windows(state: &mut State, default_output: usize) {
    let mut focus = None;
    // A child may be announced before its parent. Assign parents first so dialog
    // placement also inherits any workspace rule applied to the parent.
    loop {
        let mut assigned = false;
        for index in 0..state.windows.len() {
            if state.windows[index].output.is_some() {
                continue;
            }
            let window = &state.windows[index];
            let parent = window
                .parent
                .as_ref()
                .and_then(|id| state.windows.iter().find(|w| w.river_window.id() == *id));
            let inherited = if let Some(parent) = parent {
                let Some(output) = parent.output else {
                    continue;
                };
                let workspace = state.outputs[output]
                    .workspaces
                    .location(&parent.river_window.id())
                    .unwrap_or(state.outputs[output].workspaces.active);
                Some((output, workspace))
            } else {
                None
            };
            let placement = rules::resolve(
                &state.config.rules,
                window.app_id.as_deref(),
                window.parent.is_some(),
                state.config.float_dialogs,
            );
            let (output, workspace) = inherited.unwrap_or((
                default_output,
                state.outputs[default_output].workspaces.active,
            ));
            let workspace = placement.workspace.unwrap_or(workspace);
            let id = window.river_window.id();
            state.windows[index].output = Some(output);
            state.windows[index].floating = placement.floating;
            if workspace == state.outputs[output].workspaces.active {
                state.outputs[output].workspaces.add(id);
                focus = Some(output);
            } else {
                state.outputs[output].workspaces.add_to(workspace, id);
            }
            assigned = true;
        }
        if !assigned {
            break;
        }
    }
    if let Some(output) = focus {
        focus_output(state, output);
    }
}

fn focus_output(state: &mut State, output: usize) {
    state.focused_output = Some(output);
    state.focused_window = state.outputs[output].workspaces.current().focused.clone();
    state.focus_dirty = true;
    // Global storage order doubles as floating stacking order, independent of columns.
    if let Some(index) = state
        .windows
        .iter()
        .position(|w| w.floating && Some(w.river_window.id()) == state.focused_window)
    {
        let window = state.windows.remove(index);
        state.windows.push(window);
    }
}

fn layout(state: &mut State) {
    for window in &mut state.windows {
        window.geometry = None;
    }
    for output in &mut state.outputs {
        let (Some((x, y)), Some((width, height))) = (output.position, output.dimensions) else {
            continue;
        };
        let workspace = output.workspaces.current_mut();
        let tiled: Vec<_> = workspace
            .windows
            .iter()
            .filter(|id| {
                state
                    .windows
                    .iter()
                    .any(|w| w.river_window.id() == **id && !w.floating)
            })
            .collect();
        // Keep a focused dialog's tiled ancestor visible behind it.
        let mut scroll_focus = workspace.focused.clone();
        let mut focused = None;
        for _ in 0..=state.windows.len() {
            focused = tiled
                .iter()
                .position(|id| Some(*id) == scroll_focus.as_ref());
            if focused.is_some() || scroll_focus.is_none() {
                break;
            }
            scroll_focus = state
                .windows
                .iter()
                .find(|w| Some(w.river_window.id()) == scroll_focus)
                .and_then(|w| w.parent.clone());
        }
        let (scroll, columns) = scrolling::columns(width, tiled.len(), focused, workspace.scroll);
        workspace.scroll = scroll;
        for (id, (left, column_width)) in tiled.into_iter().zip(columns) {
            if let Some(window) = state
                .windows
                .iter_mut()
                .find(|w| w.river_window.id() == *id)
            {
                window.geometry = Some((x + left, y, column_width, height));
            }
        }
        for window in state
            .windows
            .iter_mut()
            .filter(|w| w.floating && workspace.windows.contains(&w.river_window.id()))
        {
            let rect = window
                .floating_rect
                .unwrap_or_else(|| Rect::centered(width, height))
                .constrained(width, height);
            window.floating_rect = Some(rect);
            window.geometry = Some((x + rect.x, y + rect.y, rect.width, rect.height));
        }
    }
}

fn run_actions(state: &mut State, manager: &RiverWindowManagerV1) {
    for action in std::mem::take(&mut state.actions) {
        match action {
            Action::Exit => {
                if manager.version() >= 4 {
                    state.exit_requested = true;
                    manager.exit_session();
                    return;
                }
                eprintln!(
                    "Exiting the session requires river-window-management-v1 version 4 or newer"
                );
            }
            Action::Terminal => {
                match std::process::Command::new(&state.config.terminal[0])
                    .args(&state.config.terminal[1..])
                    .spawn()
                {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(error) => eprintln!("Cannot start terminal: {error}"),
                }
            }
            Action::Workspace(target) => {
                cancel_drag(state);
                if let Some(output) = output_at_pointer(state).or(state.focused_output) {
                    state.outputs[output].workspaces.select(target);
                    focus_output(state, output);
                }
            }
            Action::ToggleFloating => {
                cancel_drag(state);
                if let Some(window) = state
                    .windows
                    .iter_mut()
                    .find(|w| Some(w.river_window.id()) == state.focused_window)
                {
                    window.floating = !window.floating;
                    if let Some(output) = window.output {
                        focus_output(state, output);
                    }
                }
            }
            Action::Close | Action::Focus(_) | Action::Move(_) | Action::MoveToWorkspace(_) => {
                let Some(window) = state
                    .windows
                    .iter()
                    .find(|w| Some(w.river_window.id()) == state.focused_window)
                else {
                    continue;
                };
                let Some(output) = window.output else {
                    continue;
                };
                match action {
                    Action::Close => window.river_window.close(),
                    Action::Focus(direction) | Action::Move(direction) => {
                        if window.floating && matches!(action, Action::Move(_)) {
                            continue;
                        }
                        if matches!(action, Action::Move(_)) {
                            let tiled: Vec<_> = state
                                .windows
                                .iter()
                                .filter(|w| !w.floating)
                                .map(|w| w.river_window.id())
                                .collect();
                            state.outputs[output].workspaces.navigate_matching(
                                direction,
                                true,
                                |id| tiled.contains(id),
                            );
                        } else {
                            state.outputs[output].workspaces.navigate(direction, false);
                        }
                        focus_output(state, output);
                    }
                    Action::MoveToWorkspace(target) => {
                        cancel_drag(state);
                        state.outputs[output].workspaces.move_focused_to(target);
                        focus_output(state, output);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

fn install_bindings(state: &mut State, qh: &QueueHandle<State>) {
    let Some(seat) = &state.seat else {
        return;
    };
    if state.pointer_bindings.is_empty() {
        let modifiers = state
            .config
            .pointer_modifiers()
            .expect("validated pointer modifiers");
        for (button, action) in [(0x110, PointerAction::Move), (0x111, PointerAction::Resize)] {
            let binding = seat.get_pointer_binding(button, modifiers, qh, action);
            binding.enable();
            state.pointer_bindings.push(binding);
        }
    }
    if !state.bindings.is_empty() {
        return;
    }
    let Some(bindings) = &state.xkb_bindings else {
        return;
    };
    for (key, modifiers, action) in state.config.keybindings().expect("validated configuration") {
        let binding = bindings.get_xkb_binding(seat, key, modifiers, qh, action);
        binding.enable();
        state.bindings.push(binding);
    }
}

fn update_focused_output(state: &mut State, pointer_x: i32, pointer_y: i32) {
    let new_focused_output = state.outputs.iter().position(|output| {
        let Some((output_x, output_y)) = output.position else {
            return false;
        };

        let Some((output_width, output_height)) = output.dimensions else {
            return false;
        };

        pointer_x >= output_x
            && pointer_x < output_x + output_width
            && pointer_y >= output_y
            && pointer_y < output_y + output_height
    });

    if state.focused_output != new_focused_output {
        state.focused_output = new_focused_output;

        if let Some(index) = new_focused_output {
            println!(
                "Focused output changed: {} (pointer: {}, {})",
                index, pointer_x, pointer_y
            );
        }
    }
}

fn output_at_pointer(state: &State) -> Option<usize> {
    let (pointer_x, pointer_y) = state.pointer_position?;

    state.outputs.iter().position(|output| {
        let Some((output_x, output_y)) = output.position else {
            return false;
        };

        let Some((output_width, output_height)) = output.dimensions else {
            return false;
        };

        pointer_x >= output_x
            && pointer_x < output_x + output_width
            && pointer_y >= output_y
            && pointer_y < output_y + output_height
    })
}

fn remove_window(state: &mut State, window_id: wayland_client::backend::ObjectId) {
    let Some(closed_index) = state
        .windows
        .iter()
        .position(|window| window.river_window.id() == window_id)
    else {
        return;
    };

    if state.pointer_window.as_ref() == Some(&window_id) {
        state.pointer_window = None;
    }
    if state.drag.as_ref().is_some_and(|d| d.window == window_id) {
        cancel_drag(state);
    }
    let removed = state.windows.remove(closed_index);
    if let Some(node) = removed.node {
        node.destroy();
    }
    removed.river_window.destroy();
    for output in &mut state.outputs {
        output.workspaces.remove(&window_id);
    }
    if let Some(workspaces) = &mut state.detached_workspaces {
        workspaces.remove(&window_id);
    }
    if state.focused_window.as_ref() == Some(&window_id) {
        if let Some(output) = removed.output {
            focus_output(state, output);
        } else {
            state.focused_window = None;
            state.focus_dirty = true;
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "river_window_manager_v1" {
                println!("Binding river_window_manager_v1 v{version}");

                registry.bind::<RiverWindowManagerV1, _, _>(name, version.min(5), qh, ());
            }

            if interface == "river_xkb_bindings_v1" {
                println!("Binding river_xkb_bindings_v1 v{version}");

                let bindings =
                    registry.bind::<RiverXkbBindingsV1, _, _>(name, version.min(3), qh, ());

                state.xkb_bindings = Some(bindings);
            }
        }
    }
}

impl Dispatch<RiverWindowManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        manager: &RiverWindowManagerV1,
        event: river::river_window_management::river_window_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            river::river_window_management::river_window_manager_v1::Event::Window { id } => {
                state.windows.push(Window {
                    river_window: id,
                    node: None,
                    geometry: None,
                    output: None,
                    floating: false,
                    floating_rect: None,
                    app_id: None,
                    parent: None,
                });
            }

            river::river_window_management::river_window_manager_v1::Event::Output { id } => {
                state.outputs.push(Output {
                    river_output: id,
                    position: None,
                    dimensions: None,
                    workspaces: Workspaces::new(state.config.workspaces),
                });

                if let Some(workspaces) = state.detached_workspaces.take() {
                    let output_index = state.outputs.len() - 1;
                    for window in &mut state.windows {
                        if workspaces
                            .entries
                            .iter()
                            .any(|ws| ws.windows.contains(&window.river_window.id()))
                        {
                            window.output = Some(output_index);
                        }
                    }
                    state.outputs[output_index].workspaces = workspaces;
                    focus_output(state, output_index);
                } else if state.focused_output.is_none() {
                    state.focused_output = Some(0);
                }
            }

            river::river_window_management::river_window_manager_v1::Event::ManageStart => {
                let current_output = output_at_pointer(state)
                    .or(state.focused_output)
                    .or_else(|| (!state.outputs.is_empty()).then_some(0));

                if let Some(output) = current_output {
                    assign_windows(state, output);
                }
                run_actions(state, manager);
                if state.exit_requested {
                    manager.manage_finish();
                    return;
                }
                update_drag(state);
                layout(state);
                for window in &mut state.windows {
                    if window.node.is_none() {
                        let node = window.river_window.get_node(qh, ());
                        window.node = Some(node);
                    }

                    if let Some((_, _, width, height)) = window.geometry {
                        window.river_window.set_tiled(if window.floating {
                            Edges::empty()
                        } else {
                            Edges::Top | Edges::Bottom | Edges::Left | Edges::Right
                        });
                        window.river_window.propose_dimensions(width, height);
                    }
                }

                if state.focus_dirty
                    && let Some(seat) = &state.seat
                {
                    if let Some(window) = state
                        .windows
                        .iter()
                        .find(|w| Some(w.river_window.id()) == state.focused_window)
                    {
                        seat.focus_window(&window.river_window);
                    } else {
                        seat.clear_focus();
                    }
                    state.focus_dirty = false;
                }

                install_bindings(state, qh);
                manager.manage_finish();
            }

            river::river_window_management::river_window_manager_v1::Event::RenderStart => {
                for window in state
                    .windows
                    .iter()
                    .filter(|w| !w.floating)
                    .chain(state.windows.iter().filter(|w| w.floating))
                {
                    if let Some(node) = &window.node {
                        if let Some((x, y, _, _)) = window.geometry {
                            let output = &state.outputs[window.output.unwrap()];
                            let (ox, _) = output.position.unwrap();
                            let (ow, oh) = output.dimensions.unwrap();
                            let (_, _, width, height) = window.geometry.unwrap();
                            let left = x.max(ox);
                            let right = (x + width).min(ox + ow);
                            if right <= left {
                                window.river_window.hide();
                                continue;
                            }
                            window.river_window.show();
                            if window.river_window.version() >= 2 {
                                window.river_window.set_clip_box(
                                    left - x,
                                    0,
                                    right - left,
                                    height.min(oh),
                                );
                            }
                            node.set_position(x, y);
                            node.place_top();
                        } else {
                            window.river_window.hide();
                        }
                    }
                }

                manager.render_finish();
            }

            _ => {}
        }
    }

    wayland_client::event_created_child!(
        State,
        RiverWindowManagerV1,
        [
            6 => (RiverWindowV1, ()),
            7 => (RiverOutputV1, ()),
            8 => (RiverSeatV1, ()),
        ]
    );
}

impl Dispatch<RiverWindowV1, ()> for State {
    fn event(
        state: &mut Self,
        window: &RiverWindowV1,
        event: river::river_window_management::river_window_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river::river_window_management::river_window_v1::Event;
        match event {
            Event::Closed => remove_window(state, window.id()),
            Event::AppId { app_id } => {
                println!("Window {} app_id: {:?}", window.id(), app_id);
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    item.app_id = app_id;
                }
            }
            Event::Parent { parent } => {
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    item.parent = parent.map(|p| p.id());
                }
            }
            Event::PointerMoveRequested { seat } if state.seat.as_ref() == Some(&seat) => {
                state.pending_drag = Some((window.id(), PointerAction::Move));
            }
            Event::PointerResizeRequested {
                seat,
                edges: wayland_client::WEnum::Value(edges),
            } if state.seat.as_ref() == Some(&seat) => {
                state.pending_drag = Some((window.id(), PointerAction::ResizeEdges(edges)));
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(
        State,
        RiverWindowV1,
        [
            2 => (RiverNodeV1, ())
        ]
    );
}

impl Dispatch<RiverNodeV1, ()> for State {
    fn event(
        _state: &mut Self,
        _node: &RiverNodeV1,
        _event: river::river_window_management::river_node_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

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

impl Dispatch<RiverSeatV1, ()> for State {
    fn event(
        state: &mut Self,
        seat: &RiverSeatV1,
        event: river::river_window_management::river_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("mywm starting");
    let config = Config::load()?;

    let conn = Connection::connect_to_env()?;
    println!("connected to Wayland");

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let display = conn.display();
    display.get_registry(&qh, ());

    let mut state = State {
        windows: Vec::new(),
        outputs: Vec::new(),
        focused_output: None,
        pointer_position: None,
        pointer_window: None,
        pointer_bindings: Vec::new(),
        pending_drag: None,
        drag: None,
        drag_delta: None,
        end_drag: false,
        seat: None,
        focused_window: None,
        focus_dirty: false,
        config,
        detached_workspaces: None,
        bindings: Vec::new(),
        actions: Vec::new(),
        exit_requested: false,
        xkb_bindings: None,
    };

    event_queue.roundtrip(&mut state)?;

    println!("Entering event loop");

    loop {
        if let Err(error) = event_queue.blocking_dispatch(&mut state) {
            // River disconnects clients when it handles exit_session.
            if state.exit_requested {
                return Ok(());
            }
            return Err(error.into());
        }
    }
}
