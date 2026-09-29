//! River window-manager events: manage and render sequences.
use crate::*;

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
                    actual_dimensions: None,
                    proposed_dimensions: None,
                    proposal_retried: false,
                    nudge: false,
                    nudge_restore: false,
                    output: None,
                    floating: false,
                    scratchpad_floating: None,
                    fullscreen: false,
                    fullscreen_output: None,
                    floating_rect: None,
                    tiled_width: None,
                    border_width: 0,
                    app_id: None,
                    parent: None,
                });
            }

            river::river_window_management::river_window_manager_v1::Event::Output { id } => {
                state.outputs.push(Output {
                    wl_global: None,
                    river_output: id,
                    position: None,
                    dimensions: None,
                    workspaces: Workspaces::new(state.config.workspaces),
                    layer_output: None,
                    non_exclusive_area: None,
                    presentation_mode_set: false,
                });

                layer_shell::setup(state, qh);
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

            river::river_window_management::river_window_manager_v1::Event::SessionLocked => {
                state.session_locked = true;
                cancel_drag(state);
            }
            river::river_window_management::river_window_manager_v1::Event::SessionUnlocked => {
                state.session_locked = false;
                state.focus_dirty = true;
            }
            river::river_window_management::river_window_manager_v1::Event::ManageStart => {
                monitor_workspaces::reconcile(state);
                let current_output = output_at_pointer(state)
                    .or(state.focused_output)
                    .or_else(|| (!state.outputs.is_empty()).then_some(0));

                layer_shell::setup(state, qh);
                if let Some(output) = current_output {
                    if let Some(layer_output) = &state.outputs[output].layer_output {
                        layer_output.set_default();
                    }
                    assign_windows(state, output);
                }
                run_actions(state, manager, qh);
                if state.exit_requested {
                    manager.manage_finish();
                    return;
                }
                update_drag(state);
                layout(state);
                fullscreen::apply(state);
                vrr::apply(state);
                let mut schedule_nudge_restore = false;
                for window in &mut state.windows {
                    // The WM supplies focus borders, but no title bar.
                    window.river_window.use_ssd();
                    if window.node.is_none() {
                        let node = window.river_window.get_node(qh, ());
                        window.node = Some(node);
                    }

                    if window.fullscreen_output.is_some() {
                        continue;
                    }
                    if let Some((_, _, width, height)) = window.geometry {
                        window.river_window.set_tiled(if window.floating {
                            Edges::empty()
                        } else {
                            Edges::Top | Edges::Bottom | Edges::Left | Edges::Right
                        });
                        let proposal = (width, height);
                        if window.proposed_dimensions != Some(proposal) {
                            window.proposal_retried = false;
                        }
                        window.proposed_dimensions = Some(proposal);
                        if window.nudge {
                            window.nudge = false;
                            window.nudge_restore = true;
                            schedule_nudge_restore = true;
                            window
                                .river_window
                                .propose_dimensions(width, (height - 1).max(1));
                        } else {
                            window.river_window.propose_dimensions(width, height);
                        }
                    }
                }

                if schedule_nudge_restore {
                    state.dimension_retry_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(150));
                }

                if state.layer_focus_granted {
                    // Let the shell receive focus this sequence, including when an
                    // application is announced at the same time as its launcher.
                    state.focus_dirty = false;
                }
                if state.focus_dirty
                    && state.layer_focus != LayerFocus::Exclusive
                    && let Some(seat) = &state.seat
                {
                    if let Some(window) = state.windows.iter().find(|w| {
                        Some(w.river_window.id()) == state.focused_window && w.geometry.is_some()
                    }) {
                        seat.focus_window(&window.river_window);
                    } else {
                        seat.clear_focus();
                    }
                    state.focus_dirty = false;
                }

                install_bindings(state, qh);
                for binding in &state.bindings {
                    if state.session_locked {
                        binding.disable();
                    } else {
                        binding.enable();
                    }
                }
                for binding in &state.pointer_bindings {
                    if state.session_locked {
                        binding.disable();
                    } else {
                        binding.enable();
                    }
                }
                state.layer_focus_granted = false;
                manager.manage_finish();
            }

            river::river_window_management::river_window_manager_v1::Event::RenderStart => {
                if state.dimension_retry_at.is_none()
                    && state.windows.iter().any(needs_dimension_retry)
                {
                    state.dimension_retry_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
                }
                for output in &mut state.outputs {
                    if !output.presentation_mode_set
                        && output.river_output.version() >= 4
                        && let Some(name) = output
                            .wl_global
                            .and_then(|global| state.wl_outputs.get(&global))
                            .and_then(|(_, name)| name.as_deref())
                    {
                        let mode = if state
                            .config
                            .async_outputs
                            .iter()
                            .any(|configured| configured == name)
                        {
                            river::river_window_management::river_output_v1::PresentationMode::Async
                        } else {
                            river::river_window_management::river_output_v1::PresentationMode::Vsync
                        };
                        output.river_output.set_presentation_mode(mode);
                        output.presentation_mode_set = true;
                    }
                }
                for window in state
                    .windows
                    .iter()
                    .filter(|w| !w.floating)
                    .chain(state.windows.iter().filter(|w| w.floating))
                {
                    if let Some(node) = &window.node {
                        if window.fullscreen_output.is_some() {
                            window.river_window.show();
                            node.place_top();
                            continue;
                        }
                        if let Some((x, y, _, _)) = window.geometry {
                            let output = &state.outputs[window.output.unwrap()];
                            let Some(area) = output.work_area() else {
                                window.river_window.hide();
                                continue;
                            };
                            let area = if window.floating {
                                area
                            } else {
                                state.config.appearance.viewport(area)
                            };
                            let (_, _, width, height) = window.geometry.unwrap();
                            let border = window.border_width;
                            let left = (x - border).max(area.x);
                            let right = (x + width + border).min(area.x + area.width);
                            let top = (y - border).max(area.y);
                            let bottom = (y + height + border).min(area.y + area.height);
                            if right <= left || bottom <= top {
                                window.river_window.hide();
                                continue;
                            }
                            let active = state.layer_focus == LayerFocus::None
                                && state.focused_window.as_ref() == Some(&window.river_window.id());
                            let color = if active {
                                state.config.appearance.active_border
                            } else {
                                state.config.appearance.inactive_border
                            };
                            let [r, g, b, a] = color.0;
                            window.river_window.set_borders(
                                Edges::Top | Edges::Bottom | Edges::Left | Edges::Right,
                                border,
                                r,
                                g,
                                b,
                                a,
                            );
                            window.river_window.show();
                            if window.river_window.version() >= 3 {
                                window
                                    .river_window
                                    .set_content_clip_box(0, 0, width, height);
                            }
                            if window.river_window.version() >= 2 {
                                window.river_window.set_clip_box(
                                    left - x,
                                    top - y,
                                    right - left,
                                    bottom - top,
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
