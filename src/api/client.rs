use crate::Feed;
use crate::api::rate_limiter::RateLimiter;
use crate::api::types::{StoryRaw, StoryWithCommentsRaw, build_comment_tree};
use crate::api::{CommentNode, Story};
use crate::logging;
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub struct LobstersClient {
    base_url: String,
    http: Client,
    semaphore: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
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
            rate_limiter: Arc::new(RateLimiter::new()),
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
        let mut attempts = 0usize;
        loop {
            attempts += 1;
            self.rate_limiter.wait().await;
            let _permit = self.acquire_permit().await?;

            let resp = self.http.get(&url).send().await.context("send request")?;

            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                let wait_for = rate_limit_wait(resp.headers()).unwrap_or(Duration::from_secs(1));
                let wait_for = wait_for.max(Duration::from_secs(1));
                self.rate_limiter.throttle_for(wait_for).await;

                let detail = resp.text().await.unwrap_or_default();
                logging::log_info(format!(
                    "http 429 (attempt={attempts}) wait_for={}s url={url} detail={}",
                    wait_for.as_secs(),
                    detail.trim()
                ));
                continue;
            }

            let resp = resp.error_for_status().context("http status")?;
            return resp.json::<T>().await.context("decode json");
        }
    }

    async fn acquire_permit(&self) -> Result<OwnedSemaphorePermit> {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .context("acquire http semaphore")
    }
}

fn rate_limit_wait(headers: &HeaderMap) -> Option<Duration> {
    parse_retry_after(headers).or_else(|| parse_ratelimit_reset(headers))
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?;
    let seconds = value.parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds))
}

fn parse_ratelimit_reset(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get("ratelimit-reset")?.to_str().ok()?;
    let reset = value.parse::<u64>().ok()?;

    let now_epoch = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let wait_seconds = if reset > 10_000_000 {
        reset.saturating_sub(now_epoch)
    } else {
        reset
    };
    Some(Duration::from_secs(wait_seconds))
}
