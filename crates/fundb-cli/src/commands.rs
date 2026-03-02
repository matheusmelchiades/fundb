//! Meta-command parser for backslash commands entered in the REPL.
use crate::args::OutputFormat;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MemorySubcommand {
    List,
    Recall(String),
    Forget(String),
}

#[derive(Debug)]
pub enum MetaCommand {
    Help,
    Quit,
    Connect { host: String, port: u16 },
    Memory { subcommand: MemorySubcommand },
    Causal { from: String, to: String },
    Understand { intent: String },
    Status,
    Format(OutputFormat),
    Unknown(String),
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

impl MetaCommand {
    /// Parse a backslash command.
    ///
    /// `input` is the raw line the user typed, including the leading `\`.
    /// Whitespace-only splits are used for tokenisation; arguments may span
    /// multiple tokens and are rejoined where a "rest of line" is expected.
    pub fn parse(input: &str) -> Self {
        // Strip optional leading backslash and trim
        let trimmed = input.trim();
        let without_slash = if let Some(stripped) = trimmed.strip_prefix('\\') {
            stripped
        } else {
            trimmed
        };

        let mut iter = without_slash
            .splitn(4, char::is_whitespace)
            .filter(|t| !t.is_empty());
        let cmd = match iter.next() {
            Some(c) => c,
            None => return MetaCommand::Unknown(input.to_string()),
        };

        // Collect remaining tokens for subcommand use
        let rest: Vec<&str> = without_slash
            .split_once(char::is_whitespace)
            .map(|x| x.1)
            .unwrap_or("")
            .split_whitespace()
            .collect();

        match cmd.to_lowercase().as_str() {
            "q" | "quit" => MetaCommand::Quit,

            "?" | "help" => MetaCommand::Help,

            "status" => MetaCommand::Status,

            "format" | "f" => {
                let fmt_str = rest.first().copied().unwrap_or("");
                let fmt = match fmt_str.to_lowercase().as_str() {
                    "table" => OutputFormat::Table,
                    "json" => OutputFormat::Json,
                    "csv" => OutputFormat::Csv,
                    _ => {
                        return MetaCommand::Unknown(format!(
                            "Unknown format '{}'. Use: table, json, csv",
                            fmt_str
                        ));
                    }
                };
                MetaCommand::Format(fmt)
            }

            "connect" | "c" => {
                // \connect host[:port]  OR  \connect host port
                let host_arg = rest.first().copied().unwrap_or("localhost");
                let (host, port) = parse_host_port(host_arg, rest.get(1).copied());
                MetaCommand::Connect { host, port }
            }

            "understand" | "u" => {
                let intent = rest.join(" ");
                if intent.is_empty() {
                    MetaCommand::Unknown("\\understand requires an intent argument".to_string())
                } else {
                    MetaCommand::Understand { intent }
                }
            }

            "causal" => {
                if rest.len() < 2 {
                    MetaCommand::Unknown(
                        "\\causal requires two arguments: <from_id> <to_id>".to_string(),
                    )
                } else {
                    MetaCommand::Causal {
                        from: rest[0].to_string(),
                        to: rest[1].to_string(),
                    }
                }
            }

            "memory" | "mem" => {
                let sub = rest.first().copied().unwrap_or("");
                match sub.to_lowercase().as_str() {
                    "list" | "" => MetaCommand::Memory {
                        subcommand: MemorySubcommand::List,
                    },
                    "recall" => {
                        let query = rest[1..].join(" ");
                        if query.is_empty() {
                            MetaCommand::Unknown(
                                "\\memory recall requires a query argument".to_string(),
                            )
                        } else {
                            MetaCommand::Memory {
                                subcommand: MemorySubcommand::Recall(query),
                            }
                        }
                    }
                    "forget" => {
                        let id = rest.get(1).copied().unwrap_or("");
                        if id.is_empty() {
                            MetaCommand::Unknown(
                                "\\memory forget requires an id argument".to_string(),
                            )
                        } else {
                            MetaCommand::Memory {
                                subcommand: MemorySubcommand::Forget(id.to_string()),
                            }
                        }
                    }
                    other => MetaCommand::Unknown(format!(
                        "Unknown memory subcommand '{}'. Use: list, recall, forget",
                        other
                    )),
                }
            }

            other => MetaCommand::Unknown(format!("Unknown meta-command: \\{}", other)),
        }
    }

    pub fn help_text() -> &'static str {
        "\
Meta-commands:
  \\q, \\quit                    Exit the REPL
  \\?, \\help                    Show this help
  \\status                       Show connection information
  \\format <table|json|csv>      Switch output format
  \\connect <host>[:<port>]      Reconnect to a different server
  \\understand <intent>          Run an UNDERSTAND query
  \\causal <from_id> <to_id>     Run a CAUSAL TRACE query
  \\memory list                  List recent memories
  \\memory recall <query>        Recall memories matching a query
  \\memory forget <id>           Forget a memory by ID

SQL:
  End a statement with ; to execute it.
  Use \\ at the end of a line to continue on the next line.

Keyboard shortcuts:
  Ctrl-D                        Exit (EOF)
"
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a "host" or "host:port" argument, optionally supplemented by a
/// separate port token.
fn parse_host_port(host_arg: &str, port_arg: Option<&str>) -> (String, u16) {
    const DEFAULT_PORT: u16 = 5433;

    if let Some(colon_pos) = host_arg.rfind(':') {
        let host = host_arg[..colon_pos].to_string();
        let port = host_arg[colon_pos + 1..]
            .parse::<u16>()
            .unwrap_or(DEFAULT_PORT);
        (host, port)
    } else {
        let port = port_arg
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);
        (host_arg.to_string(), port)
    }
}
