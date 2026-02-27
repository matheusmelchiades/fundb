/// Interactive REPL loop.
///
/// Reads SQL and meta-commands from stdin, dispatches them to the server or
/// handles them locally, and prints results via the `Renderer`.

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::commands::{MemorySubcommand, MetaCommand};
use crate::connection::FunDbConn;
use crate::display::Renderer;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const HISTORY_LIMIT: usize = 100;

const BANNER: &str = "FunDB v0.1.0 — Type \\? for help, \\q to quit";

// ---------------------------------------------------------------------------
// REPL struct
// ---------------------------------------------------------------------------

pub struct Repl {
    conn: FunDbConn,
    renderer: Renderer,
    history: Vec<String>,
    running: bool,
}

impl Repl {
    pub fn new(conn: FunDbConn, renderer: Renderer) -> Self {
        Self {
            conn,
            renderer,
            history: Vec::new(),
            running: true,
        }
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Print welcome banner
        println!("{}", BANNER);
        println!(
            "Connected to {}@{}/{}",
            self.conn.user(),
            self.conn.server_addr(),
            self.conn.database()
        );
        println!();

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);

        // Multi-line SQL buffer
        let mut sql_buf = String::new();
        let mut continuation_lines: usize = 0;

        loop {
            if !self.running {
                break;
            }

            // Print prompt
            if continuation_lines == 0 {
                print!("fundb> ");
            } else {
                print!("fundb({})> ", continuation_lines);
            }
            // Flush stdout so the prompt appears before we block on read
            use std::io::Write;
            std::io::stdout().flush().ok();

            // Read a line
            let mut line = String::new();
            let bytes_read = reader
                .read_line(&mut line)
                .await
                .unwrap_or(0);

            // EOF (Ctrl-D)
            if bytes_read == 0 {
                println!("\nBye!");
                break;
            }

            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');

            // Empty line: just reprint the prompt
            if trimmed.trim().is_empty() {
                continue;
            }

            // --- Meta-command (starts with backslash) ---
            if trimmed.trim_start().starts_with('\\') {
                // If we have a pending SQL buffer, warn the user and discard
                if !sql_buf.is_empty() {
                    println!(
                        "{}",
                        self.renderer
                            .render_notice("Discarding pending SQL buffer.")
                    );
                    sql_buf.clear();
                    continuation_lines = 0;
                }

                let cmd = MetaCommand::parse(trimmed.trim_start());
                self.handle_meta(cmd).await;
                continue;
            }

            // --- Backslash line continuation ---
            if trimmed.ends_with('\\') {
                // Strip the trailing backslash and append to buffer
                let without_cont = &trimmed[..trimmed.len() - 1];
                sql_buf.push_str(without_cont);
                sql_buf.push(' ');
                continuation_lines += 1;
                continue;
            }

            // --- Append to SQL buffer ---
            sql_buf.push_str(trimmed);

            // --- Execute when the buffer ends with a semicolon ---
            let trimmed_buf = sql_buf.trim();
            if trimmed_buf.ends_with(';') {
                let sql = trimmed_buf.to_string();

                // Add to history (trimmed, without trailing semicolon for convenience)
                self.add_history(sql.trim_end_matches(';').trim());

                // Execute
                match self.conn.simple_query(&sql).await {
                    Ok(response) => {
                        let rendered = self.renderer.render(&response);
                        print!("{}", rendered);
                        if !rendered.ends_with('\n') {
                            println!();
                        }
                    }
                    Err(e) => {
                        println!("{}", self.renderer.render_error(&e.to_string()));
                    }
                }

                sql_buf.clear();
                continuation_lines = 0;
            } else {
                // Buffer accumulates; next prompt will show continuation number
                sql_buf.push('\n');
                continuation_lines += 1;
            }
        }

        // Graceful shutdown
        let _ = self.conn.close().await;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Meta-command handler
    // -----------------------------------------------------------------------

    async fn handle_meta(&mut self, cmd: MetaCommand) {
        match cmd {
            MetaCommand::Quit => {
                println!("Bye!");
                self.running = false;
            }

            MetaCommand::Help => {
                println!("{}", MetaCommand::help_text());
            }

            MetaCommand::Status => {
                println!("Connected to: {}", self.conn.server_addr());
                println!("Database:     {}", self.conn.database());
                println!("User:         {}", self.conn.user());
            }

            MetaCommand::Format(fmt) => {
                let name = match &fmt {
                    crate::args::OutputFormat::Table => "table",
                    crate::args::OutputFormat::Json => "json",
                    crate::args::OutputFormat::Csv => "csv",
                };
                self.renderer.format = fmt;
                println!("{}", self.renderer.render_ok(&format!("Output format: {}", name)));
            }

            MetaCommand::Understand { intent } => {
                // Expand to SQL and execute
                let sql = format!("UNDERSTAND '{}' LIMIT 10;", intent.replace('\'', "''"));
                println!(
                    "{}",
                    self.renderer.render_notice(&format!("Executing: {}", sql))
                );
                self.execute_sql(&sql).await;
            }

            MetaCommand::Causal { from, to } => {
                let sql = format!("CAUSAL TRACE '{}' TO '{}';", from, to);
                println!(
                    "{}",
                    self.renderer.render_notice(&format!("Executing: {}", sql))
                );
                self.execute_sql(&sql).await;
            }

            MetaCommand::Memory { subcommand } => match subcommand {
                MemorySubcommand::List => {
                    self.execute_sql("SELECT * FROM memories ORDER BY created_at DESC LIMIT 20;")
                        .await;
                }
                MemorySubcommand::Recall(query) => {
                    let sql = format!(
                        "RECALL '{}' FROM memories LIMIT 10;",
                        query.replace('\'', "''")
                    );
                    self.execute_sql(&sql).await;
                }
                MemorySubcommand::Forget(id) => {
                    let sql = format!(
                        "DELETE FROM memories WHERE id = '{}';",
                        id.replace('\'', "''")
                    );
                    self.execute_sql(&sql).await;
                }
            },

            MetaCommand::Connect { host, port } => {
                println!(
                    "{}",
                    self.renderer.render_notice(&format!(
                        "Reconnect not yet supported in this session. \
                         Start a new session with: fundb --host {} --port {}",
                        host, port
                    ))
                );
            }

            MetaCommand::Unknown(msg) => {
                println!("{}", self.renderer.render_error(&msg));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Execute a SQL string and print the rendered result.
    async fn execute_sql(&mut self, sql: &str) {
        match self.conn.simple_query(sql).await {
            Ok(response) => {
                let rendered = self.renderer.render(&response);
                print!("{}", rendered);
                if !rendered.ends_with('\n') {
                    println!();
                }
            }
            Err(e) => {
                println!("{}", self.renderer.render_error(&e.to_string()));
            }
        }
    }

    /// Add an entry to the in-memory history, evicting the oldest if needed.
    fn add_history(&mut self, entry: &str) {
        if entry.is_empty() {
            return;
        }
        if self.history.len() >= HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(entry.to_string());
    }
}
