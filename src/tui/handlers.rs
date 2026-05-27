use std::error::Error;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType};
use tui_textarea::{CursorMove, TextArea};

use crate::db;
use super::app::{InputMode, TuiApp};
use super::utils::{add_one_month, subtract_one_month};

pub fn handle_key(
    app: &mut TuiApp,
    conn: &rusqlite::Connection,
    key: KeyEvent,
) -> Result<(), Box<dyn Error>> {
    match app.input_mode {
        InputMode::Normal => handle_normal_key(app, conn, key),
        InputMode::EditingDueDate => handle_date_picker_key(app, conn, key),
        InputMode::RecycleBin => handle_recycle_bin_key(app, conn, key),
        _ => handle_input_key(app, conn, key),
    }
}

fn handle_normal_key(
    app: &mut TuiApp,
    conn: &rusqlite::Connection,
    key: KeyEvent,
) -> Result<(), Box<dyn Error>> {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Up => {
                let col = &app.columns[app.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    if db::move_todo_up(conn, todo.id)? {
                        app.status = format!(" Moved #{} UP", todo.id);
                        app.reload(conn)?;
                        let col_state = &mut app.columns[app.focused_col];
                        if col_state.selected > 0 {
                            col_state.selected -= 1;
                        }
                    }
                }
                return Ok(());
            }
            KeyCode::Down => {
                let col = &app.columns[app.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    if db::move_todo_down(conn, todo.id)? {
                        app.status = format!(" Moved #{} DOWN", todo.id);
                        app.reload(conn)?;
                        let col_state = &mut app.columns[app.focused_col];
                        if col_state.selected + 1 < col_state.todos.len() {
                            col_state.selected += 1;
                        }
                    }
                }
                return Ok(());
            }
            KeyCode::Left => {
                let col = &app.columns[app.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    let todo_id = todo.id;
                    let next = if app.focused_col == 0 {
                        app.columns.len() - 1
                    } else {
                        app.focused_col - 1
                    };
                    let name = &app.columns[next].name;
                    db::move_todo(conn, todo_id, name)?;
                    app.status = format!(" Moved #{} to {}", todo_id, name);
                    app.reload(conn)?;
                    app.focused_col = next;
                    if let Some(pos) = app.columns[next].todos.iter().position(|t| t.id == todo_id) {
                        app.columns[next].selected = pos;
                    }
                }
                return Ok(());
            }
            KeyCode::Right => {
                let col = &app.columns[app.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    let todo_id = todo.id;
                    let next = (app.focused_col + 1) % app.columns.len();
                    let name = &app.columns[next].name;
                    db::move_todo(conn, todo_id, name)?;
                    app.status = format!(" Moved #{} to {}", todo_id, name);
                    app.reload(conn)?;
                    app.focused_col = next;
                    if let Some(pos) = app.columns[next].todos.iter().position(|t| t.id == todo_id) {
                        app.columns[next].selected = pos;
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('a') => {
            app.input_mode = InputMode::Adding;
            app.textarea = TextArea::default();
            app.textarea.set_block(Block::bordered()
                .title(" ➕ Add Todo ")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)));
            app.textarea.set_cursor_line_style(Style::default());
            app.edit_todo_id = None;
            app.status.clear();
        }
        KeyCode::Char('e') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                app.input_mode = InputMode::EditingTitle;
                app.textarea = TextArea::new(vec![todo.title.clone()]);
                app.textarea.set_block(Block::bordered()
                    .title(" ✏️ Edit Title ")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)));
                app.textarea.set_cursor_line_style(Style::default());
                app.textarea.move_cursor(CursorMove::Bottom);
                app.textarea.move_cursor(CursorMove::End);
                app.edit_todo_id = Some(todo.id);
                app.status.clear();
            }
        }
        KeyCode::Char('c') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                app.input_mode = InputMode::EditingDescription;
                let lines: Vec<String> = todo.description.split('\n').map(String::from).collect();
                app.textarea = TextArea::new(lines);
                app.textarea.set_block(Block::bordered()
                    .title(" 📝 Edit Description ")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)));
                app.textarea.set_cursor_line_style(Style::default());
                app.textarea.move_cursor(CursorMove::Bottom);
                app.textarea.move_cursor(CursorMove::End);
                app.edit_todo_id = Some(todo.id);
                app.status.clear();
            }
        }
        KeyCode::Char('t') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                app.input_mode = InputMode::EditingDueDate;
                app.edit_todo_id = Some(todo.id);
                
                let parsed_date = chrono::NaiveDate::parse_from_str(&todo.due_date, "%Y-%m-%d").ok();
                app.due_date_picker = Some(parsed_date.unwrap_or_else(|| chrono::Local::now().date_naive()));
                app.status.clear();
            }
        }
        KeyCode::Char('p') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                let next_priority = match todo.priority.as_str() {
                    "Low" => "Medium",
                    "Medium" => "High",
                    _ => "Low",
                };
                db::update_todo_priority(conn, todo.id, next_priority)?;
                app.status = format!(" Priority of #{} set to {}", todo.id, next_priority);
                app.reload(conn)?;
            }
        }
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Searching;
            app.textarea = TextArea::new(vec![app.search_query.clone()]);
            app.textarea.set_block(Block::bordered()
                .title(" 🔍 Search Board ")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)));
            app.textarea.set_cursor_line_style(Style::default());
            app.textarea.move_cursor(CursorMove::Bottom);
            app.textarea.move_cursor(CursorMove::End);
            app.status.clear();
        }
        KeyCode::Char('d') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                db::delete_todo(conn, todo.id)?;
                app.status = format!(" Deleted #{}", todo.id);
                app.reload(conn)?;
            }
        }
        KeyCode::Char('m') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                let todo_id = todo.id;
                let next = (app.focused_col + 1) % app.columns.len();
                let name = &app.columns[next].name;
                db::move_todo(conn, todo_id, name)?;
                app.status = format!(" Moved #{} to {}", todo_id, name);
                app.reload(conn)?;
                app.focused_col = next;
                if let Some(pos) = app.columns[next].todos.iter().position(|t| t.id == todo_id) {
                    app.columns[next].selected = pos;
                }
            }
        }
        KeyCode::Char('r') => {
            let archived = db::list_archived_todos(conn)?;
            app.input_mode = InputMode::RecycleBin;
            app.recycle_bin_todos = archived;
            app.recycle_bin_selected = 0;
            app.status.clear();
        }
        KeyCode::Char('K') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                if db::move_todo_up(conn, todo.id)? {
                    app.status = format!(" Moved #{} UP", todo.id);
                    app.reload(conn)?;
                    let col_state = &mut app.columns[app.focused_col];
                    if col_state.selected > 0 {
                        col_state.selected -= 1;
                    }
                }
            }
        }
        KeyCode::Char('J') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                if db::move_todo_down(conn, todo.id)? {
                    app.status = format!(" Moved #{} DOWN", todo.id);
                    app.reload(conn)?;
                    let col_state = &mut app.columns[app.focused_col];
                    if col_state.selected + 1 < col_state.todos.len() {
                        col_state.selected += 1;
                    }
                }
            }
        }
        KeyCode::Char('H') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                let todo_id = todo.id;
                let next = if app.focused_col == 0 {
                    app.columns.len() - 1
                } else {
                    app.focused_col - 1
                };
                let name = &app.columns[next].name;
                db::move_todo(conn, todo_id, name)?;
                app.status = format!(" Moved #{} to {}", todo_id, name);
                app.reload(conn)?;
                app.focused_col = next;
                if let Some(pos) = app.columns[next].todos.iter().position(|t| t.id == todo_id) {
                    app.columns[next].selected = pos;
                }
            }
        }
        KeyCode::Char('L') => {
            let col = &app.columns[app.focused_col];
            if let Some(todo) = col.todos.get(col.selected) {
                let todo_id = todo.id;
                let next = (app.focused_col + 1) % app.columns.len();
                let name = &app.columns[next].name;
                db::move_todo(conn, todo_id, name)?;
                app.status = format!(" Moved #{} to {}", todo_id, name);
                app.reload(conn)?;
                app.focused_col = next;
                if let Some(pos) = app.columns[next].todos.iter().position(|t| t.id == todo_id) {
                    app.columns[next].selected = pos;
                }
            }
        }
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            app.focused_col = (app.focused_col + 1) % app.columns.len();
            app.status.clear();
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            app.focused_col = if app.focused_col == 0 {
                app.columns.len() - 1
            } else {
                app.focused_col - 1
            };
            app.status.clear();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let col = &mut app.columns[app.focused_col];
            if !col.todos.is_empty() {
                col.selected = (col.selected + 1) % col.todos.len();
            }
            app.status.clear();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let col = &mut app.columns[app.focused_col];
            if !col.todos.is_empty() {
                col.selected = if col.selected == 0 {
                    col.todos.len() - 1
                } else {
                    col.selected - 1
                };
            }
            app.status.clear();
        }
        _ => {}
    }
    Ok(())
}

