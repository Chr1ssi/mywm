/// Compute fixed-width columns and minimally scroll the focused column into view.
pub fn columns(
    width: i32,
    count: usize,
    focus: Option<usize>,
    scroll: i32,
) -> (i32, Vec<(i32, i32)>) {
    if count == 0 || width <= 0 {
        return (0, Vec::new());
    }
    let column_width = if count == 1 {
        width
    } else {
        (width / 2).max(1)
    };
    let max_scroll = (column_width * count as i32 - width).max(0);
    let mut scroll = scroll.clamp(0, max_scroll);
    if let Some(focus) = focus.filter(|i| *i < count) {
        let left = focus as i32 * column_width;
        if left < scroll {
            scroll = left;
        } else if left + column_width > scroll + width {
            scroll = left + column_width - width;
        }
    }
    (
        scroll,
        (0..count)
            .map(|i| (i as i32 * column_width - scroll, column_width))
            .collect(),
    )
}

pub fn index_after_remove(index: usize, removed: usize) -> Option<usize> {
    if index == removed {
        None
    } else if index > removed {
        Some(index - 1)
    } else {
        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_fills_monitor_two_split_it() {
        assert_eq!(columns(1920, 1, Some(0), 960), (0, vec![(0, 1920)]));
        assert_eq!(
            columns(1920, 2, Some(1), 0),
            (0, vec![(0, 960), (960, 960)])
        );
        assert_eq!(columns(1920, 0, None, 960), (0, vec![]));
    }

    #[test]
    fn focus_scrolls_only_when_needed() {
        let (scroll, positions) = columns(1920, 3, Some(2), 0);
        assert_eq!(scroll, 960);
        assert_eq!(positions, vec![(-960, 960), (0, 960), (960, 960)]);
        assert_eq!(columns(1920, 3, Some(1), scroll).0, 960);
        assert_eq!(columns(1920, 3, Some(0), scroll).0, 0);
    }

    #[test]
    fn removal_and_resize_clamp_viewport() {
        assert_eq!(columns(1920, 2, Some(1), 1920).0, 0);
        let (scroll, positions) = columns(1280, 4, Some(3), 1920);
        assert_eq!(scroll, 1280);
        assert_eq!(positions[3], (640, 640));
        let (_, positions) = columns(1919, 3, Some(2), 0);
        assert_eq!(positions[2].0 + positions[2].1, 1919);
    }

    #[test]
    fn removed_indices_cannot_refer_to_another_window() {
        assert_eq!(index_after_remove(2, 2), None);
        assert_eq!(index_after_remove(3, 2), Some(2));
        assert_eq!(index_after_remove(1, 2), Some(1));
    }
}
