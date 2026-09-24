/// A workspace owns its window order, remembered focus and horizontal viewport.
#[derive(Debug)]
pub struct Workspace<T> {
    pub windows: Vec<T>,
    pub focused: Option<T>,
    pub scroll: i32,
}

impl<T> Default for Workspace<T> {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            focused: None,
            scroll: 0,
        }
    }
}

#[derive(Debug)]
pub struct Workspaces<T> {
    pub active: usize,
    pub entries: Vec<Workspace<T>>,
}

impl<T: Clone + Eq> Workspaces<T> {
    pub fn new(count: usize) -> Self {
        assert!(count > 0);
        Self {
            active: 0,
            entries: (0..count).map(|_| Workspace::default()).collect(),
        }
    }

    pub fn current(&self) -> &Workspace<T> {
        &self.entries[self.active]
    }

    pub fn current_mut(&mut self) -> &mut Workspace<T> {
        &mut self.entries[self.active]
    }

    pub fn select(&mut self, workspace: usize) {
        if workspace < self.entries.len() {
            self.active = workspace;
        }
    }

    pub fn add(&mut self, window: T) {
        self.add_to(self.active, window);
    }

    pub fn add_to(&mut self, target: usize, window: T) {
        let workspace = &mut self.entries[target];
        workspace.focused = Some(window.clone());
        workspace.windows.push(window);
    }

    pub fn location(&self, window: &T) -> Option<usize> {
        self.entries
            .iter()
            .position(|workspace| workspace.windows.contains(window))
    }

    pub fn focus(&mut self, window: &T) -> bool {
        if self.current().windows.contains(window) {
            self.current_mut().focused = Some(window.clone());
            true
        } else {
            false
        }
    }

    pub fn navigate(&mut self, direction: isize, reorder: bool) {
        self.navigate_matching(direction, reorder, |_| true);
    }

    pub fn navigate_matching(
        &mut self,
        direction: isize,
        reorder: bool,
        mut include: impl FnMut(&T) -> bool,
    ) {
        let workspace = self.current_mut();
        let indices: Vec<_> = workspace
            .windows
            .iter()
            .enumerate()
            .filter_map(|(i, id)| include(id).then_some(i))
            .collect();
        let Some(slot) = indices
            .iter()
            .position(|i| Some(&workspace.windows[*i]) == workspace.focused.as_ref())
        else {
            return;
        };
        let Some(next) = slot
            .checked_add_signed(direction)
            .and_then(|i| indices.get(i))
            .copied()
        else {
            return;
        };
        if reorder {
            workspace.windows.swap(indices[slot], next);
        } else {
            workspace.focused = Some(workspace.windows[next].clone());
        }
    }

    pub fn remove(&mut self, window: &T) {
        for workspace in &mut self.entries {
            if let Some(index) = workspace.windows.iter().position(|id| id == window) {
                workspace.windows.remove(index);
                if workspace.focused.as_ref() == Some(window) {
                    workspace.focused = workspace
                        .windows
                        .get(index)
                        .or_else(|| workspace.windows.last())
                        .cloned();
                }
                if workspace.windows.is_empty() {
                    workspace.scroll = 0;
                }
                return;
            }
        }
    }

    #[cfg(test)]
    pub fn move_focused_to(&mut self, target: usize) {
        if target == self.active || target >= self.entries.len() {
            return;
        }
        let Some(window) = self.current().focused.clone() else {
            return;
        };
        self.remove(&window);
        self.entries[target].windows.push(window.clone());
        self.entries[target].focused = Some(window);
    }

    /// Preserve workspace numbers when an output is unplugged.
    pub fn absorb(&mut self, other: Self) {
        for (target, source) in self.entries.iter_mut().zip(other.entries) {
            target.windows.extend(source.windows);
            if target.focused.is_none() {
                target.focused = source.focused;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_restores_focus_order_and_scroll() {
        let mut desktop = Workspaces::new(3);
        desktop.add(1);
        desktop.add(2);
        desktop.add(3);
        desktop.current_mut().scroll = 960;
        desktop.select(1);
        assert!(desktop.current().windows.is_empty());
        assert_eq!(desktop.current().focused, None);
        desktop.add(4);
        desktop.select(0);
        assert_eq!(desktop.current().windows, [1, 2, 3]);
        assert_eq!(desktop.current().focused, Some(3));
        assert_eq!(desktop.current().scroll, 960);
    }

    #[test]
    fn moving_does_not_follow_and_never_duplicates_a_window() {
        let mut desktop = Workspaces::new(2);
        desktop.add(1);
        desktop.add(2);
        desktop.move_focused_to(1);
        assert_eq!(desktop.active, 0);
        assert_eq!(desktop.current().windows, [1]);
        assert_eq!(desktop.current().focused, Some(1));
        desktop.move_focused_to(1);
        assert_eq!(desktop.current().focused, None);
        desktop.select(1);
        assert_eq!(desktop.current().windows, [2, 1]);
        assert_eq!(desktop.current().focused, Some(1));
        desktop.move_focused_to(1);
        assert_eq!(desktop.current().windows, [2, 1]);
    }

    #[test]
    fn navigation_and_removal_stay_inside_workspace() {
        let mut desktop = Workspaces::new(2);
        desktop.add(1);
        desktop.add(2);
        desktop.select(1);
        desktop.add(3);
        assert!(!desktop.focus(&1));
        desktop.remove(&2);
        assert_eq!(desktop.current().focused, Some(3));
        desktop.select(0);
        assert_eq!(desktop.current().focused, Some(1));
        desktop.add(4);
        desktop.navigate(-1, true);
        assert_eq!(desktop.current().windows, [4, 1]);
        assert_eq!(desktop.current().focused, Some(4));
        desktop.navigate(-1, false);
        assert_eq!(desktop.current().focused, Some(4));
        desktop.navigate(1, false);
        assert_eq!(desktop.current().focused, Some(1));
    }

    #[test]
    fn reordering_columns_skips_floating_windows() {
        let mut desktop = Workspaces::new(1);
        desktop.add(1);
        desktop.add(2); // floating, retains its position in keyboard focus order
        desktop.add(3);
        desktop.navigate_matching(-1, true, |id| *id != 2);
        assert_eq!(desktop.current().windows, [3, 2, 1]);
        assert_eq!(desktop.current().focused, Some(3));
        desktop.navigate(1, false);
        assert_eq!(desktop.current().focused, Some(2));
    }

    #[test]
    fn monitors_are_independent_and_unplug_preserves_workspace_numbers() {
        let mut left = Workspaces::new(3);
        let mut right = Workspaces::new(3);
        left.add(1);
        left.select(1);
        left.add(2);
        right.add(3);
        right.select(2);
        right.add(4);
        left.select(0);
        assert_eq!(right.active, 2);
        left.absorb(right);
        assert_eq!(left.current().windows, [1, 3]);
        assert_eq!(left.current().focused, Some(1));
        left.select(2);
        assert_eq!(left.current().windows, [4]);
        assert_eq!(left.current().focused, Some(4));
    }
}
