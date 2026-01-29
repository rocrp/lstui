use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const LIMIT_1S: usize = 4;
const LIMIT_1M: usize = 30;
const LIMIT_10M: usize = 100;
const LIMIT_1H: usize = 400;

const WINDOW_1S: Duration = Duration::from_secs(1);
const WINDOW_1M: Duration = Duration::from_secs(60);
const WINDOW_10M: Duration = Duration::from_secs(600);
const WINDOW_1H: Duration = Duration::from_secs(3600);

#[derive(Debug)]
pub struct RateLimiter {
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    blocked_until: Instant,
    hits_1s: VecDeque<Instant>,
    hits_1m: VecDeque<Instant>,
    hits_10m: VecDeque<Instant>,
    hits_1h: VecDeque<Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                blocked_until: Instant::now(),
                hits_1s: VecDeque::new(),
                hits_1m: VecDeque::new(),
                hits_10m: VecDeque::new(),
                hits_1h: VecDeque::new(),
            }),
        }
    }

    pub async fn wait(&self) {
        loop {
            let sleep_for = {
                let mut state = self.state.lock().await;
                let now = Instant::now();
                state.prune(now);

                let mut next = state.blocked_until.max(now);
                next = next.max(next_allowed(&state.hits_1s, LIMIT_1S, WINDOW_1S, now));
                next = next.max(next_allowed(&state.hits_1m, LIMIT_1M, WINDOW_1M, now));
                next = next.max(next_allowed(&state.hits_10m, LIMIT_10M, WINDOW_10M, now));
                next = next.max(next_allowed(&state.hits_1h, LIMIT_1H, WINDOW_1H, now));

                if next <= now {
                    state.record(now);
                    None
                } else {
                    Some(next.duration_since(now))
                }
            };

            let Some(sleep_for) = sleep_for else {
                return;
            };
            tokio::time::sleep(sleep_for).await;
        }
    }

    pub async fn throttle_for(&self, duration: Duration) {
        if duration.is_zero() {
            return;
        }
        let until = Instant::now() + duration;
        let mut state = self.state.lock().await;
        if until > state.blocked_until {
            state.blocked_until = until;
        }
    }
}

fn next_allowed(hits: &VecDeque<Instant>, limit: usize, window: Duration, now: Instant) -> Instant {
    if hits.len() < limit {
        return now;
    }
    hits.front()
        .expect("len checked")
        .checked_add(window)
        .expect("instant overflow")
}

impl State {
    fn prune(&mut self, now: Instant) {
        prune_hits(&mut self.hits_1s, WINDOW_1S, now);
        prune_hits(&mut self.hits_1m, WINDOW_1M, now);
        prune_hits(&mut self.hits_10m, WINDOW_10M, now);
        prune_hits(&mut self.hits_1h, WINDOW_1H, now);
    }

    fn record(&mut self, now: Instant) {
        self.hits_1s.push_back(now);
        self.hits_1m.push_back(now);
        self.hits_10m.push_back(now);
        self.hits_1h.push_back(now);
    }
}

fn prune_hits(hits: &mut VecDeque<Instant>, window: Duration, now: Instant) {
    while hits
        .front()
        .is_some_and(|t| now.duration_since(*t) >= window)
    {
        hits.pop_front();
    }
}
