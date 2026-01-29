use super::{App, AppEvent, StoriesLoadMode, nav};
use crate::logging;

impl App {
    pub(super) fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::StoriesLoaded {
                generation,
                mode,
                stories,
                next_page,
                end_reached,
            } => {
                if generation != self.stories_generation {
                    return;
                }
                self.story_loading = false;
                self.prefetch_in_flight = false;
                self.last_error = None;
                self.story_next_page = next_page.max(1);
                self.story_end_reached = end_reached;

                match mode {
                    StoriesLoadMode::Replace => {
                        self.stories = stories;
                        self.prefetched_comments_cache.clear();
                        self.comment_prefetch_in_flight_ids.clear();
                        self.comment_prefetch_generations.clear();
                        let select_idx = self
                            .pending_story_selection_id
                            .take()
                            .and_then(|id| self.stories.iter().position(|s| s.short_id == id))
                            .unwrap_or(0);
                        self.story_list_state.select(Some(select_idx));
                        *self.story_list_state.offset_mut() = 0;
                    }
                    StoriesLoadMode::Append => {
                        self.stories.extend(stories);
                    }
                }

                nav::ensure_visible(
                    &mut self.story_list_state,
                    self.stories.len(),
                    self.story_page_size,
                );
                self.save_story_list_state_background();
                self.maybe_prefetch_comments();
            }
            AppEvent::CommentsLoaded {
                generation,
                story_id,
                story,
                comments,
            } => {
                if generation != self.comments_generation {
                    return;
                }
                if self
                    .current_story
                    .as_ref()
                    .is_some_and(|s| s.short_id != story_id)
                {
                    return;
                }

                self.apply_comments_for_story(story, comments, false);
            }
            AppEvent::CommentsPrefetched {
                generation,
                story_id,
                story,
                comments,
            } => {
                let expected = self.comment_prefetch_generations.get(&story_id).copied();
                if expected != Some(generation) {
                    return;
                }

                self.comment_prefetch_in_flight_ids.remove(&story_id);
                self.comment_prefetch_generations.remove(&story_id);

                if self
                    .awaiting_prefetch_story_id
                    .as_deref()
                    .is_some_and(|id| id == story_id)
                {
                    self.apply_comments_for_story(story, comments, false);
                    return;
                }

                self.prefetched_comments_cache
                    .insert(story_id, (story, comments));
                self.maybe_prefetch_comments();
            }
            AppEvent::Error {
                generation,
                message,
            } => {
                if generation != self.stories_generation && generation != self.comments_generation {
                    return;
                }
                self.story_loading = false;
                self.prefetch_in_flight = false;
                self.comment_loading = false;
                logging::log_error(format!("load error: {message}"));
                self.last_error = Some(message);
            }
            AppEvent::PrefetchError {
                generation,
                story_id,
                message,
            } => {
                let expected = self.comment_prefetch_generations.get(&story_id).copied();
                if expected != Some(generation) {
                    return;
                }
                self.comment_prefetch_in_flight_ids.remove(&story_id);
                self.comment_prefetch_generations.remove(&story_id);
                if self
                    .awaiting_prefetch_story_id
                    .as_deref()
                    .is_some_and(|id| id == story_id)
                {
                    self.awaiting_prefetch_story_id = None;
                    self.comment_loading = false;
                }
                logging::log_error(format!("prefetch error story_id={story_id}: {message}"));
                self.last_error = Some(message);
                self.maybe_prefetch_comments();
            }
        }
    }
}
