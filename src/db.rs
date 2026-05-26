use chrono::Local;
use rusqlite::{params, Connection, Result};

use crate::models::{Column, Todo};

const DEFAULT_COLUMNS: &[(&str, i32)] = &[
    ("backlog", 0),
    ("todo", 1),
    ("in-progress", 2),
    ("done", 3),
];

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS columns (
            id    INTEGER PRIMARY KEY,
            name  TEXT UNIQUE NOT NULL,
            position INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS todos (
            id         INTEGER PRIMARY KEY,
            title      TEXT NOT NULL,
            column_id  INTEGER NOT NULL,
            position   INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (column_id) REFERENCES columns(id)
        );",
    )?;

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM columns", [], |r| r.get(0))?;
    if count == 0 {
        let mut stmt = conn.prepare("INSERT INTO columns (name, position) VALUES (?1, ?2)")?;
        for (name, pos) in DEFAULT_COLUMNS {
            stmt.execute(params![name, pos])?;
        }
    }

    // safe migrations
    let mut stmt = conn.prepare("PRAGMA table_info(todos)")?;
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))?
        .collect::<Result<Vec<String>, _>>()?;

    if !cols.contains(&"description".to_string()) {
        conn.execute("ALTER TABLE todos ADD COLUMN description TEXT NOT NULL DEFAULT ''", [])?;
    }
    if !cols.contains(&"priority".to_string()) {
        conn.execute("ALTER TABLE todos ADD COLUMN priority TEXT NOT NULL DEFAULT 'Medium'", [])?;
    }
    if !cols.contains(&"due_date".to_string()) {
        conn.execute("ALTER TABLE todos ADD COLUMN due_date TEXT NOT NULL DEFAULT ''", [])?;
    }
    if !cols.contains(&"archived".to_string()) {
        conn.execute("ALTER TABLE todos ADD COLUMN archived INTEGER NOT NULL DEFAULT 0", [])?;
    }

    Ok(())
}

pub fn get_db_path() -> String {
    let dir = dirs_data_local();
    std::fs::create_dir_all(&dir).ok();
    format!("{}\\kanban.db", dir)
}

fn dirs_data_local() -> String {
    if let Ok(val) = std::env::var("KANBAN_DATA_DIR") {
        return val;
    }
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    format!("{}\\kanban", base)
}

pub fn open_connection() -> Result<Connection> {
    let path = get_db_path();
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn add_todo(conn: &Connection, title: &str, column_name: Option<&str>) -> Result<Todo> {
    let col_id: i64 = match column_name {
        Some(name) => {
            conn.query_row("SELECT id FROM columns WHERE name = ?1", params![name], |r| r.get(0))?
        }
        None => {
            conn.query_row("SELECT id FROM columns ORDER BY position LIMIT 1", [], |r| r.get(0))?
        }
    };

    let max_pos: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) FROM todos WHERE column_id = ?1",
            params![col_id],
            |r| r.get(0),
        )?;

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO todos (title, column_id, position, created_at, description, priority, due_date) VALUES (?1, ?2, ?3, ?4, '', 'Medium', '')",
        params![title, col_id, max_pos + 1, now],
    )?;

    let id = conn.last_insert_rowid();
    get_todo(conn, id)
}

pub fn get_todo(conn: &Connection, id: i64) -> Result<Todo> {
    conn.query_row(
        "SELECT t.id, t.title, t.column_id, c.name, t.position, t.created_at, t.description, t.priority, t.due_date
         FROM todos t JOIN columns c ON t.column_id = c.id
         WHERE t.id = ?1",
        params![id],
        |r| {
            let created_str: String = r.get(5)?;
            Ok(Todo {
                id: r.get(0)?,
                title: r.get(1)?,
                column_id: r.get(2)?,
                column_name: r.get(3)?,
                position: r.get(4)?,
                created_at: created_str
                    .parse::<chrono::NaiveDateTime>()
                    .map(|d| d.and_local_timezone(Local).unwrap())
                    .unwrap_or_else(|_| Local::now()),
                description: r.get(6)?,
                priority: r.get(7)?,
                due_date: r.get(8)?,
            })
        },
    )
}

pub fn list_todos(conn: &Connection, column_filter: Option<&str>) -> Result<Vec<Todo>> {
    let query = match column_filter {
        Some(_) => "SELECT t.id, t.title, t.column_id, c.name, t.position, t.created_at, t.description, t.priority, t.due_date
                     FROM todos t JOIN columns c ON t.column_id = c.id
                     WHERE c.name = ?1 AND t.archived = 0
                     ORDER BY c.position, t.position",
        None => "SELECT t.id, t.title, t.column_id, c.name, t.position, t.created_at, t.description, t.priority, t.due_date
                 FROM todos t JOIN columns c ON t.column_id = c.id
                 WHERE t.archived = 0
                 ORDER BY c.position, t.position",
    };

    let mut stmt = conn.prepare(query)?;

    let rows = match column_filter {
        Some(col) => stmt.query_map(params![col], map_todo)?,
        None => stmt.query_map([], map_todo)?,
    };

    let mut todos = Vec::new();
    for row in rows {
        todos.push(row?);
    }
    Ok(todos)
}

fn map_todo(row: &rusqlite::Row) -> rusqlite::Result<Todo> {
    let created_str: String = row.get(5)?;
    Ok(Todo {
        id: row.get(0)?,
        title: row.get(1)?,
        column_id: row.get(2)?,
        column_name: row.get(3)?,
        position: row.get(4)?,
        created_at: created_str
            .parse::<chrono::NaiveDateTime>()
            .map(|d| d.and_local_timezone(Local).unwrap())
            .unwrap_or_else(|_| Local::now()),
        description: row.get(6)?,
        priority: row.get(7)?,
        due_date: row.get(8)?,
    })
}

