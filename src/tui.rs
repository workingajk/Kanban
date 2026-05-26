use std::error::Error;

use chrono::Datelike;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use tui_textarea::{CursorMove, TextArea};

use crate::db;

const COLUMN_STYLES: &[(&str, Color)] = &[
    ("backlog", Color::DarkGray),
    ("todo", Color::Cyan),
    ("in-progress", Color::Yellow),
    ("done", Color::Green),
];

struct ColumnState {
    id: i64,
    name: String,
    todos: Vec<crate::models::Todo>,
    selected: usize,
}

enum InputMode {
    Normal,
    Adding,
    EditingTitle,
    EditingDescription,
    EditingDueDate,
    Searching,
    RecycleBin,
}

pub struct TuiApp {
    columns: Vec<ColumnState>,
    focused_col: usize,
    input_mode: InputMode,
    textarea: TextArea<'static>,
    search_query: String,
    due_date_picker: Option<chrono::NaiveDate>,
    recycle_bin_todos: Vec<crate::models::Todo>,
    recycle_bin_selected: usize,
    edit_todo_id: Option<i64>,
    should_quit: bool,
    status: String,
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

    fn reload(&mut self, conn: &rusqlite::Connection) -> Result<(), Box<dyn Error>> {
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
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        self.render_header(frame, header);
        self.render_body(frame, body);
        self.render_footer(frame, footer);

        if !matches!(self.input_mode, InputMode::Normal) {
            self.render_input_popup(frame, frame.area());
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let mut total = 0;
        let mut backlog = 0;
        let mut todo = 0;
        let mut in_progress = 0;
        let mut done = 0;

        for col in &self.columns {
            let count = col.todos.len();
            total += count;
            match col.name.as_str() {
                "backlog" => backlog = count,
                "todo" => todo = count,
                "in-progress" => in_progress = count,
                "done" => done = count,
                _ => {}
            }
        }

        let today_str = chrono::Local::now().format("%A, %Y-%m-%d").to_string();

        let [title_area, search_area, stats_area] = Layout::horizontal([
            Constraint::Length(38),
            Constraint::Fill(1),
            Constraint::Length(65),
        ])
        .areas(area);

        let logo_line = Line::from(vec![
            Span::styled(" 🚀 KANBAN ", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  📅 {} ", today_str), Style::default().fg(Color::LightCyan).dim()),
        ]);

        frame.render_widget(
            Paragraph::new(logo_line),
            title_area,
        );

        let search_text = if !self.search_query.is_empty() || matches!(self.input_mode, InputMode::Searching) {
            format!("🔍 Search: {} ", self.search_query)
        } else {
            "".to_string()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(search_text, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ])),
            search_area,
        );

        let stats_line = Line::from(vec![
            Span::styled("Total: ", Style::default().dim()),
            Span::styled(format!("{} ", total), Style::default().add_modifier(Modifier::BOLD).fg(Color::White)),
            Span::raw(" | "),
            Span::styled("📋 Backlog: ", Style::default().dim()),
            Span::styled(format!("{} ", backlog), Style::default().add_modifier(Modifier::BOLD).fg(Color::DarkGray)),
            Span::raw(" | "),
            Span::styled("📥 Todo: ", Style::default().dim()),
            Span::styled(format!("{} ", todo), Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled("⚡ In Progress: ", Style::default().dim()),
            Span::styled(format!("{} ", in_progress), Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)),
            Span::raw(" | "),
            Span::styled("✅ Done: ", Style::default().dim()),
            Span::styled(format!("{} ", done), Style::default().add_modifier(Modifier::BOLD).fg(Color::Green)),
        ]);

        frame.render_widget(
            Paragraph::new(stats_line).alignment(Alignment::Right),
            stats_area,
        );
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::horizontal([
            Constraint::Percentage(73),
            Constraint::Percentage(27),
        ])
        .split(area);

        let board_area = chunks[0];
        let detail_area = chunks[1];

        let n = self.columns.len() as u16;
        let col_areas = Layout::horizontal(vec![Constraint::Ratio(1, n as u32); n as usize])
            .spacing(1)
            .split(board_area);

        let focused = self.focused_col;
        for (i, col) in self.columns.iter_mut().enumerate() {
            col.render(frame, col_areas[i], i == focused);
        }

        self.render_details_panel(frame, detail_area);
    }

