use super::{App, AppEvent, View, nav};
use crate::api::{CommentNode, Story};
use crate::ui::theme;
use std::time::Duration;

const IDLE_PREFETCH_DELAY: Duration = Duration::from_millis(500);
const MAX_COMMENT_PREFETCH_IN_FLIGHT: usize = 3;

impl App {
    pub(super) fn refresh_comments(&mut self) {
        let Some(story) = self.current_story.clone() else {
            self.last_error = Some("no current story".to_string());
            return;
        };
        self.load_comments_for_story(story, true);
    }

    pub(super) fn maybe_prefetch_comments(&mut self) {
        if self.view != View::Stories {
            return;
        }
        if self.comment_prefetch_in_flight_ids.len() >= MAX_COMMENT_PREFETCH_IN_FLIGHT {
            return;
        }
        if self.story_loading && self.stories.is_empty() {
            return;
        }
        if !self.is_idle_for_prefetch() {
            return;
        }

        let candidates = self.prefetch_story_candidates();
        if candidates.is_empty() {
            return;
        }

        for story in candidates {
            if self.comment_prefetch_in_flight_ids.len() >= MAX_COMMENT_PREFETCH_IN_FLIGHT {
                break;
            }
            self.start_comment_prefetch(story);
        }
    }

    fn start_comment_prefetch(&mut self, story: Story) {
        self.comments_prefetch_generation = self.comments_prefetch_generation.wrapping_add(1);
        let generation = self.comments_prefetch_generation;

        self.comment_prefetch_in_flight_ids
            .insert(story.short_id.to_string());
        self.comment_prefetch_generations
            .insert(story.short_id.to_string(), generation);

        let story_id = story.short_id.to_string();
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.fetch_story_detail(&story_id).await;
            match res {
                Ok((story, comments)) => {
                    let _ = tx.send(AppEvent::CommentsPrefetched {
                        generation,
                        story_id,
                        story,
                        comments,
                    });
                }
                Err(err) => {
                    let _ = tx.send(AppEvent::PrefetchError {
                        generation,
                        story_id,
                        message: format!("{err:#}"),
                    });
                }
            }
        });
    }

    pub(super) fn open_comments_for_selected_story(&mut self) {
        let Some(story) = self.selected_story().cloned() else {
            return;
        };

        if self
            .current_story
            .as_ref()
            .is_some_and(|s| s.short_id == story.short_id)
            && !self.comment_tree.is_empty()
        {
            self.view = View::Comments;
            return;
        }

        if let Some((story, comments)) = self.prefetched_comments_cache.remove(&story.short_id) {
            self.apply_comments_for_story(story, comments, true);
            return;
        }

        if self
            .comment_prefetch_in_flight_ids
            .contains(&story.short_id)
        {
            self.awaiting_prefetch_story_id = Some(story.short_id.to_string());
            self.view = View::Comments;
            self.last_error = None;
            let is_same_story = self
                .current_story
                .as_ref()
                .is_some_and(|current| current.short_id == story.short_id);
            self.current_story = Some(story);
            self.comment_loading = true;
            if !is_same_story {
                self.reset_comment_state();
            }
            return;
        }

        self.load_comments_for_story(story, true);
    }

    fn load_comments_for_story(&mut self, story: Story, switch_view: bool) {
        self.comments_generation = self.comments_generation.wrapping_add(1);
        let generation = self.comments_generation;
        self.awaiting_prefetch_story_id = None;

        if switch_view {
            self.view = View::Comments;
        }

        self.last_error = None;
        let is_same_story = self
            .current_story
            .as_ref()
            .is_some_and(|current| current.short_id == story.short_id);
        self.current_story = Some(story.clone());
        self.comment_loading = true;
        if !is_same_story {
            self.reset_comment_state();
        }

        let story_id = story.short_id.to_string();
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.fetch_story_detail(&story_id).await;
            match res {
                Ok((story, comments)) => {
                    let _ = tx.send(AppEvent::CommentsLoaded {
                        generation,
                        story_id,
                        story,
                        comments,
                    });
                }
                Err(err) => {
                    let _ = tx.send(AppEvent::Error {
                        generation,
                        message: format!("{err:#}"),
                    });
                }
            }
        });
    }

    pub(super) fn apply_comments_for_story(
        &mut self,
        story: Story,
        comments: Vec<CommentNode>,
        switch: bool,
    ) {
        if switch {
            self.view = View::Comments;
        }
        self.awaiting_prefetch_story_id = None;
        self.comment_loading = false;
        self.last_error = None;
        self.current_story = Some(story);
        self.comment_tree = comments;
        self.apply_default_comment_expansion();
        self.rebuild_comment_list(None);
        self.comment_list_state.select(Some(0));
        self.comment_line_offset = 0;
        *self.comment_list_state.offset_mut() = 0;
    }

    fn reset_comment_state(&mut self) {
        self.comment_tree.clear();
        self.comment_list.clear();
        self.comment_item_heights.clear();
        self.comment_line_offset = 0;
        self.comment_list_state.select(Some(0));
        *self.comment_list_state.offset_mut() = 0;
    }

    fn is_idle_for_prefetch(&self) -> bool {
        self.last_user_activity.elapsed() >= IDLE_PREFETCH_DELAY
    }

    fn prefetch_story_candidates(&self) -> Vec<Story> {
        let len = self.stories.len();
        if len == 0 {
            return Vec::new();
        }

        let offset = self.story_list_state.offset().min(len);
        let page_size = self.story_page_size.max(1);
        let end = (offset + page_size).min(len);
        let selected = self.story_list_state.selected().unwrap_or(offset);

        let mut indices = (offset..end).collect::<Vec<_>>();
        indices.sort_by_key(|idx| idx.abs_diff(selected));

        let mut out = Vec::new();
        for idx in indices {
            let Some(story) = self.stories.get(idx) else {
                continue;
            };
            if !self.can_prefetch_story(story) {
                continue;
            }
            out.push(story.clone());
        }

        out
    }

    fn can_prefetch_story(&self, story: &Story) -> bool {
        if story.comment_count <= 0 {
            return false;
        }
        if self.prefetched_comments_cache.contains_key(&story.short_id) {
            return false;
        }
        if self
            .comment_prefetch_in_flight_ids
            .contains(&story.short_id)
        {
            return false;
        }
        true
    }

    fn rebuild_comment_list(&mut self, preserve_comment_id: Option<&str>) {
        fn walk(nodes: &[CommentNode], out: &mut Vec<crate::api::Comment>) {
            for node in nodes {
                out.push(node.comment.clone());
                if !node.comment.collapsed {
                    walk(&node.children, out);
                }
            }
        }

        let mut flat = Vec::new();
        walk(&self.comment_tree, &mut flat);
        self.comment_list = flat;
        self.comment_item_heights.clear();

        let Some(id) = preserve_comment_id else {
            return;
        };
        if let Some(idx) = self.comment_list.iter().position(|c| c.id == id) {
            self.comment_list_state.select(Some(idx));
        }
    }

    fn apply_default_comment_expansion(&mut self) {
        let visible_levels = theme::layout().comment_default_visible_levels;
        let expand_depth_exclusive = visible_levels.saturating_sub(1);

        fn walk(nodes: &mut [CommentNode], expand_depth_exclusive: usize) {
            for node in nodes {
                if node.comment.depth < expand_depth_exclusive && !node.comment.kids.is_empty() {
                    node.comment.collapsed = false;
                }
                if !node.children.is_empty() {
                    walk(&mut node.children, expand_depth_exclusive);
                }
            }
        }

        walk(&mut self.comment_tree, expand_depth_exclusive);
    }

    pub(super) fn collapse_selected_comment(&mut self) {
        let Some(selected) = self.comment_list_state.selected() else {
            return;
        };
        let Some(comment) = self.comment_list.get(selected) else {
            return;
        };
        if comment.kids.is_empty() || comment.collapsed {
            return;
        }

        let id = comment.id.to_string();
        if set_collapse_in_tree(&mut self.comment_tree, &id, true).is_none() {
            self.last_error = Some(format!("comment not found id={id}"));
            return;
        }

        self.rebuild_comment_list(Some(&id));
        nav::ensure_comment_visible(
            &mut self.comment_list_state,
            &mut self.comment_line_offset,
            self.comment_list.len(),
            &self.comment_item_heights,
            self.comment_viewport_height,
        );
    }

    pub(super) fn expand_selected_comment(&mut self) {
        let Some(selected) = self.comment_list_state.selected() else {
            return;
        };
        let Some(comment) = self.comment_list.get(selected) else {
            return;
        };
        if comment.kids.is_empty() {
            return;
        }

        let id = comment.id.to_string();
        if set_collapse_in_tree(&mut self.comment_tree, &id, false).is_none() {
            self.last_error = Some(format!("comment not found id={id}"));
            return;
        }

        self.rebuild_comment_list(Some(&id));
        nav::ensure_comment_visible(
            &mut self.comment_list_state,
            &mut self.comment_line_offset,
            self.comment_list.len(),
            &self.comment_item_heights,
            self.comment_viewport_height,
        );
    }

    pub(super) fn toggle_selected_comment_collapse(&mut self) {
        let Some(selected) = self.comment_list_state.selected() else {
            return;
        };
        let Some(comment) = self.comment_list.get(selected) else {
            return;
        };
        if comment.kids.is_empty() {
            return;
        }
        if comment.collapsed {
            self.expand_selected_comment();
        } else {
            self.collapse_selected_comment();
        }
    }
}

fn set_collapse_in_tree(tree: &mut [CommentNode], target: &str, collapsed: bool) -> Option<()> {
    for node in tree {
        if node.comment.id == target {
            node.comment.collapsed = collapsed;
            return Some(());
        }
        if set_collapse_in_tree(&mut node.children, target, collapsed).is_some() {
            return Some(());
        }
    }
    None
}