pub fn move_todo(conn: &Connection, todo_id: i64, target_column: &str) -> Result<Option<Todo>> {
    let target: Option<(i64,)> =
        conn.query_row(
            "SELECT id FROM columns WHERE name = ?1",
            params![target_column],
            |r| Ok((r.get(0)?,)),
        )
        .ok();

    let target_id = match target {
        Some((id,)) => id,
        None => return Ok(None),
    };

    let max_pos: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) FROM todos WHERE column_id = ?1",
            params![target_id],
            |r| r.get(0),
        )?;

    conn.execute(
        "UPDATE todos SET column_id = ?1, position = ?2 WHERE id = ?3",
        params![target_id, max_pos + 1, todo_id],
    )?;

    get_todo(conn, todo_id).map(Some)
}

pub fn update_todo(conn: &Connection, todo_id: i64, title: &str) -> Result<Option<Todo>> {
    let updated = conn.execute("UPDATE todos SET title = ?1 WHERE id = ?2", params![title, todo_id])?;
    if updated == 0 {
        return Ok(None);
    }
    get_todo(conn, todo_id).map(Some)
}

pub fn update_todo_description(conn: &Connection, todo_id: i64, description: &str) -> Result<Option<Todo>> {
    let updated = conn.execute("UPDATE todos SET description = ?1 WHERE id = ?2", params![description, todo_id])?;
    if updated == 0 {
        return Ok(None);
    }
    get_todo(conn, todo_id).map(Some)
}

pub fn update_todo_priority(conn: &Connection, todo_id: i64, priority: &str) -> Result<Option<Todo>> {
    let updated = conn.execute("UPDATE todos SET priority = ?1 WHERE id = ?2", params![priority, todo_id])?;
    if updated == 0 {
        return Ok(None);
    }
    get_todo(conn, todo_id).map(Some)
}

pub fn update_todo_due_date(conn: &Connection, todo_id: i64, due_date: &str) -> Result<Option<Todo>> {
    let updated = conn.execute("UPDATE todos SET due_date = ?1 WHERE id = ?2", params![due_date, todo_id])?;
    if updated == 0 {
        return Ok(None);
    }
    get_todo(conn, todo_id).map(Some)
}

pub fn move_todo_up(conn: &Connection, todo_id: i64) -> Result<bool> {
    let (column_id, position): (i64, i32) = conn.query_row(
        "SELECT column_id, position FROM todos WHERE id = ?1",
        params![todo_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    // Find the todo in the same column with the largest position less than `position`
    let peer: Option<(i64, i32)> = conn
        .query_row(
            "SELECT id, position FROM todos 
             WHERE column_id = ?1 AND position < ?2 
             ORDER BY position DESC LIMIT 1",
            params![column_id, position],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();

    if let Some((peer_id, peer_pos)) = peer {
        // Swap positions!
        conn.execute("UPDATE todos SET position = ?1 WHERE id = ?2", params![peer_pos, todo_id])?;
        conn.execute("UPDATE todos SET position = ?1 WHERE id = ?2", params![position, peer_id])?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn move_todo_down(conn: &Connection, todo_id: i64) -> Result<bool> {
    let (column_id, position): (i64, i32) = conn.query_row(
        "SELECT column_id, position FROM todos WHERE id = ?1",
        params![todo_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    // Find the todo in the same column with the smallest position greater than `position`
    let peer: Option<(i64, i32)> = conn
        .query_row(
            "SELECT id, position FROM todos 
             WHERE column_id = ?1 AND position > ?2 
             ORDER BY position ASC LIMIT 1",
            params![column_id, position],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();

    if let Some((peer_id, peer_pos)) = peer {
        // Swap positions!
        conn.execute("UPDATE todos SET position = ?1 WHERE id = ?2", params![peer_pos, todo_id])?;
        conn.execute("UPDATE todos SET position = ?1 WHERE id = ?2", params![position, peer_id])?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn delete_todo(conn: &Connection, todo_id: i64) -> Result<bool> {
    let updated = conn.execute("UPDATE todos SET archived = 1 WHERE id = ?1", params![todo_id])?;
    Ok(updated > 0)
}

pub fn list_archived_todos(conn: &Connection) -> Result<Vec<Todo>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.title, t.column_id, c.name, t.position, t.created_at, t.description, t.priority, t.due_date
         FROM todos t JOIN columns c ON t.column_id = c.id
         WHERE t.archived = 1
         ORDER BY t.id DESC"
    )?;

    let rows = stmt.query_map([], map_todo)?;
    let mut todos = Vec::new();
    for row in rows {
        todos.push(row?);
    }
    Ok(todos)
}

pub fn restore_todo(conn: &Connection, todo_id: i64) -> Result<bool> {
    let updated = conn.execute("UPDATE todos SET archived = 0 WHERE id = ?1", params![todo_id])?;
    Ok(updated > 0)
}

pub fn delete_todo_permanently(conn: &Connection, todo_id: i64) -> Result<bool> {
    let deleted = conn.execute("DELETE FROM todos WHERE id = ?1", params![todo_id])?;
    Ok(deleted > 0)
}

pub fn list_columns(conn: &Connection) -> Result<Vec<Column>> {
    let mut stmt = conn.prepare("SELECT id, name, position FROM columns ORDER BY position")?;
    let rows = stmt.query_map([], |r| {
        Ok(Column {
            id: r.get(0)?,
            name: r.get(1)?,
            position: r.get(2)?,
        })
    })?;

    let mut cols = Vec::new();
    for row in rows {
        cols.push(row?);
    }
    Ok(cols)
}
