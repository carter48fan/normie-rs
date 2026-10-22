mod app;
mod config;
mod db;
mod events;
mod ui;
mod utils;

use crate::app::App;
use crate::events::{Event, Events};
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, panic, time::Duration};

fn main() -> Result<()> {
    // Setup panic hook to restore terminal
    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_panic(info);
    }));

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App & Event System
    let mut app = App::new();
    let events = Events::new(Duration::from_millis(250));

    // Main Loop
    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            terminal.draw(|f| ui::draw(f, &mut app))?;
            needs_redraw = false;
        }

        match events.next()? {
            Event::Input(key) => {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                app.handle_event(key)?;
                needs_redraw = true;
            }
            Event::Resize => {
                needs_redraw = true;
            }
            Event::Tick => {}
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
