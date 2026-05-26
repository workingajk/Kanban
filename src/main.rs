mod db;
mod models;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kanban", version, about = "A kanban-style todo app")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the database
    Init,
    /// Add a new todo
    Add {
        /// Title of the todo
        title: String,
    },
    /// List all todos
    List {
        /// Filter by column name (backlog, todo, in-progress, done)
        column: Option<String>,
    },
    /// Move a todo to another column
    Move {
        /// Todo ID
        id: i64,
        /// Target column name
        column: String,
    },
    /// Update a todo's title
    Update {
        /// Todo ID
        id: i64,
        /// New title
        title: String,
    },
    /// Delete a todo
    Delete {
        /// Todo ID
        id: i64,
    },
    /// List all columns
    Columns,
    /// Launch the terminal UI
    Tui,
}

fn main() {
    let cli = Cli::parse();

    let conn = db::open_connection().unwrap_or_else(|e| {
        eprintln!("Error: Could not open database: {}", e);
        std::process::exit(1);
    });

    match &cli.command {
        Commands::Init => cmd_init(&conn),
        Commands::Add { title } => cmd_add(&conn, title),
        Commands::List { column } => cmd_list(&conn, column.as_deref()),
        Commands::Move { id, column } => cmd_move(&conn, *id, column),
        Commands::Update { id, title } => cmd_update(&conn, *id, title),
        Commands::Delete { id } => cmd_delete(&conn, *id),
        Commands::Columns => cmd_columns(&conn),
        Commands::Tui => cmd_tui(&conn),
    }
}

fn cmd_init(conn: &rusqlite::Connection) {
    db::init_db(conn).unwrap_or_else(|e| {
        eprintln!("Error initializing database: {}", e);
        std::process::exit(1);
    });
    println!("Database initialized at {}", db::get_db_path());
}

fn cmd_add(conn: &rusqlite::Connection, title: &str) {
    let todo = db::add_todo(conn, title, None).unwrap_or_else(|e| {
        eprintln!("Error adding todo: {}", e);
        std::process::exit(1);
    });
    println!("Added [{}] to [{}]: {}", todo.id, todo.column_name, todo.title);
}

fn cmd_list(conn: &rusqlite::Connection, column: Option<&str>) {
    let todos = db::list_todos(conn, column).unwrap_or_else(|e| {
        eprintln!("Error listing todos: {}", e);
        std::process::exit(1);
    });

    let columns = db::list_columns(conn).unwrap_or_else(|e| {
        eprintln!("Error listing columns: {}", e);
        std::process::exit(1);
    });

    if todos.is_empty() {
        println!("No todos found.");
        return;
    }

    let column_colors: [(&str, &str); 4] = [
        ("backlog", "\x1b[90m"),
        ("todo", "\x1b[94m"),
        ("in-progress", "\x1b[93m"),
        ("done", "\x1b[92m"),
    ];

    for col in &columns {
        let color = column_colors
            .iter()
            .find(|(name, _)| *name == col.name)
            .map(|(_, c)| *c)
            .unwrap_or("\x1b[0m");
        let reset = "\x1b[0m";

        let col_todos: Vec<&models::Todo> = todos.iter().filter(|t| t.column_id == col.id).collect();
        if col_todos.is_empty() {
            continue;
        }

        println!("\n{}{}{}", color, col.name.to_uppercase(), reset);
        println!("{}", "-".repeat(50));
        for todo in &col_todos {
            println!(
                "  {}#{:<4}{} {}",
                color, todo.id, reset, todo.title
            );
        }
    }
}

fn cmd_move(conn: &rusqlite::Connection, id: i64, column: &str) {
    match db::move_todo(conn, id, column) {
        Ok(Some(todo)) => {
            println!("Moved #{} to [{}]: {}", todo.id, todo.column_name, todo.title);
        }
        Ok(None) => {
            eprintln!("Error: Column '{}' not found or todo #{} does not exist.", column, id);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error moving todo: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_update(conn: &rusqlite::Connection, id: i64, title: &str) {
    match db::update_todo(conn, id, title) {
        Ok(Some(todo)) => {
            println!("Updated #{}: {}", todo.id, todo.title);
        }
        Ok(None) => {
            eprintln!("Error: Todo #{} not found.", id);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error updating todo: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_delete(conn: &rusqlite::Connection, id: i64) {
    match db::delete_todo(conn, id) {
        Ok(true) => println!("Deleted #{}", id),
        Ok(false) => {
            eprintln!("Error: Todo #{} not found.", id);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error deleting todo: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_columns(conn: &rusqlite::Connection) {
    let columns = db::list_columns(conn).unwrap_or_else(|e| {
        eprintln!("Error listing columns: {}", e);
        std::process::exit(1);
    });
    println!("Kanban columns:");
    for col in &columns {
        println!("  {} (#{})", col.name, col.id);
    }
}

fn cmd_tui(conn: &rusqlite::Connection) {
    tui::run_tui(conn).unwrap_or_else(|e| {
        eprintln!("Error in TUI: {}", e);
        std::process::exit(1);
    });
}