    fn render_details_panel(&self, frame: &mut Frame, area: Rect) {
        let active_col = &self.columns[self.focused_col];
        let selected_todo = active_col.todos.get(active_col.selected);

        let block = Block::bordered()
            .title(" 🔍 Task Details ")
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        if let Some(todo) = selected_todo {
            let prio_style = match todo.priority.as_str() {
                "High" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                "Medium" => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                "Low" => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                _ => Style::default().fg(Color::White),
            };

            let due_str = if todo.due_date.is_empty() {
                "Not set".to_string()
            } else {
                todo.due_date.clone()
            };

            let created_str = todo.created_at.format("%Y-%m-%d %H:%M").to_string();

            let inner_area = block.inner(area);
            frame.render_widget(block, area);

            let details_chunks = Layout::vertical([
                Constraint::Length(3), // ID & Title
                Constraint::Length(1), // Divider
                Constraint::Length(1), // Column Status
                Constraint::Length(1), // Priority
                Constraint::Length(1), // Due Date
                Constraint::Length(1), // Created At
                Constraint::Length(1), // Divider
                Constraint::Fill(1),   // Description
            ])
            .split(inner_area);

            let id_title = Line::from(vec![
                Span::styled(format!("#{} ", todo.id), Style::default().dim()),
                Span::styled(&todo.title, Style::default().add_modifier(Modifier::BOLD).fg(Color::White)),
            ]);
            frame.render_widget(Paragraph::new(id_title).wrap(Wrap { trim: true }), details_chunks[0]);

            frame.render_widget(Paragraph::new("─".repeat(inner_area.width as usize)).dim(), details_chunks[1]);

            let status_line = Line::from(vec![
                Span::styled("Status:   ", Style::default().dim()),
                Span::styled(&todo.column_name, Style::default().fg(Color::Magenta)),
            ]);
            frame.render_widget(Paragraph::new(status_line), details_chunks[2]);

            let prio_line = Line::from(vec![
                Span::styled("Priority: ", Style::default().dim()),
                Span::styled(format!("[{}]", todo.priority.to_uppercase()), prio_style),
            ]);
            frame.render_widget(Paragraph::new(prio_line), details_chunks[3]);

            let due_line = Line::from(vec![
                Span::styled("Due Date: ", Style::default().dim()),
                Span::styled(due_str, Style::default().fg(Color::LightCyan)),
            ]);
            frame.render_widget(Paragraph::new(due_line), details_chunks[4]);

            let created_line = Line::from(vec![
                Span::styled("Created:  ", Style::default().dim()),
                Span::styled(created_str, Style::default().dim()),
            ]);
            frame.render_widget(Paragraph::new(created_line), details_chunks[5]);

            frame.render_widget(Paragraph::new("─".repeat(inner_area.width as usize)).dim(), details_chunks[6]);

            let desc_title = Line::from(vec![
                Span::styled("Description:", Style::default().add_modifier(Modifier::UNDERLINED).dim()),
            ]);
            let desc_body = if todo.description.is_empty() {
                "\n  No description. Press [c] to add one."
            } else {
                &todo.description
            };
            
            let desc_paragraph = Paragraph::new(format!("{}\n\n{}", desc_title.to_string(), desc_body))
                .wrap(Wrap { trim: false });
            frame.render_widget(desc_paragraph, details_chunks[7]);

        } else {
            let paragraph = Paragraph::new("\n\n  No card selected\n  in this column.")
                .block(block)
                .alignment(Alignment::Center)
                .dim();
            frame.render_widget(paragraph, area);
        }
    }
}

impl ColumnState {
    fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let color = COLUMN_STYLES
            .iter()
            .find(|(n, _)| *n == self.name)
            .map(|(_, c)| *c)
            .unwrap_or(Color::Reset);

        let title_name = match self.name.as_str() {
            "backlog" => "📋 BACKLOG",
            "todo" => "📥 TODO",
            "in-progress" => "⚡ IN PROGRESS",
            "done" => "✅ DONE",
            _ => &self.name,
        };
        let title = format!(" {} ({}) ", title_name, self.todos.len());

        // Calculate maximum allowed characters for card title based on column width
        let max_title_len = area.width.saturating_sub(10) as usize;

