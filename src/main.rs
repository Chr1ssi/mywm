mod river;

use river::river_window_management::{
    river_node_v1::RiverNodeV1,
    river_output_v1::RiverOutputV1,
    river_seat_v1::RiverSeatV1,
    river_window_manager_v1::RiverWindowManagerV1,
    river_window_v1::RiverWindowV1,
};

use river::river_window_management::river_seat_v1::Modifiers;
use river::river_xkb_bindings::river_xkb_bindings_v1::RiverXkbBindingsV1;
use river::river_xkb_bindings::river_xkb_binding_v1::RiverXkbBindingV1;
use river::river_xkb_bindings::river_xkb_bindings_seat_v1::RiverXkbBindingsSeatV1;

use wayland_client::{
    Connection,
    Dispatch,
    Proxy,
    QueueHandle,
    protocol::wl_registry,
};

struct Window {
    river_window: RiverWindowV1,
    node: Option<RiverNodeV1>,
    geometry: Option<(i32, i32, i32, i32)>,
    output: Option<usize>,
}

struct Output {
    river_output: RiverOutputV1,
    position: Option<(i32, i32)>,
    dimensions: Option<(i32, i32)>,
}

struct State {
    windows: Vec<Window>,
    outputs: Vec<Output>,
    focused_output: Option<usize>,
    pointer_position: Option<(i32, i32)>,
    seat: Option<RiverSeatV1>,
    focused_window: Option<usize>,
    pending_focus: Option<usize>,

    super_return_binding: Option<RiverXkbBindingV1>,
    xkb_bindings: Option<RiverXkbBindingsV1>,
}

fn layout(state: &mut State) {
    for output_index in 0..state.outputs.len() {
        let Some((output_x, output_y)) = state.outputs[output_index].position else {
            continue;
        };

        let Some((output_width, output_height)) = state.outputs[output_index].dimensions else {
            continue;
        };

        let window_indices: Vec<usize> = state
            .windows
            .iter()
            .enumerate()
            .filter_map(|(index, window)| {
                (window.output == Some(output_index)).then_some(index)
            })
            .collect();

        if window_indices.is_empty() {
            continue;
        }

        let window_height = output_height / window_indices.len() as i32;

        for (slot, window_index) in window_indices.into_iter().enumerate() {
            let x = output_x;
            let y = output_y + slot as i32 * window_height;

            state.windows[window_index].geometry =
                Some((x, y, output_width, window_height));

        }
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
                index,
                pointer_x,
                pointer_y
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

    let was_focused = state.focused_window == Some(closed_index);

    state.windows.remove(closed_index);

    if state.windows.is_empty() {
        state.focused_window = None;
        println!("Window {window_id} closed; no windows remaining");
        return;
    }

    match state.focused_window {
        Some(index) if was_focused => {
            state.focused_window = Some(index.min(state.windows.len() - 1));
        }

        Some(index) if index > closed_index => {
            state.focused_window = Some(index - 1);
        }

        Some(_) => {}

        None => {}
    }

    println!("Window {window_id} closed");
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

                registry.bind::<RiverWindowManagerV1, _, _>(
                    name,
                    version.min(5),
                    qh,
                    (),
                );
            }
	
	    if interface == "river_xkb_bindings_v1" {
    		println!("Binding river_xkb_bindings_v1 v{version}");

    		let bindings = registry.bind::<RiverXkbBindingsV1, _, _>(
        	    name,
        	    version.min(3),
        	    qh,
        	    (),
    		);

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
                });

		let window_index = state.windows.len() - 1;

                state.focused_window = Some(window_index);
		state.pending_focus = Some(window_index);
            }

            river::river_window_management::river_window_manager_v1::Event::Output { id } => {
                state.outputs.push(Output {
                    river_output: id,
                    position: None,
                    dimensions: None,
                });

                if state.focused_output.is_none() {
                    state.focused_output = Some(0);
                }
            }

            river::river_window_management::river_window_manager_v1::Event::ManageStart => {
                let current_output = output_at_pointer(state);

                for window in &mut state.windows {
                    if window.output.is_none() {
                        window.output = current_output;

                        println!(
                        "Assigning new window to output: {:?}",
                        window.output
                        );
                    }

                    if window.node.is_none() {
                        let node = window.river_window.get_node(qh, ());
                        window.node = Some(node);
                    }

                    if let Some((_, _, width, height)) = window.geometry {
                        window.river_window.propose_dimensions(width, height);
                    }
                }

                if let (Some(seat), Some(window_index)) = (
                    state.seat.clone(),
                    state.pending_focus.take(),
                ) {
		    let window = &state.windows[window_index];

                    seat.focus_window(&window.river_window);
                }

                if state.super_return_binding.is_none() {
                    if let (Some(bindings), Some(seat)) = (&state.xkb_bindings, &state.seat) {
                        let binding = bindings.get_xkb_binding(
                            seat,
                            0xff0d, // XKB_KEY_Return
                            Modifiers::Mod4, // Super
                            qh,
                            (),
                        );

                        binding.set_layout_override(0);
                        binding.enable();

                        state.super_return_binding = Some(binding);
                    }
                }
                manager.manage_finish();
            }

            river::river_window_management::river_window_manager_v1::Event::RenderStart => {
                layout(state);

                for window in &state.windows {
                    if let Some(node) = &window.node {
                        if let Some((x, y, _, _)) = window.geometry {
                            node.set_position(x, y);
                            node.place_top();
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
        if let river::river_window_management::river_window_v1::Event::Closed = event {
            remove_window(state, window.id());
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

                println!(
                    "Output {} dimensions: {width}x{height}",
                    output.id()
                );
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
        state.seat = Some(seat.clone());

	match event {
            river::river_window_management::river_seat_v1::Event::PointerPosition {
                x,
                y,
            } => {
            state.pointer_position = Some((x, y));

            update_focused_output(state, x, y);
        }

        river::river_window_management::river_seat_v1::Event::WindowInteraction {
            window,
        } => {
            let Some(window_index) = state
                .windows
                .iter()
                .position(|item| item.river_window.id() == window.id())
            else {
                println!("Window interaction for unknown window");
                return;
            };

            state.focused_window = Some(window_index);
	    state.pending_focus = Some(window_index);
    	}

    	_ => {}
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

impl Dispatch<RiverXkbBindingV1, ()> for State {
    fn event(
        _state: &mut State,
        _binding: &RiverXkbBindingV1,
        event: river::river_xkb_bindings::river_xkb_binding_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        match event {
            river::river_xkb_bindings::river_xkb_binding_v1::Event::Pressed => {
                println!("Super + Return pressed!");
            }
            _ => {}
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
        seat: None,
        focused_window: None,
	    pending_focus: None,
        super_return_binding: None,
    	xkb_bindings: None,
    };

    event_queue.roundtrip(&mut state)?;

    println!("Entering event loop");

    loop {
        event_queue.blocking_dispatch(&mut state)?;
    }
}
