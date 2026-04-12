use std::fmt;
use std::io::Stdout;
use std::io::stdout;

use anyhow::Result;
use crossterm::Command;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableAlternateScroll;

impl Command for EnableAlternateScroll {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007h")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute EnableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableAlternateScroll;

impl Command for DisableAlternateScroll {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007l")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute DisableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

pub struct TerminalGuard {
    terminal: AppTerminal,
    restored: bool,
}

impl TerminalGuard {
    pub fn terminal_mut(&mut self) -> &mut AppTerminal {
        &mut self.terminal
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }

        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            DisableAlternateScroll,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn setup_terminal() -> Result<TerminalGuard> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(
        &mut stdout,
        EnterAlternateScreen,
        EnableAlternateScroll,
        EnableBracketedPaste,
        crossterm::cursor::Hide
    )?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    Ok(TerminalGuard {
        terminal,
        restored: false,
    })
}

#[cfg(test)]
mod tests {
    use super::DisableAlternateScroll;
    use super::EnableAlternateScroll;
    use crossterm::Command;

    #[test]
    fn enable_alternate_scroll_writes_expected_ansi() {
        let mut ansi = String::new();
        EnableAlternateScroll
            .write_ansi(&mut ansi)
            .expect("write ansi");
        assert_eq!(ansi, "\u{1b}[?1007h");
    }

    #[test]
    fn disable_alternate_scroll_writes_expected_ansi() {
        let mut ansi = String::new();
        DisableAlternateScroll
            .write_ansi(&mut ansi)
            .expect("write ansi");
        assert_eq!(ansi, "\u{1b}[?1007l");
    }
}
