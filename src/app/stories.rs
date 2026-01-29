use super::{App, AppEvent, StoriesLoadMode};
use crate::logging;

impl App {
    pub(super) fn restore_story_list_state(
        &mut self,
        feed: String,
        next_page: usize,
        end_reached: bool,
        stories: Vec<crate::api::Story>,
    ) {
        if feed != self.cli.feed.path() {
            self.last_error = Some(format!(
                "refusing to restore story list state: feed mismatch (state={feed}, cli={})",
                self.cli.feed.path()
            ));
            return;
        }
        if stories.is_empty() {
            self.last_error = Some("refusing to restore empty story list state".to_string());
            return;
        }
        if next_page == 0 {
            self.last_error = Some("refusing to restore next_page=0".to_string());
            return;
        }

        self.stories = stories;
        self.story_loading = false;
        self.prefetch_in_flight = false;
        self.story_next_page = next_page;
        self.story_end_reached = end_reached;
        self.story_list_state.select(Some(0));
        *self.story_list_state.offset_mut() = 0;
    }

    pub(super) fn save_story_list_state_background(&self) {
        let Some(store) = self.state_store.clone() else {
            return;
        };
        if self.stories.is_empty() {
            return;
        }

        let feed = self.cli.feed.path().to_string();
        let next_page = self.story_next_page;
        let end_reached = self.story_end_reached;
        let stories = self.stories.clone();
        tokio::spawn(async move {
            if let Err(err) = store
                .save_story_list_state(feed, next_page, end_reached, stories)
                .await
            {
                logging::log_error(format!("failed to save story list state: {err:#}"));
            }
        });
    }

    pub(super) fn refresh_stories(&mut self) {
        self.stories_generation = self.stories_generation.wrapping_add(1);
        let generation = self.stories_generation;

        self.pending_story_selection_id = self.selected_story().map(|s| s.short_id.to_string());

        self.last_error = None;
        self.story_loading = true;
        self.prefetch_in_flight = false;
        self.story_end_reached = false;
        self.story_next_page = 1;
        if self.stories.is_empty() {
            self.story_list_state.select(Some(0));
            *self.story_list_state.offset_mut() = 0;
        }

        let feed = self.cli.feed;
        let count = self.cli.count;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.fetch_feed(feed, count).await;
            match res {
                Ok((stories, next_page, end_reached)) => {
                    let _ = tx.send(AppEvent::StoriesLoaded {
                        generation,
                        mode: StoriesLoadMode::Replace,
                        stories,
                        next_page,
                        end_reached,
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

    pub fn maybe_prefetch_stories(&mut self) {
        if self.story_loading || self.prefetch_in_flight || self.story_end_reached {
            return;
        }
        if self.stories.is_empty() {
            return;
        }

        let selected = self.story_list_state.selected().unwrap_or(0);
        let loaded = self.stories.len();
        let should_fill_viewport = loaded < self.story_page_size;
        let should_prefetch =
            should_fill_viewport || selected.saturating_mul(10) >= loaded.saturating_mul(8);
        if !should_prefetch {
            return;
        }

        self.prefetch_in_flight = true;
        let generation = self.stories_generation;
        let feed = self.cli.feed;
        let page = self.story_next_page;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.fetch_feed_page(feed, page).await;
            match res {
                Ok(stories) => {
                    let end_reached = stories.is_empty();
                    let next_page = if end_reached { page } else { page + 1 };
                    let _ = tx.send(AppEvent::StoriesLoaded {
                        generation,
                        mode: StoriesLoadMode::Append,
                        stories,
                        next_page,
                        end_reached,
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
}