        let items: Vec<ListItem> = self
            .todos
            .iter()
            .map(|t| {
                let prio_span = match t.priority.as_str() {
                    "High" => Span::styled(" [H]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    "Medium" => Span::styled(" [M]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    "Low" => Span::styled(" [L]", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    _ => Span::raw(""),
                };

                let wrapped_lines = wrap_text(&t.title, max_title_len);
                let mut list_lines = Vec::new();

                for (idx, line) in wrapped_lines.iter().enumerate() {
                    if idx == 0 {
                        list_lines.push(Line::from(vec![
                            Span::styled(format!("#{} ", t.id), Style::default().dim()),
                            Span::raw(line.clone()),
                        ]));
                    } else {
                        let id_padding = " ".repeat(format!("#{} ", t.id).len());
                        list_lines.push(Line::from(vec![
                            Span::raw(id_padding),
                            Span::raw(line.clone()),
                        ]));
                    }
                }

                if let Some(last_line) = list_lines.last_mut() {
                    last_line.spans.push(prio_span);
                }

                ListItem::new(list_lines)
            })
            .collect();

        let mut state = ratatui::widgets::ListState::default();
        if !self.todos.is_empty() {
            state.select(Some(self.selected));
        }

        let list = List::new(items)
            .block(
                Block::bordered()
                    .title(title)
                    .border_style(if focused {
                        Style::default().fg(color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(color)
                    })
                    .border_type(if focused {
                        BorderType::Thick
                    } else {
                        BorderType::Plain
                    }),
            )
            .highlight_style(
                Style::default()
                    .bg(color)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut state);
    }
}

impl TuiApp {
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let line = match &self.input_mode {
            InputMode::Normal => {
                if !self.status.is_empty() {
                    Line::from(vec![
                        Span::styled("✨ Status: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(&self.status, Style::default().fg(Color::White)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(" [Tab]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" Col "),
                        Span::styled("[↑↓]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" Sel "),
                        Span::styled("[a]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" Add "),
                        Span::styled("[e]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" Title "),
                        Span::styled("[c]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" Desc "),
                        Span::styled("[p]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" Prio "),
                        Span::styled("[t]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" Due "),
                        Span::styled("[d]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" Del "),
                        Span::styled("[Shift+↑↓]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" Sort "),
                        Span::styled("[Shift+←→]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" Move "),
                        Span::styled("[/]", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw(" Search "),
                        Span::styled("[r]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" Bin "),
                        Span::styled("[q]", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
                        Span::raw(" Quit"),
                    ])
                }
            }
            _ => {
                Line::from(vec![
                    Span::styled(" [Enter]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(" Confirm  "),
                    Span::styled("[Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::raw(" Cancel"),
                ])
            }
        };

        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_input_popup(&self, frame: &mut Frame, area: Rect) {
        if matches!(self.input_mode, InputMode::EditingDueDate) {
            self.render_weekly_date_picker(frame, area);
            return;
        }
        if matches!(self.input_mode, InputMode::RecycleBin) {
            self.render_recycle_bin(frame, area);
            return;
        }

        let (width_pct, height) = match self.input_mode {
            InputMode::EditingDescription => (70, 10),
            _ => (50, 5),
        };

        let popup = centered_rect(area, width_pct, height);
        frame.render_widget(Clear, popup); // Clear background to make it opaque
        frame.render_widget(&self.textarea, popup);
    }

    fn render_weekly_date_picker(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(area, 60, 6);
        frame.render_widget(Clear, popup); // Clear background to make it opaque
        let block = Block::bordered()
            .title(" 📅 Select Due Date ")
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        if let Some(selected_date) = self.due_date_picker {
            let year = selected_date.year();
            let month = selected_date.month();
            
            let month_name = match month {
                1 => "January", 2 => "February", 3 => "March", 4 => "April",
                5 => "May", 6 => "June", 7 => "July", 8 => "August",
                9 => "September", 10 => "October", 11 => "November", 12 => "December",
                _ => "",
            };

            let header_str = format!("◄  {} {}  ►", month_name, year);
            let weekday_header = "  Su   Mo   Tu   We   Th   Fr   Sa  ";

            let weekday = selected_date.weekday();
            let days_from_sunday = weekday.num_days_from_sunday();
            
            let mut start_of_week = selected_date;
            for _ in 0..days_from_sunday {
                start_of_week = start_of_week.pred_opt().unwrap();
            }

            let mut week_dates = Vec::new();
            let mut curr = start_of_week;
            for _ in 0..7 {
                week_dates.push(curr);
                curr = curr.succ_opt().unwrap();
            }

            let today = chrono::Local::now().date_naive();

            let mut day_spans = Vec::new();
            day_spans.push(Span::raw(" "));
            for date in week_dates {
                let day_num = date.day();
                let day_str = format!(" {:>2} ", day_num);
                if date == selected_date {
                    day_spans.push(Span::styled(
                        day_str,
                        Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
                    ));
                } else if date == today {
                    day_spans.push(Span::styled(
                        day_str,
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    ));
                } else {
                    day_spans.push(Span::raw(day_str));
                }
                day_spans.push(Span::raw(" "));
            }

            let inner_area = block.inner(popup);
            frame.render_widget(block, popup);

            let chunks = Layout::vertical([
                Constraint::Length(1), // Month Year
                Constraint::Length(1), // Weekdays header
                Constraint::Length(1), // Days row
                Constraint::Length(1), // Instructions
            ])
            .split(inner_area);

            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(header_str, Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))
                ])).alignment(Alignment::Center),
                chunks[0]
            );

            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(weekday_header, Style::default().dim())
                ])).alignment(Alignment::Center),
                chunks[1]
            );

            frame.render_widget(
                Paragraph::new(Line::from(day_spans)).alignment(Alignment::Center),
                chunks[2]
            );

            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" [←→/hj]Day  [↑↓/jk]Week  [[/]]Month  [t]Today  [Enter]Save", Style::default().dim())
                ])).alignment(Alignment::Center),
                chunks[3]
            );
        }
    }

    fn render_recycle_bin(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(area, 60, 12);
        frame.render_widget(Clear, popup);

        let block = Block::bordered()
            .title(" 🗑️ Recycle Bin ")
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Red));

        if self.recycle_bin_todos.is_empty() {
            let paragraph = Paragraph::new("\n\n  Recycle Bin is empty.")
                .block(block)
                .alignment(Alignment::Center)
                .dim();
            frame.render_widget(paragraph, popup);
            return;
        }

        let items: Vec<ListItem> = self
            .recycle_bin_todos
            .iter()
            .enumerate()
            .map(|(idx, t)| {
                let text = format!("#{} {} (original: {})", t.id, t.title, t.column_name);
                let style = if idx == self.recycle_bin_selected {
                    Style::default().bg(Color::Red).fg(Color::Black).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(vec![Span::styled(text, style)]))
            })
            .collect();

        let inner_area = block.inner(popup);
        frame.render_widget(block, popup);

        let chunks = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1), // help text
        ])
        .split(inner_area);

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(self.recycle_bin_selected));

        let list = List::new(items)
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, chunks[0], &mut list_state);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" [↑↓/jk]Select  [r/Enter]Restore  [d]Delete Permanently  [Esc]Close", Style::default().dim())
            ])).alignment(Alignment::Center),
            chunks[1]
        );
    }

    pub fn handle_key(
        &mut self,
        conn: &rusqlite::Connection,
        key: KeyEvent,
    ) -> Result<(), Box<dyn Error>> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(conn, key),
            InputMode::EditingDueDate => self.handle_date_picker_key(conn, key),
            InputMode::RecycleBin => self.handle_recycle_bin_key(conn, key),
            _ => self.handle_input_key(conn, key),
        }
    }

    fn handle_normal_key(
        &mut self,
        conn: &rusqlite::Connection,
        key: KeyEvent,
    ) -> Result<(), Box<dyn Error>> {
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            match key.code {
                KeyCode::Up => {
                    let col = &self.columns[self.focused_col];
                    if let Some(todo) = col.todos.get(col.selected) {
                        if db::move_todo_up(conn, todo.id)? {
                            self.status = format!(" Moved #{} UP", todo.id);
                            self.reload(conn)?;
                            let col_state = &mut self.columns[self.focused_col];
                            if col_state.selected > 0 {
                                col_state.selected -= 1;
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Down => {
                    let col = &self.columns[self.focused_col];
                    if let Some(todo) = col.todos.get(col.selected) {
                        if db::move_todo_down(conn, todo.id)? {
                            self.status = format!(" Moved #{} DOWN", todo.id);
                            self.reload(conn)?;
                            let col_state = &mut self.columns[self.focused_col];
                            if col_state.selected + 1 < col_state.todos.len() {
                                col_state.selected += 1;
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Left => {
                    let col = &self.columns[self.focused_col];
                    if let Some(todo) = col.todos.get(col.selected) {
                        let next = if self.focused_col == 0 {
                            self.columns.len() - 1
                        } else {
                            self.focused_col - 1
                        };
                        let name = &self.columns[next].name;
                        db::move_todo(conn, todo.id, name)?;
                        self.status = format!(" Moved #{} to {}", todo.id, name);
                        self.reload(conn)?;
                        self.focused_col = next;
                    }
                    return Ok(());
                }
                KeyCode::Right => {
                    let col = &self.columns[self.focused_col];
                    if let Some(todo) = col.todos.get(col.selected) {
                        let next = (self.focused_col + 1) % self.columns.len();
                        let name = &self.columns[next].name;
                        db::move_todo(conn, todo.id, name)?;
                        self.status = format!(" Moved #{} to {}", todo.id, name);
                        self.reload(conn)?;
                        self.focused_col = next;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('a') => {
                self.input_mode = InputMode::Adding;
                self.textarea = TextArea::default();
                self.textarea.set_block(Block::bordered()
                    .title(" ➕ Add Todo ")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)));
                self.textarea.set_cursor_line_style(Style::default());
                self.edit_todo_id = None;
                self.status.clear();
            }
            KeyCode::Char('e') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    self.input_mode = InputMode::EditingTitle;
                    self.textarea = TextArea::new(vec![todo.title.clone()]);
                    self.textarea.set_block(Block::bordered()
                        .title(" ✏️ Edit Title ")
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Cyan)));
                    self.textarea.set_cursor_line_style(Style::default());
                    self.textarea.move_cursor(CursorMove::Bottom);
                    self.textarea.move_cursor(CursorMove::End);
                    self.edit_todo_id = Some(todo.id);
                    self.status.clear();
                }
            }
            KeyCode::Char('c') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    self.input_mode = InputMode::EditingDescription;
                    let lines: Vec<String> = todo.description.split('\n').map(String::from).collect();
                    self.textarea = TextArea::new(lines);
                    self.textarea.set_block(Block::bordered()
                        .title(" 📝 Edit Description ")
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Cyan)));
                    self.textarea.set_cursor_line_style(Style::default());
                    self.textarea.move_cursor(CursorMove::Bottom);
                    self.textarea.move_cursor(CursorMove::End);
                    self.edit_todo_id = Some(todo.id);
                    self.status.clear();
                }
            }
            KeyCode::Char('t') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    self.input_mode = InputMode::EditingDueDate;
                    self.edit_todo_id = Some(todo.id);
                    
                    let parsed_date = chrono::NaiveDate::parse_from_str(&todo.due_date, "%Y-%m-%d").ok();
                    self.due_date_picker = Some(parsed_date.unwrap_or_else(|| chrono::Local::now().date_naive()));
                    self.status.clear();
                }
            }
            KeyCode::Char('p') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    let next_priority = match todo.priority.as_str() {
                        "Low" => "Medium",
                        "Medium" => "High",
                        _ => "Low",
                    };
                    db::update_todo_priority(conn, todo.id, next_priority)?;
                    self.status = format!(" Priority of #{} set to {}", todo.id, next_priority);
                    self.reload(conn)?;
                }
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Searching;
                self.textarea = TextArea::new(vec![self.search_query.clone()]);
                self.textarea.set_block(Block::bordered()
                    .title(" 🔍 Search Board ")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)));
                self.textarea.set_cursor_line_style(Style::default());
                self.textarea.move_cursor(CursorMove::Bottom);
                self.textarea.move_cursor(CursorMove::End);
                self.status.clear();
            }
            KeyCode::Char('d') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    db::delete_todo(conn, todo.id)?;
                    self.status = format!(" Deleted #{}", todo.id);
                    self.reload(conn)?;
                }
            }
            KeyCode::Char('m') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    let next = (self.focused_col + 1) % self.columns.len();
                    let name = &self.columns[next].name;
                    db::move_todo(conn, todo.id, name)?;
                    self.status = format!(" Moved #{} to {}", todo.id, name);
                    self.reload(conn)?;
                    self.focused_col = next;
                }
            }
            KeyCode::Char('r') => {
                let archived = db::list_archived_todos(conn)?;
                self.input_mode = InputMode::RecycleBin;
                self.recycle_bin_todos = archived;
                self.recycle_bin_selected = 0;
                self.status.clear();
            }
            KeyCode::Char('K') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    if db::move_todo_up(conn, todo.id)? {
                        self.status = format!(" Moved #{} UP", todo.id);
                        self.reload(conn)?;
                        let col_state = &mut self.columns[self.focused_col];
                        if col_state.selected > 0 {
                            col_state.selected -= 1;
                        }
                    }
                }
            }
            KeyCode::Char('J') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    if db::move_todo_down(conn, todo.id)? {
                        self.status = format!(" Moved #{} DOWN", todo.id);
                        self.reload(conn)?;
                        let col_state = &mut self.columns[self.focused_col];
                        if col_state.selected + 1 < col_state.todos.len() {
                            col_state.selected += 1;
                        }
                    }
                }
            }
            KeyCode::Char('H') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    let next = if self.focused_col == 0 {
                        self.columns.len() - 1
                    } else {
                        self.focused_col - 1
                    };
                    let name = &self.columns[next].name;
                    db::move_todo(conn, todo.id, name)?;
                    self.status = format!(" Moved #{} to {}", todo.id, name);
                    self.reload(conn)?;
                    self.focused_col = next;
                }
            }
            KeyCode::Char('L') => {
                let col = &self.columns[self.focused_col];
                if let Some(todo) = col.todos.get(col.selected) {
                    let next = (self.focused_col + 1) % self.columns.len();
                    let name = &self.columns[next].name;
                    db::move_todo(conn, todo.id, name)?;
                    self.status = format!(" Moved #{} to {}", todo.id, name);
                    self.reload(conn)?;
                    self.focused_col = next;
                }
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.focused_col = (self.focused_col + 1) % self.columns.len();
                self.status.clear();
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.focused_col = if self.focused_col == 0 {
                    self.columns.len() - 1
                } else {
                    self.focused_col - 1
                };
                self.status.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let col = &mut self.columns[self.focused_col];
                if !col.todos.is_empty() {
                    col.selected = (col.selected + 1) % col.todos.len();
                }
                self.status.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let col = &mut self.columns[self.focused_col];
                if !col.todos.is_empty() {
                    col.selected = if col.selected == 0 {
                        col.todos.len() - 1
                    } else {
                        col.selected - 1
                    };
                }
                self.status.clear();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_date_picker_key(
        &mut self,
        conn: &rusqlite::Connection,
        key: KeyEvent,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(ref mut date) = self.due_date_picker {
            match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.edit_todo_id = None;
                    self.due_date_picker = None;
                }
                KeyCode::Enter => {
                    let formatted = date.format("%Y-%m-%d").to_string();
                    if let Some(id) = self.edit_todo_id {
                        db::update_todo_due_date(conn, id, &formatted)?;
                        self.status = format!(" Due date set to '{}' for #{}", formatted, id);
                    }
                    self.reload(conn)?;
                    self.input_mode = InputMode::Normal;
                    self.edit_todo_id = None;
                    self.due_date_picker = None;
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
        &mut self,
        conn: &rusqlite::Connection,
        key: KeyEvent,
    ) -> Result<(), Box<dyn Error>> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.recycle_bin_todos.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.recycle_bin_todos.is_empty() {
                    self.recycle_bin_selected = (self.recycle_bin_selected + 1) % self.recycle_bin_todos.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.recycle_bin_todos.is_empty() {
                    self.recycle_bin_selected = if self.recycle_bin_selected == 0 {
                        self.recycle_bin_todos.len() - 1
                    } else {
                        self.recycle_bin_selected - 1
                    };
                }
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                if let Some(todo) = self.recycle_bin_todos.get(self.recycle_bin_selected) {
                    db::restore_todo(conn, todo.id)?;
                    self.status = format!(" Restored #{} to board", todo.id);
                }
                self.reload(conn)?;
                self.recycle_bin_todos = db::list_archived_todos(conn)?;
                if self.recycle_bin_todos.is_empty() {
                    self.input_mode = InputMode::Normal;
                } else if self.recycle_bin_selected >= self.recycle_bin_todos.len() {
                    self.recycle_bin_selected = self.recycle_bin_todos.len() - 1;
                }
            }
            KeyCode::Char('d') => {
                if let Some(todo) = self.recycle_bin_todos.get(self.recycle_bin_selected) {
                    db::delete_todo_permanently(conn, todo.id)?;
                    self.status = format!(" Permanently deleted #{}", todo.id);
                }
                self.reload(conn)?;
                self.recycle_bin_todos = db::list_archived_todos(conn)?;
                if self.recycle_bin_todos.is_empty() {
                    self.input_mode = InputMode::Normal;
                } else if self.recycle_bin_selected >= self.recycle_bin_todos.len() {
                    self.recycle_bin_selected = self.recycle_bin_todos.len() - 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_input_key(
        &mut self,
        conn: &rusqlite::Connection,
        key: KeyEvent,
    ) -> Result<(), Box<dyn Error>> {
        // Handle custom exits/saves first
        match key.code {
            KeyCode::Esc => {
                if matches!(self.input_mode, InputMode::EditingDescription) {
                    let content = self.textarea.lines().join("\n").trim().to_string();
                    if let Some(id) = self.edit_todo_id {
                        db::update_todo_description(conn, id, &content)?;
                        self.status = format!(" Description updated for #{}", id);
                    }
                    self.reload(conn)?;
                }
                self.input_mode = InputMode::Normal;
                self.edit_todo_id = None;
                return Ok(());
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if matches!(self.input_mode, InputMode::EditingDescription) {
                    let content = self.textarea.lines().join("\n").trim().to_string();
                    if let Some(id) = self.edit_todo_id {
                        db::update_todo_description(conn, id, &content)?;
                        self.status = format!(" Description updated for #{}", id);
                    }
                    self.reload(conn)?;
                }
                self.input_mode = InputMode::Normal;
                self.edit_todo_id = None;
                return Ok(());
            }
            KeyCode::Enter if !matches!(self.input_mode, InputMode::EditingDescription) => {
                let content = self.textarea.lines().join("\n").trim().to_string();
                match self.input_mode {
                    InputMode::Adding => {
                        if !content.is_empty() {
                            let col_name = &self.columns[self.focused_col].name;
                            let todo = db::add_todo(conn, &content, Some(col_name))?;
                            self.status = format!(" Added #{} to {}", todo.id, col_name);
                        }
                    }
                    InputMode::EditingTitle => {
                        if !content.is_empty() {
                            if let Some(id) = self.edit_todo_id {
                                db::update_todo(conn, id, &content)?;
                                self.status = format!(" Title updated for #{}", id);
                            }
                        }
                    }
                    InputMode::Searching => {
                        self.search_query = content;
                        self.status = if self.search_query.is_empty() {
                            " Search cleared".to_string()
                        } else {
                            format!(" Filtering: '{}'", self.search_query)
                        };
                    }
                    _ => {}
                }
                self.reload(conn)?;
                self.input_mode = InputMode::Normal;
                self.edit_todo_id = None;
                return Ok(());
            }
            _ => {}
        }

        // Pass all other keys directly to tui-textarea
        self.textarea.input(key);
        Ok(())
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn add_one_month(date: chrono::NaiveDate) -> chrono::NaiveDate {
    let mut year = date.year();
    let mut month = date.month() + 1;
    if month > 12 {
        month = 1;
        year += 1;
    }
    let day = std::cmp::min(date.day(), days_in_month(year, month));
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn subtract_one_month(date: chrono::NaiveDate) -> chrono::NaiveDate {
    let mut year = date.year();
    let mut month = date.month() as i32 - 1;
    if month < 1 {
        month = 12;
        year -= 1;
    }
    let month = month as u32;
    let day = std::cmp::min(date.day(), days_in_month(year, month));
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn centered_rect(r: Rect, width_pct: u16, height: u16) -> Rect {
    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(r);
    let h = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Percentage(width_pct),
        Constraint::Fill(1),
    ])
    .split(v[1])[1];
    h
}

pub fn run_tui(conn: &rusqlite::Connection) -> Result<(), Box<dyn Error>> {
    db::init_db(conn)?;
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

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let words = text.split_whitespace();
    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() && !text.is_empty() {
        lines.push(text.to_string());
    }
    lines
}
