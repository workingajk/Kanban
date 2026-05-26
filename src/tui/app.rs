use std::error::Error;

use ratatui::crossterm::event::KeyEvent;
use ratatui::style::Color;
use ratatui::Frame;
use tui_textarea::TextArea;

use crate::db;

pub(crate) const COLUMN_STYLES: &[(&str, Color)] = &[
    ("backlog", Color::DarkGray),
    ("todo", Color::Cyan),
    ("in-progress", Color::Yellow),
    ("done", Color::Green),
];

pub(crate) struct ColumnState {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) todos: Vec<crate::models::Todo>,
    pub(crate) selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Normal,
    Adding,
    EditingTitle,
    EditingDescription,
    EditingDueDate,
    Searching,
    RecycleBin,
}

pub struct TuiApp {
    pub(crate) columns: Vec<ColumnState>,
    pub(crate) focused_col: usize,
    pub(crate) input_mode: InputMode,
    pub(crate) textarea: TextArea<'static>,
    pub(crate) search_query: String,
    pub(crate) due_date_picker: Option<chrono::NaiveDate>,
    pub(crate) recycle_bin_todos: Vec<crate::models::Todo>,
    pub(crate) recycle_bin_selected: usize,
    pub(crate) edit_todo_id: Option<i64>,
    pub(crate) should_quit: bool,
    pub(crate) status: String,
}

impl TuiApp {
    pub fn load(conn: &rusqlite::Connection) -> Result<Self, Box<dyn Error>> {
        let db_cols = db::list_columns(conn)?;
        let all = db::list_todos(conn, None)?;

        let columns: Vec<ColumnState> = db_cols
            .iter()
            .map(|c| {
                let todos: Vec<_> = all
                    .iter()
                    .filter(|t| t.column_id == c.id)
                    .cloned()
                    .collect();
                ColumnState {
                    id: c.id,
                    name: c.name.clone(),
                    selected: 0,
                    todos,
                }
            })
            .collect();

        Ok(Self {
            columns,
            focused_col: 0,
            input_mode: InputMode::Normal,
            textarea: TextArea::default(),
            search_query: String::new(),
            due_date_picker: None,
            recycle_bin_todos: Vec::new(),
            recycle_bin_selected: 0,
            edit_todo_id: None,
            should_quit: false,
            status: "💡 [/] Search | [Shift+Arrows] Move/Sort | [c] Description | [r] Recycle Bin".to_string(),
        })
    }

    pub(crate) fn reload(&mut self, conn: &rusqlite::Connection) -> Result<(), Box<dyn Error>> {
        let all = db::list_todos(conn, None)?;
        let query = self.search_query.to_lowercase();
        for col in &mut self.columns {
            col.todos = all
                .iter()
                .filter(|t| t.column_id == col.id)
                .filter(|t| {
                    if query.is_empty() {
                        true
                    } else {
                        t.title.to_lowercase().contains(&query)
                            || t.description.to_lowercase().contains(&query)
                    }
                })
                .cloned()
                .collect();
            if !col.todos.is_empty() && col.selected >= col.todos.len() {
                col.selected = col.todos.len() - 1;
            }
        }
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame) {
        super::render::render_app(self, frame);
    }

    pub fn handle_key(
        &mut self,
        conn: &rusqlite::Connection,
        key: KeyEvent,
    ) -> Result<(), Box<dyn Error>> {
        super::handlers::handle_key(self, conn, key)
    }
}
