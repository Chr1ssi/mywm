//! Keybinding actions, command spawning, config reload and binding installation.
use crate::*;

pub(crate) fn run_actions(
    state: &mut State,
    manager: &RiverWindowManagerV1,
    qh: &QueueHandle<State>,
) {
    for action in std::mem::take(&mut state.actions) {
        if state.session_locked && !matches!(action, Action::Lock) {
            continue;
        }
        match action {
            Action::Reload => reload_config(state, qh),
            Action::Wallpaper => {
                let (x, y) = state
                    .pointer_position
                    .or_else(|| state.focused_output.and_then(|i| state.outputs[i].position))
                    .unwrap_or((0, 0));
                match wallpaper::picker(x, y).spawn() {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(error) => eprintln!("Cannot open wallpaper picker: {error}"),
                }
            }
            Action::Lock => {
                session::start_lock(&state.config, &mut state.locker, state.session_locked)
            }
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
            Action::Launcher => {
                let mut command = std::process::Command::new(&state.config.launcher[0]);
                command.args(&state.config.launcher[1..]);
                state.config.apply_theme(&mut command);
                command.env(
                    "MYWM_TERMINAL_COUNT",
                    state.config.terminal.len().to_string(),
                );
                for (index, argument) in state.config.terminal.iter().enumerate() {
                    command.env(format!("MYWM_TERMINAL_{index}"), argument);
                }
                if let Some((x, y)) = state.pointer_position.or_else(|| {
                    state
                        .focused_output
                        .and_then(|i| state.outputs[i].work_area())
                        .map(|r| (r.x + r.width / 2, r.y + r.height / 2))
                }) {
                    command
                        .env("MYWM_LAUNCHER_X", x.to_string())
                        .env("MYWM_LAUNCHER_Y", y.to_string());
                }
                match command.spawn() {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(error) => eprintln!("Cannot start launcher: {error}"),
                }
            }
            Action::Program(index) => {
                if let Some(binding) = state.config.program_bindings.values().nth(index) {
                    spawn_command(&binding.command, "program binding");
                }
            }
            Action::WorkspaceOnOutput(id, target) => {
                if let Some(output) = state
                    .outputs
                    .iter()
                    .position(|o| o.river_output.id().protocol_id() == id)
                {
                    if !state.outputs[output].workspaces.contains(target) {
                        continue;
                    }
                    cancel_drag(state);
                    monitor_workspaces::select(state, output, target);
                }
            }
            Action::Workspace(target) => {
                cancel_drag(state);
                if let Some(output) = monitor_workspaces::owner(state, target) {
                    monitor_workspaces::select(state, output, target);
                }
            }
            Action::NewWorkspace => {
                cancel_drag(state);
                if let Some(output) = output_at_pointer(state).or(state.focused_output)
                    && let Some(number) = monitor_workspaces::create(state, output)
                {
                    monitor_workspaces::select(state, output, number);
                }
            }
            Action::WorkspaceRelative(direction) => {
                cancel_drag(state);
                if let Some(output) = output_at_pointer(state).or(state.focused_output)
                    && let Some(target) = relative_workspace(state, output, direction)
                {
                    monitor_workspaces::select(state, output, target);
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
            Action::ToggleScratchpad => {
                if state.scratchpad.windows.is_empty() {
                    continue;
                }
                cancel_drag(state);
                state.scratchpad_visible = !state.scratchpad_visible;
                if state.scratchpad_visible {
                    focus_scratchpad(state);
                } else if let Some(output) = output_at_pointer(state).or(state.focused_output) {
                    focus_output(state, output);
                }
            }
            Action::MoveToScratchpad => {
                let Some(id) = state.focused_window.clone() else {
                    continue;
                };
                cancel_drag(state);
                if state.scratchpad.windows.contains(&id) {
                    state.scratchpad.remove(&id);
                    state.scratchpad_visible = !state.scratchpad.windows.is_empty();
                    if let Some(output) = output_at_pointer(state).or(state.focused_output) {
                        state.outputs[output].workspaces.add(id.clone());
                        if let Some(window) =
                            state.windows.iter_mut().find(|w| w.river_window.id() == id)
                        {
                            window.output = Some(output);
                            window.floating = window.scratchpad_floating.take().unwrap_or(true);
                        }
                        focus_output(state, output);
                    }
                } else {
                    for output in &mut state.outputs {
                        output.workspaces.remove(&id);
                    }
                    state.scratchpad.windows.push(id.clone());
                    state.scratchpad.focused = Some(id.clone());
                    state.scratchpad_visible = true;
                    if let Some(window) =
                        state.windows.iter_mut().find(|w| w.river_window.id() == id)
                    {
                        window.scratchpad_floating = Some(window.floating);
                        window.floating = true;
                    }
                    focus_scratchpad(state);
                }
            }
            Action::FocusOutput(direction) => {
                if let Some(source) = output_at_pointer(state).or(state.focused_output)
                    && let Some(target) = adjacent_output(state, source, direction)
                {
                    focus_output(state, target);
                    warp_to_output(state, target);
                }
            }
            Action::MoveToOutput(direction) => {
                let Some(id) = state.focused_window.clone() else {
                    continue;
                };
                let Some(source) = state
                    .windows
                    .iter()
                    .find(|window| window.river_window.id() == id)
                    .and_then(|window| window.output)
                else {
                    continue;
                };
                let horizontal = match direction {
                    OutputDirection::Left => Some(-1),
                    OutputDirection::Right => Some(1),
                    OutputDirection::Up | OutputDirection::Down => None,
                };
                if !state
                    .windows
                    .iter()
                    .any(|window| window.river_window.id() == id && window.floating)
                    && let Some(direction) = horizontal
                {
                    let tiled: Vec<_> = state
                        .windows
                        .iter()
                        .filter(|window| !window.floating)
                        .map(|window| window.river_window.id())
                        .collect();
                    if state.outputs[source]
                        .workspaces
                        .can_navigate_matching(direction, |candidate| tiled.contains(candidate))
                    {
                        state.outputs[source].workspaces.navigate_matching(
                            direction,
                            true,
                            |candidate| tiled.contains(candidate),
                        );
                        focus_output(state, source);
                        continue;
                    }
                }
                let Some(target) = adjacent_output(state, source, direction) else {
                    continue;
                };
                let workspace = state.outputs[target].workspaces.active;
                if !workspace_accepts(state, workspace, &id) {
                    continue;
                }
                cancel_drag(state);
                state.outputs[source].workspaces.remove(&id);
                state.outputs[target]
                    .workspaces
                    .add_to(workspace, id.clone());
                if let Some(window) = state
                    .windows
                    .iter_mut()
                    .find(|window| window.river_window.id() == id)
                {
                    window.output = Some(target);
                    if window.floating {
                        window.floating_rect = None;
                    }
                }
                focus_output(state, target);
                warp_to_output(state, target);
            }
            Action::Close
            | Action::Focus(_)
            | Action::Move(_)
            | Action::MoveToWorkspace(_)
            | Action::MoveToWorkspaceRelative(_)
            | Action::MoveToNewWorkspace => {
                let Some((window_id, output, floating)) = state
                    .windows
                    .iter()
                    .find(|w| Some(w.river_window.id()) == state.focused_window)
                    .and_then(|window| {
                        Some((window.river_window.id(), window.output?, window.floating))
                    })
                else {
                    continue;
                };
                match action {
                    Action::Close => {
                        if let Some(window) = state
                            .windows
                            .iter()
                            .find(|window| window.river_window.id() == window_id)
                        {
                            window.river_window.close();
                        }
                    }
                    Action::Focus(direction) | Action::Move(direction) => {
                        if floating && matches!(action, Action::Move(_)) {
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
                        if workspace_accepts(state, target, &window_id) {
                            monitor_workspaces::move_window(state, output, target);
                        }
                    }
                    Action::MoveToWorkspaceRelative(direction) => {
                        cancel_drag(state);
                        if let Some(target) = relative_workspace(state, output, direction)
                            && workspace_accepts(state, target, &window_id)
                        {
                            monitor_workspaces::move_window(state, output, target);
                        }
                    }
                    Action::MoveToNewWorkspace => {
                        cancel_drag(state);
                        if let Some(target) = monitor_workspaces::create(state, output) {
                            monitor_workspaces::move_window(state, output, target);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

pub(crate) fn spawn_command(command: &[String], context: &str) {
    match std::process::Command::new(&command[0])
        .args(&command[1..])
        .spawn()
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("Cannot start {context} '{}': {error}", command[0]),
    }
}

pub(crate) fn reload_config(state: &mut State, qh: &QueueHandle<State>) {
    let new = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Configuration reload failed; keeping current configuration: {error}");
            return;
        }
    };

    vrr::disable(state);
    for binding in state.bindings.drain(..) {
        binding.destroy();
    }
    for binding in state.pointer_bindings.drain(..) {
        binding.destroy();
    }
    state.config.apply_reloadable(new);
    state.vrr_sender = vrr::worker(&state.config.vrr);
    install_bindings(state, qh);
    state.focus_dirty = true;
    eprintln!("Configuration reloaded");
}

pub(crate) fn install_bindings(state: &mut State, qh: &QueueHandle<State>) {
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
