use anyhow::{Context, Result};
use crossterm::ExecutableCommand;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};
use std::panic;

pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct Tui {
    terminal: TuiTerminal,
}

impl Tui {
    pub fn init() -> Result<Self> {
        enable_raw_mode().context("enable raw mode")?;

        let mut out = stdout();
        out.execute(EnterAlternateScreen)
            .context("enter alternate screen")?;
        out.execute(EnableMouseCapture)
            .context("enable mouse capture")?;

        let backend = CrosstermBackend::new(out);
        let mut terminal = Terminal::new(backend).context("create terminal")?;
        terminal.clear().context("clear terminal")?;

        install_panic_hook();

        Ok(Self { terminal })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f).context("draw frame")?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode().context("disable raw mode")?;

    let mut out = stdout();
    out.execute(DisableMouseCapture)
        .context("disable mouse capture")?;
    out.execute(LeaveAlternateScreen)
        .context("leave alternate screen")?;
    Ok(())
}

fn install_panic_hook() {
    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        prev(info);
    }));
}