fn handle_date_picker_key(
    app: &mut TuiApp,
    conn: &rusqlite::Connection,
    key: KeyEvent,
) -> Result<(), Box<dyn Error>> {
    if let Some(ref mut date) = app.due_date_picker {
        match key.code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.edit_todo_id = None;
                app.due_date_picker = None;
            }
            KeyCode::Enter => {
                let formatted = date.format("%Y-%m-%d").to_string();
                if let Some(id) = app.edit_todo_id {
                    db::update_todo_due_date(conn, id, &formatted)?;
                    app.status = format!(" Due date set to '{}' for #{}", formatted, id);
                }
                app.reload(conn)?;
                app.input_mode = InputMode::Normal;
                app.edit_todo_id = None;
                app.due_date_picker = None;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                *date = date.pred_opt().unwrap();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                *date = date.succ_opt().unwrap();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let mut temp = *date;
                for _ in 0..7 {
                    temp = temp.pred_opt().unwrap();
                }
                *date = temp;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let mut temp = *date;
                for _ in 0..7 {
                    temp = temp.succ_opt().unwrap();
                }
                *date = temp;
            }
            KeyCode::Char('[') | KeyCode::PageUp => {
                *date = subtract_one_month(*date);
            }
            KeyCode::Char(']') | KeyCode::PageDown => {
                *date = add_one_month(*date);
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                *date = chrono::Local::now().date_naive();
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_recycle_bin_key(
    app: &mut TuiApp,
    conn: &rusqlite::Connection,
    key: KeyEvent,
) -> Result<(), Box<dyn Error>> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.recycle_bin_todos.clear();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.recycle_bin_todos.is_empty() {
                app.recycle_bin_selected = (app.recycle_bin_selected + 1) % app.recycle_bin_todos.len();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.recycle_bin_todos.is_empty() {
                app.recycle_bin_selected = if app.recycle_bin_selected == 0 {
                    app.recycle_bin_todos.len() - 1
                } else {
                    app.recycle_bin_selected - 1
                };
            }
        }
        KeyCode::Char('r') | KeyCode::Enter => {
            if let Some(todo) = app.recycle_bin_todos.get(app.recycle_bin_selected) {
                db::restore_todo(conn, todo.id)?;
                app.status = format!(" Restored #{} to board", todo.id);
            }
            app.reload(conn)?;
            app.recycle_bin_todos = db::list_archived_todos(conn)?;
            if app.recycle_bin_todos.is_empty() {
                app.input_mode = InputMode::Normal;
            } else if app.recycle_bin_selected >= app.recycle_bin_todos.len() {
                app.recycle_bin_selected = app.recycle_bin_todos.len() - 1;
            }
        }
        KeyCode::Char('d') => {
            if let Some(todo) = app.recycle_bin_todos.get(app.recycle_bin_selected) {
                db::delete_todo_permanently(conn, todo.id)?;
                app.status = format!(" Permanently deleted #{}", todo.id);
            }
            app.reload(conn)?;
            app.recycle_bin_todos = db::list_archived_todos(conn)?;
            if app.recycle_bin_todos.is_empty() {
                app.input_mode = InputMode::Normal;
            } else if app.recycle_bin_selected >= app.recycle_bin_todos.len() {
                app.recycle_bin_selected = app.recycle_bin_todos.len() - 1;
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_input_key(
    app: &mut TuiApp,
    conn: &rusqlite::Connection,
    key: KeyEvent,
) -> Result<(), Box<dyn Error>> {
    match key.code {
        KeyCode::Esc => {
            if matches!(app.input_mode, InputMode::EditingDescription) {
                let content = app.textarea.lines().join("\n").trim().to_string();
                if let Some(id) = app.edit_todo_id {
                    db::update_todo_description(conn, id, &content)?;
                    app.status = format!(" Description updated for #{}", id);
                }
                app.reload(conn)?;
            }
            app.input_mode = InputMode::Normal;
            app.edit_todo_id = None;
            return Ok(());
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if matches!(app.input_mode, InputMode::EditingDescription) {
                let content = app.textarea.lines().join("\n").trim().to_string();
                if let Some(id) = app.edit_todo_id {
                    db::update_todo_description(conn, id, &content)?;
                    app.status = format!(" Description updated for #{}", id);
                }
                app.reload(conn)?;
            }
            app.input_mode = InputMode::Normal;
            app.edit_todo_id = None;
            return Ok(());
        }
        KeyCode::Enter if !matches!(app.input_mode, InputMode::EditingDescription) => {
            let content = app.textarea.lines().join("\n").trim().to_string();
            match app.input_mode {
                InputMode::Adding => {
                    if !content.is_empty() {
                        let col_name = &app.columns[app.focused_col].name;
                        let todo = db::add_todo(conn, &content, Some(col_name))?;
                        app.status = format!(" Added #{} to {}", todo.id, col_name);
                    }
                }
                InputMode::EditingTitle => {
                    if !content.is_empty() {
                        if let Some(id) = app.edit_todo_id {
                            db::update_todo(conn, id, &content)?;
                            app.status = format!(" Title updated for #{}", id);
                        }
                    }
                }
                InputMode::Searching => {
                    app.search_query = content;
                    app.status = if app.search_query.is_empty() {
                        " Search cleared".to_string()
                    } else {
                        format!(" Filtering: '{}'", app.search_query)
                    };
                }
                _ => {}
            }
            app.reload(conn)?;
            app.input_mode = InputMode::Normal;
            app.edit_todo_id = None;
            return Ok(());
        }
        _ => {}
    }

    app.textarea.input(key);
    Ok(())
}
