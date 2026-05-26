use chrono::Datelike;
use ratatui::layout::{Constraint, Layout, Rect};

pub fn days_in_month(year: i32, month: u32) -> u32 {
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

pub fn add_one_month(date: chrono::NaiveDate) -> chrono::NaiveDate {
    let mut year = date.year();
    let mut month = date.month() + 1;
    if month > 12 {
        month = 1;
        year += 1;
    }
    let day = std::cmp::min(date.day(), days_in_month(year, month));
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub fn subtract_one_month(date: chrono::NaiveDate) -> chrono::NaiveDate {
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

pub fn centered_rect(r: Rect, width_pct: u16, height: u16) -> Rect {
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

pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
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
