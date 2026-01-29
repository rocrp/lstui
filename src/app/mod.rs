mod actions;
mod comments;
mod core;
mod handlers;
mod nav;
mod run;
mod stories;

pub use run::run;

use crate::Cli;
use crate::api::{CommentNode, LobstersClient, Story};
use crate::input::KeyState;
use crate::state::StateStore;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Stories,
    Comments,
}

#[derive(Debug)]
enum AppEvent {
    StoriesLoaded {
        generation: u64,
        mode: StoriesLoadMode,
        stories: Vec<Story>,
        next_page: usize,
        end_reached: bool,
    },
    CommentsLoaded {
        generation: u64,
        story_id: String,
        story: Story,
        comments: Vec<CommentNode>,
    },
    CommentsPrefetched {
        generation: u64,
        story_id: String,
        story: Story,
        comments: Vec<CommentNode>,
    },
    Error {
        generation: u64,
        message: String,
    },
    PrefetchError {
        generation: u64,
        story_id: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoriesLoadMode {
    Replace,
    Append,
}

pub struct App {
    pub view: View,
    pub help_visible: bool,

    pub stories: Vec<Story>,
    pub story_list_state: ListState,
    pub story_loading: bool,
    pub story_page_size: usize,
    pub story_next_page: usize,
    pub story_end_reached: bool,
    pub prefetch_in_flight: bool,

    pub current_story: Option<Story>,
    pub comment_tree: Vec<CommentNode>,
    pub comment_list: Vec<crate::api::Comment>,
    pub comment_list_state: ListState,
    pub comment_loading: bool,
    pub comment_page_size: usize,
    pub comment_item_heights: Vec<usize>,
    pub comment_viewport_height: usize,
    pub comment_line_offset: usize,

    pub last_error: Option<String>,
    pub comment_prefetch_in_flight_ids: HashSet<String>,

    pub(crate) cli: Cli,
    client: LobstersClient,
    tx: mpsc::UnboundedSender<AppEvent>,
    state_store: Option<StateStore>,

    stories_generation: u64,
    comments_generation: u64,
    comments_prefetch_generation: u64,
    comment_prefetch_generations: HashMap<String, u64>,
    prefetched_comments_cache: HashMap<String, (Story, Vec<CommentNode>)>,
    awaiting_prefetch_story_id: Option<String>,

    input: KeyState,
    should_quit: bool,
    spinner_idx: usize,
    last_user_activity: Instant,
    pending_story_selection_id: Option<String>,
}
