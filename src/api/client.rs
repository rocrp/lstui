use crate::Feed;
use crate::api::types::{StoryRaw, StoryWithCommentsRaw, build_comment_tree};
use crate::api::{CommentNode, Story};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub struct LobstersClient {
    base_url: String,
    http: Client,
    semaphore: Arc<Semaphore>,
}

impl LobstersClient {
    pub fn new(base_url: String, concurrency: usize) -> Result<Self> {
        anyhow::ensure!(concurrency > 0, "concurrency must be > 0");

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: Client::builder()
                .pool_max_idle_per_host(10)
                .pool_idle_timeout(Duration::from_secs(30))
                .build()
                .context("build http client")?,
            semaphore: Arc::new(Semaphore::new(concurrency)),
        })
    }

    pub async fn fetch_feed(&self, feed: Feed, count: usize) -> Result<(Vec<Story>, usize, bool)> {
        anyhow::ensure!(count > 0, "count must be > 0");

        let mut page = 1usize;
        let mut out = Vec::new();
        loop {
            let stories = self.fetch_feed_page(feed, page).await?;
            if stories.is_empty() {
                return Ok((out, page, true));
            }
            out.extend(stories);
            if out.len() >= count {
                return Ok((out, page + 1, false));
            }
            page = page.saturating_add(1);
        }
    }

    pub async fn fetch_feed_page(&self, feed: Feed, page: usize) -> Result<Vec<Story>> {
        anyhow::ensure!(page > 0, "page must be > 0");

        let url = format!("{}/{}.json?page={page}", self.base_url, feed.path());
        let raw = self.get_json::<Vec<StoryRaw>>(url).await?;
        raw.into_iter().map(Story::from_raw).collect()
    }

    pub async fn fetch_story_detail(&self, short_id: &str) -> Result<(Story, Vec<CommentNode>)> {
        anyhow::ensure!(!short_id.trim().is_empty(), "short_id must be non-empty");

        let url = format!("{}/s/{short_id}.json", self.base_url);
        let raw = self.get_json::<StoryWithCommentsRaw>(url).await?;
        let story = Story::from_raw(raw.story)?;
        let comments = build_comment_tree(raw.comments)?;
        Ok((story, comments))
    }

    async fn get_json<T: DeserializeOwned>(&self, url: String) -> Result<T> {
        let _permit = self.acquire_permit().await?;
        self.http
            .get(url)
            .send()
            .await
            .context("send request")?
            .error_for_status()
            .context("http status")?
            .json::<T>()
            .await
            .context("decode json")
    }

    async fn acquire_permit(&self) -> Result<OwnedSemaphorePermit> {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .context("acquire http semaphore")
    }
}
