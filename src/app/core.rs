use super::{App, nav};
use crate::Cli;
use crate::api::{LobstersClient, Story};
use crate::input::KeyState;
use crate::state::StateStore;
use crossterm::event::KeyEventKind;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::sync::mpsc;

impl App {
    pub(super) fn new(
        cli: Cli,
        client: LobstersClient,
        tx: mpsc::UnboundedSender<super::AppEvent>,
        state_store: Option<StateStore>,
    ) -> Self {
        let mut story_list_state = ListState::default();
        story_list_state.select(Some(0));

        let mut comment_list_state = ListState::default();
        comment_list_state.select(Some(0));

        Self {
            view: super::View::Stories,
            help_visible: false,

            stories: Vec::new(),
            story_list_state,
            story_loading: false,
            story_page_size: 10,
            story_next_page: 1,
            story_end_reached: false,
            prefetch_in_flight: false,

            current_story: None,
            comment_tree: Vec::new(),
            comment_list: Vec::new(),
            comment_list_state,
            comment_loading: false,
            comment_page_size: 10,
            comment_item_heights: Vec::new(),
            comment_viewport_height: 0,
            comment_line_offset: 0,

            last_error: None,
            comment_prefetch_in_flight_ids: HashSet::new(),

            cli,
            client,
            tx,
            state_store,

            stories_generation: 0,
            comments_generation: 0,
            comments_prefetch_generation: 0,
            comment_prefetch_generations: HashMap::new(),
            prefetched_comments_cache: HashMap::new(),
            awaiting_prefetch_story_id: None,

            input: KeyState::default(),
            should_quit: false,
            spinner_idx: 0,
            last_user_activity: Instant::now(),
            pending_story_selection_id: None,
        }
    }

    pub fn spinner_frame(&self) -> char {
        const FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        FRAMES[self.spinner_idx % FRAMES.len()]
    }

    pub(super) fn tick(&mut self) {
        if self.is_busy() {
            self.spinner_idx = self.spinner_idx.wrapping_add(1);
        }
        self.maybe_prefetch_comments();
    }

    pub(super) fn is_busy(&self) -> bool {
        self.story_loading
            || self.prefetch_in_flight
            || self.comment_loading
            || !self.comment_prefetch_in_flight_ids.is_empty()
    }

    pub(super) fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn ensure_comment_line_offset(&mut self) {
        nav::ensure_comment_line_offset(
            &mut self.comment_list_state,
            &mut self.comment_line_offset,
            &self.comment_item_heights,
            self.comment_viewport_height,
        );
    }

    pub(super) fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.last_user_activity = Instant::now();
        if let Some(action) = self.input.on_key(key) {
            self.handle_action(action);
        }
    }

    pub fn selected_story(&self) -> Option<&Story> {
        let idx = self.story_list_state.selected().unwrap_or(0);
        self.stories.get(idx)
    }

    pub fn is_comment_prefetching_for_story(&self, story_id: &str) -> bool {
        self.comment_prefetch_in_flight_ids.contains(story_id)
    }
}
