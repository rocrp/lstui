use super::{App, AppEvent};
use crate::Cli;
use crate::api::LobstersClient;
use crate::state::StateStore;
use crate::tui::Tui;
use crate::ui;
use anyhow::{Context, Result};
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

pub async fn run(cli: Cli) -> Result<()> {
    let cache_dir = if cli.no_cache {
        None
    } else {
        Some(match cli.cache_dir.clone() {
            Some(dir) => dir,
            None => {
                let proj = directories::ProjectDirs::from("dev", "lstui", "lstui")
                    .context("resolve OS cache dir")?;
                proj.cache_dir().to_path_buf()
            }
        })
    };
    let state_store = cache_dir.clone().map(StateStore::new);

    let client = LobstersClient::new(cli.base_url.clone(), cli.concurrency)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(cli, client, tx.clone(), state_store.clone());

    if let Some(store) = &state_store {
        if let Some(state) = store.load_story_list_state().await? {
            app.restore_story_list_state(
                state.feed,
                state.next_page,
                state.end_reached,
                state.stories,
            );
        }
    }

    app.refresh_stories();
    app.maybe_prefetch_comments();

    let mut tui = Tui::init()?;
    let mut events = EventStream::new();

    loop {
        tui.draw(|f| ui::render(f, &mut app))?;

        let tick_duration = if app.is_busy() {
            Duration::from_millis(120)
        } else {
            Duration::from_millis(200)
        };

        tokio::select! {
            maybe_event = events.next() => {
                let Some(event) = maybe_event else {
                    return Err(anyhow::anyhow!("crossterm event stream ended unexpectedly"));
                };

                let event = event.context("read terminal event")?;
                match event {
                    Event::Key(key) => app.handle_key(key),
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
            maybe_app_event = rx.recv() => {
                let Some(app_event) = maybe_app_event else {
                    return Err(anyhow::anyhow!("app event channel closed unexpectedly"));
                };
                app.handle_app_event(app_event);
            }
            _ = tokio::time::sleep(tick_duration) => {
                app.tick();
            }
        }

        if app.should_quit() {
            break;
        }
    }

    drop(tui);
    if let Some(store) = &state_store {
        if !app.stories.is_empty() {
            store
                .save_story_list_state(
                    app.cli.feed.path().to_string(),
                    app.story_next_page,
                    app.story_end_reached,
                    app.stories.clone(),
                )
                .await?;
        }
    }

    Ok(())
}
