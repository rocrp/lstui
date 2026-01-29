use super::{App, View, nav};
use crate::input::Action;
use anyhow::{Context, Result};

impl App {
    pub(super) fn handle_action(&mut self, action: Action) {
        if action == Action::ToggleHelp {
            self.help_visible = !self.help_visible;
            return;
        }
        if self.help_visible {
            if action == Action::BackOrQuit {
                self.help_visible = false;
            }
            return;
        }

        match (self.view, action) {
            (View::Stories, Action::BackOrQuit) => self.should_quit = true,
            (View::Comments, Action::BackOrQuit) => {
                self.view = View::Stories;
                self.maybe_prefetch_comments();
            }
            (View::Stories, Action::Refresh) => self.refresh_stories(),
            (View::Comments, Action::Refresh) => self.refresh_comments(),

            (View::Stories, Action::Enter)
            | (View::Stories, Action::OpenComments)
            | (View::Stories, Action::Expand) => self.open_comments_for_selected_story(),

            (View::Stories, Action::OpenPrimaryBrowser) => {
                if let Err(err) = self.open_selected_story_in_browser() {
                    self.last_error = Some(format!("{err:#}"));
                }
            }
            (View::Stories, Action::OpenSecondaryBrowser) => {
                if let Err(err) = self.open_selected_story_comments_in_browser() {
                    self.last_error = Some(format!("{err:#}"));
                }
            }
            (View::Comments, Action::OpenPrimaryBrowser) => {
                if let Err(err) = self.open_current_story_comments_in_browser() {
                    self.last_error = Some(format!("{err:#}"));
                }
            }
            (View::Comments, Action::OpenSecondaryBrowser) => {
                if let Err(err) = self.open_current_story_in_browser() {
                    self.last_error = Some(format!("{err:#}"));
                }
            }

            (View::Stories, Action::MoveDown) => {
                nav::move_selection_down(&mut self.story_list_state, self.stories.len());
                nav::ensure_visible(
                    &mut self.story_list_state,
                    self.stories.len(),
                    self.story_page_size,
                );
                self.maybe_prefetch_stories();
                self.maybe_prefetch_comments();
            }
            (View::Stories, Action::MoveUp) => {
                nav::move_selection_up(&mut self.story_list_state);
                nav::ensure_visible(
                    &mut self.story_list_state,
                    self.stories.len(),
                    self.story_page_size,
                );
                self.maybe_prefetch_comments();
            }
            (View::Stories, Action::PageDown) => {
                nav::page_down(
                    &mut self.story_list_state,
                    self.stories.len(),
                    self.story_page_size,
                );
                self.maybe_prefetch_stories();
                self.maybe_prefetch_comments();
            }
            (View::Stories, Action::PageUp) => {
                nav::page_up(&mut self.story_list_state, self.story_page_size);
                self.maybe_prefetch_comments();
            }
            (View::Stories, Action::GoTop) => {
                self.story_list_state.select(Some(0));
                *self.story_list_state.offset_mut() = 0;
                self.maybe_prefetch_comments();
            }
            (View::Stories, Action::GoBottom) => {
                if !self.stories.is_empty() {
                    self.story_list_state.select(Some(self.stories.len() - 1));
                    nav::ensure_visible(
                        &mut self.story_list_state,
                        self.stories.len(),
                        self.story_page_size,
                    );
                    self.maybe_prefetch_stories();
                    self.maybe_prefetch_comments();
                }
            }

            (View::Comments, Action::MoveDown) => {
                let comment_len = self.comment_list.len();
                nav::move_selection_down(&mut self.comment_list_state, comment_len);
                nav::ensure_comment_visible(
                    &mut self.comment_list_state,
                    &mut self.comment_line_offset,
                    comment_len,
                    &self.comment_item_heights,
                    self.comment_viewport_height,
                );
            }
            (View::Comments, Action::MoveUp) => {
                nav::move_selection_up(&mut self.comment_list_state);
                nav::ensure_comment_visible(
                    &mut self.comment_list_state,
                    &mut self.comment_line_offset,
                    self.comment_list.len(),
                    &self.comment_item_heights,
                    self.comment_viewport_height,
                );
            }
            (View::Comments, Action::PageDown) => {
                nav::page_down_comment_list(
                    &mut self.comment_list_state,
                    self.comment_list.len(),
                    self.comment_page_size,
                    &mut self.comment_line_offset,
                    &self.comment_item_heights,
                    self.comment_viewport_height,
                );
            }
            (View::Comments, Action::PageUp) => {
                nav::page_up_comment_list(
                    &mut self.comment_list_state,
                    self.comment_list.len(),
                    self.comment_page_size,
                    &mut self.comment_line_offset,
                    &self.comment_item_heights,
                    self.comment_viewport_height,
                );
            }
            (View::Comments, Action::GoTop) => {
                self.comment_list_state.select(Some(0));
                nav::ensure_comment_visible(
                    &mut self.comment_list_state,
                    &mut self.comment_line_offset,
                    self.comment_list.len(),
                    &self.comment_item_heights,
                    self.comment_viewport_height,
                );
            }
            (View::Comments, Action::GoBottom) => {
                if !self.comment_list.is_empty() {
                    self.comment_list_state
                        .select(Some(self.comment_list.len() - 1));
                    nav::ensure_comment_visible(
                        &mut self.comment_list_state,
                        &mut self.comment_line_offset,
                        self.comment_list.len(),
                        &self.comment_item_heights,
                        self.comment_viewport_height,
                    );
                }
            }
            (View::Comments, Action::Enter) => self.toggle_selected_comment_collapse(),
            (View::Comments, Action::Collapse) => self.collapse_selected_comment(),
            (View::Comments, Action::Expand) => self.expand_selected_comment(),
            (View::Comments, Action::ToggleCollapse) => self.toggle_selected_comment_collapse(),

            (_, _) => {}
        }
    }

    fn open_selected_story_in_browser(&self) -> Result<()> {
        let story = self.selected_story().context("no selected story")?;
        open_story(story)
    }

    fn open_selected_story_comments_in_browser(&self) -> Result<()> {
        let story = self.selected_story().context("no selected story")?;
        open_story_comments(story)
    }

    fn open_current_story_in_browser(&self) -> Result<()> {
        let story = self.current_story.as_ref().context("no current story")?;
        open_story(story)
    }

    fn open_current_story_comments_in_browser(&self) -> Result<()> {
        let story = self.current_story.as_ref().context("no current story")?;
        open_story_comments(story)
    }
}

fn open_story(story: &crate::api::Story) -> Result<()> {
    let url = story
        .url
        .clone()
        .unwrap_or_else(|| story.comments_url.to_string());
    open::that(url).context("open in browser")?;
    Ok(())
}

fn open_story_comments(story: &crate::api::Story) -> Result<()> {
    open::that(&story.comments_url).context("open comments in browser")?;
    Ok(())
}
