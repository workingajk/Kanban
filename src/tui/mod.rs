pub mod app;
pub mod handlers;
pub mod render;
pub mod utils;

use std::error::Error;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

pub use app::TuiApp;

pub fn run_tui(conn: &rusqlite::Connection) -> Result<(), Box<dyn Error>> {
    crate::db::init_db(conn)?;
    let mut app = TuiApp::load(conn)?;

    let mut terminal = ratatui::init();

    while !app.should_quit {
        terminal.draw(|frame| app.render(frame))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_key(conn, key)?;
            }
        }
    }

    ratatui::restore();
    Ok(())
}
