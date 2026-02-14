mod api;
mod app;
mod input;
mod logging;
mod state;
mod tui;
mod ui;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Feed {
    Hottest,
    Newest,
    Active,
}

impl Feed {
    pub fn path(self) -> &'static str {
        match self {
            Feed::Hottest => "hottest",
            Feed::Newest => "newest",
            Feed::Active => "active",
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "lstui", about = "Lobsters TUI")]
pub struct Cli {
    /// Feed: hottest|newest|active.
    #[arg(long, value_enum, default_value_t = Feed::Hottest)]
    pub feed: Feed,

    /// Minimum initial number of stories to load.
    #[arg(long, default_value_t = 30)]
    pub count: usize,

    /// Max simultaneous HTTP requests.
    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,

    /// Disable the on-disk state cache (story list state).
    #[arg(long, default_value_t = false)]
    pub no_cache: bool,

    /// Directory for the on-disk state cache (defaults to OS cache dir).
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,

    /// Log file path (disabled by default).
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// Lobsters base URL.
    #[arg(long, default_value = "https://lobste.rs")]
    pub base_url: String,

    /// UI config file path (optional; will search defaults).
    #[arg(long)]
    pub ui_config: Option<PathBuf>,
}

impl Cli {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.count > 0, "--count must be > 0");
        anyhow::ensure!(self.concurrency > 0, "--concurrency must be > 0");
        anyhow::ensure!(
            !self.base_url.trim().is_empty(),
            "--base-url must be non-empty"
        );
        if let Some(path) = &self.ui_config {
            anyhow::ensure!(
                !path.as_os_str().is_empty(),
                "--ui-config must be non-empty"
            );
        }
        if let Some(path) = &self.log_file {
            anyhow::ensure!(!path.as_os_str().is_empty(), "--log-file must be non-empty");
        }
        Ok(())
    }
}

fn ui_config_candidates(cli: &Cli) -> Vec<PathBuf> {
    if let Some(path) = &cli.ui_config {
        return vec![path.clone()];
    }

    let mut candidates = Vec::new();
    let cwd = PathBuf::from("ui-config.toml");
    if !candidates.contains(&cwd) {
        candidates.push(cwd);
    }

    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let exe_cfg = exe_dir.join("ui-config.toml");
        if !candidates.contains(&exe_cfg) {
            candidates.push(exe_cfg);
        }
    }

    if let Some(proj) = directories::ProjectDirs::from("dev", "lstui", "lstui") {
        let cfg = proj.config_dir().join("ui-config.toml");
        if !candidates.contains(&cfg) {
            candidates.push(cfg);
        }
    }

    candidates
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.validate()?;
    logging::init(cli.log_file.clone()).context("init logging")?;
    let ui_candidates = ui_config_candidates(&cli);
    let allow_default = cli.ui_config.is_none();
    ui::theme::init_from_candidates(&ui_candidates, allow_default)
        .with_context(|| "load ui config")?;
    app::run(cli).await
}
