use ratatui::widgets::ListState;
use std::cmp;

pub(crate) fn move_selection_down(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        *state.offset_mut() = 0;
        return;
    }
    let selected = state.selected().unwrap_or(0);
    let next = cmp::min(selected + 1, len - 1);
    state.select(Some(next));
}

pub(crate) fn move_selection_up(state: &mut ListState) {
    let Some(selected) = state.selected() else {
        return;
    };
    state.select(Some(selected.saturating_sub(1)));
}

pub(crate) fn page_down(state: &mut ListState, len: usize, page_size: usize) {
    if len == 0 {
        state.select(None);
        *state.offset_mut() = 0;
        return;
    }
    let selected = state.selected().unwrap_or(0);
    let page_size = cmp::max(page_size, 1);
    let next = cmp::min(selected + page_size, len - 1);
    state.select(Some(next));
    ensure_visible(state, len, page_size);
}

pub(crate) fn page_up(state: &mut ListState, page_size: usize) {
    let Some(selected) = state.selected() else {
        return;
    };
    let page_size = cmp::max(page_size, 1);
    state.select(Some(selected.saturating_sub(page_size)));
    ensure_visible(state, selected + 1, page_size);
}

pub(crate) fn ensure_visible(state: &mut ListState, len: usize, page_size: usize) {
    if len == 0 {
        *state.offset_mut() = 0;
        return;
    }
    let Some(selected) = state.selected() else {
        *state.offset_mut() = 0;
        return;
    };
    let page_size = cmp::max(page_size, 1);

    let offset = state.offset();
    if selected < offset {
        *state.offset_mut() = selected;
    } else if selected >= offset + page_size {
        *state.offset_mut() = selected.saturating_sub(page_size - 1);
    }
}

pub(crate) fn ensure_comment_line_offset(
    state: &mut ListState,
    line_offset: &mut usize,
    item_heights: &[usize],
    viewport_height: usize,
) {
    if item_heights.is_empty() || viewport_height == 0 {
        *line_offset = 0;
        return;
    }
    let Some(selected) = state.selected() else {
        *line_offset = 0;
        return;
    };

    let len = item_heights.len();
    let selected = if selected >= len {
        let last = len - 1;
        state.select(Some(last));
        last
    } else {
        selected
    };

    let viewport_height = viewport_height.max(1);
    let total_lines = comment_total_lines(item_heights);
    let max_offset = total_lines.saturating_sub(viewport_height);

    let (start, end) = comment_line_range(item_heights, selected);
    let height = end.saturating_sub(start);
    if height >= viewport_height {
        *line_offset = start.min(max_offset);
        return;
    }

    let mut offset = (*line_offset).min(max_offset);
    if start < offset {
        offset = start;
    } else if end > offset + viewport_height {
        offset = end.saturating_sub(viewport_height);
    }
    *line_offset = offset.min(max_offset);
}

pub(crate) fn page_down_comment_list(
    state: &mut ListState,
    len: usize,
    page_size: usize,
    line_offset: &mut usize,
    item_heights: &[usize],
    viewport_height: usize,
) {
    if comment_heights_ready(len, item_heights, viewport_height) {
        page_down_with_heights(state, item_heights, viewport_height);
        ensure_comment_line_offset(state, line_offset, item_heights, viewport_height);
    } else {
        page_down(state, len, page_size);
    }
}

pub(crate) fn page_up_comment_list(
    state: &mut ListState,
    len: usize,
    page_size: usize,
    line_offset: &mut usize,
    item_heights: &[usize],
    viewport_height: usize,
) {
    if comment_heights_ready(len, item_heights, viewport_height) {
        page_up_with_heights(state, item_heights, viewport_height);
        ensure_comment_line_offset(state, line_offset, item_heights, viewport_height);
    } else {
        page_up(state, page_size);
    }
}

pub(crate) fn ensure_comment_visible(
    state: &mut ListState,
    line_offset: &mut usize,
    len: usize,
    item_heights: &[usize],
    viewport_height: usize,
) {
    if comment_heights_ready(len, item_heights, viewport_height) {
        ensure_comment_line_offset(state, line_offset, item_heights, viewport_height);
    }
}

fn comment_heights_ready(len: usize, item_heights: &[usize], viewport_height: usize) -> bool {
    if len == 0 || viewport_height == 0 {
        return false;
    }
    if item_heights.is_empty() {
        return false;
    }
    if item_heights.len() != len {
        panic!(
            "comment item heights out of sync: expected {len}, got {}",
            item_heights.len()
        );
    }
    true
}

fn comment_total_lines(item_heights: &[usize]) -> usize {
    item_heights.iter().map(|height| (*height).max(1)).sum()
}

fn comment_line_range(item_heights: &[usize], index: usize) -> (usize, usize) {
    let mut start = 0usize;
    for (idx, height) in item_heights.iter().enumerate() {
        let height = (*height).max(1);
        if idx == index {
            return (start, start + height);
        }
        start += height;
    }
    (start, start)
}

fn page_down_with_heights(state: &mut ListState, item_heights: &[usize], viewport_height: usize) {
    let len = item_heights.len();
    if len == 0 {
        state.select(None);
        *state.offset_mut() = 0;
        return;
    }

    let selected = state.selected().unwrap_or(0).min(len - 1);
    let viewport_height = viewport_height.max(1);

    let mut target = selected;
    let mut used = 0usize;
    while target + 1 < len {
        let next = target + 1;
        let height = item_heights[next].max(1);
        if used == 0 && height >= viewport_height {
            target = next;
            break;
        }
        if used + height > viewport_height {
            break;
        }
        used += height;
        target = next;
    }

    state.select(Some(target));
}

fn page_up_with_heights(state: &mut ListState, item_heights: &[usize], viewport_height: usize) {
    let len = item_heights.len();
    if len == 0 {
        state.select(None);
        *state.offset_mut() = 0;
        return;
    }

    let selected = state.selected().unwrap_or(0).min(len - 1);
    let viewport_height = viewport_height.max(1);

    let mut target = selected;
    let mut used = 0usize;
    while target > 0 {
        let prev = target - 1;
        let height = item_heights[prev].max(1);
        if used == 0 && height >= viewport_height {
            target = prev;
            break;
        }
        if used + height > viewport_height {
            break;
        }
        used += height;
        target = prev;
    }

    state.select(Some(target));
}
