mod app;
mod config;
mod db;
mod events;
mod table;
mod ui;
mod utils;

use crate::app::{App, AppState, DashboardState, FileBrowserState};
use crate::config::Config;
use crate::events::{Event, Events};
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, panic, path::PathBuf, time::Duration};

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
    let mut config = Config::load();
    let mut initial_state = AppState::Dashboard(DashboardState::new());

    // Check CLI argument for direct file or directory opening
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let target_path = PathBuf::from(&args[1]);
        if target_path.is_file() {
            match app::load_normalizer(target_path.clone()) {
                Ok(normalizer) => {
                    config.add_recent(target_path);
                    initial_state = AppState::Normalizer(Box::new(normalizer));
                }
                Err(e) => {
                    let mut dash = DashboardState::new();
                    dash.error_msg = Some(format!("Could not open {:?}: {}", target_path, e));
                    initial_state = AppState::Dashboard(dash);
                }
            }
        } else if target_path.is_dir() {
            initial_state = AppState::FileBrowser(FileBrowserState::new(target_path));
        } else {
            let mut dash = DashboardState::new();
            dash.error_msg = Some(format!("Path not found: {:?}", target_path));
            initial_state = AppState::Dashboard(dash);
        }
    }

    let mut app = App {
        state: initial_state,
        config,
        should_quit: false,
    };
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
