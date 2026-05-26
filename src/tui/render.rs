use chrono::Datelike;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{ColumnState, InputMode, TuiApp, COLUMN_STYLES};
use super::utils::{centered_rect, wrap_text};

pub fn render_app(app: &mut TuiApp, frame: &mut Frame) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(app, frame, header);
    render_body(app, frame, body);
    render_footer(app, frame, footer);

    if !matches!(app.input_mode, InputMode::Normal) {
        render_input_popup(app, frame, frame.area());
    }
}

fn render_header(app: &TuiApp, frame: &mut Frame, area: Rect) {
    let mut total = 0;
    let mut backlog = 0;
    let mut todo = 0;
    let mut in_progress = 0;
    let mut done = 0;

    for col in &app.columns {
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

    let search_text = if !app.search_query.is_empty() || matches!(app.input_mode, InputMode::Searching) {
        format!("🔍 Search: {} ", app.search_query)
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

fn render_body(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(73),
        Constraint::Percentage(27),
    ])
    .split(area);

    let board_area = chunks[0];
    let detail_area = chunks[1];

    let n = app.columns.len() as u16;
    let col_areas = Layout::horizontal(vec![Constraint::Ratio(1, n as u32); n as usize])
        .spacing(1)
        .split(board_area);

    let focused = app.focused_col;
    for (i, col) in app.columns.iter_mut().enumerate() {
        render_column(col, frame, col_areas[i], i == focused);
    }

    render_details_panel(app, frame, detail_area);
}

fn render_details_panel(app: &TuiApp, frame: &mut Frame, area: Rect) {
    let active_col = &app.columns[app.focused_col];
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

pub(crate) fn render_column(col: &mut ColumnState, frame: &mut Frame, area: Rect, focused: bool) {
    let color = COLUMN_STYLES
        .iter()
        .find(|(n, _)| *n == col.name)
        .map(|(_, c)| *c)
        .unwrap_or(Color::Reset);

    let title_name = match col.name.as_str() {
        "backlog" => "📋 BACKLOG",
        "todo" => "📥 TODO",
        "in-progress" => "⚡ IN PROGRESS",
        "done" => "✅ DONE",
        _ => &col.name,
    };
    let title = format!(" {} ({}) ", title_name, col.todos.len());

    let max_title_len = area.width.saturating_sub(10) as usize;

    let items: Vec<ListItem> = col
        .todos
        .iter()
        .enumerate()
        .map(|(item_idx, t)| {
            let is_selected = item_idx == col.selected;

            let normal_fg = if is_selected {
                Color::Black
            } else {
                Color::White
            };

            let id_style = if is_selected {
                Style::default().fg(Color::Black).add_modifier(Modifier::DIM)
            } else {
                Style::default().dim()
            };

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
                        Span::styled(format!("#{} ", t.id), id_style),
                        Span::styled(line.clone(), Style::default().fg(normal_fg)),
                    ]));
                } else {
                    let id_padding = " ".repeat(format!("#{} ", t.id).len());
                    list_lines.push(Line::from(vec![
                        Span::raw(id_padding),
                        Span::styled(line.clone(), Style::default().fg(normal_fg)),
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
    if !col.todos.is_empty() {
        state.select(Some(col.selected));
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
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_footer(app: &TuiApp, frame: &mut Frame, area: Rect) {
    let line = match &app.input_mode {
        InputMode::Normal => {
            if !app.status.is_empty() {
                Line::from(vec![
                    Span::styled("✨ Status: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(&app.status, Style::default().fg(Color::White)),
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

fn render_input_popup(app: &TuiApp, frame: &mut Frame, area: Rect) {
    if matches!(app.input_mode, InputMode::EditingDueDate) {
        render_weekly_date_picker(app, frame, area);
        return;
    }
    if matches!(app.input_mode, InputMode::RecycleBin) {
        render_recycle_bin(app, frame, area);
        return;
    }

    let (width_pct, height) = match app.input_mode {
        InputMode::EditingDescription => (70, 10),
        _ => (50, 5),
    };

    let popup = centered_rect(area, width_pct, height);
    frame.render_widget(Clear, popup);
    frame.render_widget(&app.textarea, popup);
}

fn render_weekly_date_picker(app: &TuiApp, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(area, 60, 6);
    frame.render_widget(Clear, popup);
    let block = Block::bordered()
        .title(" 📅 Select Due Date ")
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    if let Some(selected_date) = app.due_date_picker {
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

fn render_recycle_bin(app: &TuiApp, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(area, 60, 12);
    frame.render_widget(Clear, popup);

    let block = Block::bordered()
        .title(" 🗑️ Recycle Bin ")
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));

    if app.recycle_bin_todos.is_empty() {
        let paragraph = Paragraph::new("\n\n  Recycle Bin is empty.")
            .block(block)
            .alignment(Alignment::Center)
            .dim();
        frame.render_widget(paragraph, popup);
        return;
    }

    let items: Vec<ListItem> = app
        .recycle_bin_todos
        .iter()
        .enumerate()
        .map(|(idx, t)| {
            let text = format!("#{} {} (original: {})", t.id, t.title, t.column_name);
            let style = if idx == app.recycle_bin_selected {
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
    list_state.select(Some(app.recycle_bin_selected));

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
