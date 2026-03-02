/// CLI argument definitions and manual parser (no clap dependency).

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

#[derive(Debug)]
pub struct CliArgs {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: Option<String>,
    pub command: Option<String>,
    pub file: Option<String>,
    pub no_color: bool,
    pub output_format: OutputFormat,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5433,
            database: "fundb".to_string(),
            user: "fundb".to_string(),
            password: None,
            command: None,
            file: None,
            no_color: false,
            output_format: OutputFormat::Table,
        }
    }
}

impl CliArgs {
    pub fn parse() -> anyhow::Result<Self> {
        let mut args = Self::default();
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;

        while i < raw.len() {
            let arg = raw[i].as_str();

            match arg {
                "--help" | "-?" => {
                    println!("{}", Self::usage());
                    std::process::exit(0);
                }
                "--version" | "-V" => {
                    println!("fundb 0.1.0");
                    std::process::exit(0);
                }
                "--no-color" => {
                    args.no_color = true;
                }
                "--host" | "-h" => {
                    i += 1;
                    args.host = Self::require_next(&raw, i, arg)?;
                }
                "--port" | "-p" => {
                    i += 1;
                    let v = Self::require_next(&raw, i, arg)?;
                    args.port = v.parse::<u16>().map_err(|_| {
                        anyhow::anyhow!("Invalid port number '{}': must be a number between 1 and 65535", v)
                    })?;
                }
                "--database" | "-d" => {
                    i += 1;
                    args.database = Self::require_next(&raw, i, arg)?;
                }
                "--user" | "-u" => {
                    i += 1;
                    args.user = Self::require_next(&raw, i, arg)?;
                }
                "--password" | "-W" => {
                    i += 1;
                    args.password = Some(Self::require_next(&raw, i, arg)?);
                }
                "--command" | "-c" => {
                    i += 1;
                    args.command = Some(Self::require_next(&raw, i, arg)?);
                }
                "--file" | "-i" => {
                    i += 1;
                    args.file = Some(Self::require_next(&raw, i, arg)?);
                }
                "--format" | "-f" => {
                    i += 1;
                    let v = Self::require_next(&raw, i, arg)?;
                    args.output_format = match v.to_lowercase().as_str() {
                        "table" => OutputFormat::Table,
                        "json" => OutputFormat::Json,
                        "csv" => OutputFormat::Csv,
                        other => {
                            return Err(anyhow::anyhow!(
                                "Unknown format '{}'. Use table, json, or csv.",
                                other
                            ))
                        }
                    };
                }
                other if other.starts_with('-') => {
                    return Err(anyhow::anyhow!("Unknown argument '{}'. Run 'fundb --help' for usage information", other));
                }
                _ => {
                    // Positional argument: treat as database name
                    args.database = arg.to_string();
                }
            }

            i += 1;
        }

        Ok(args)
    }

    fn require_next(raw: &[String], i: usize, flag: &str) -> anyhow::Result<String> {
        raw.get(i)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Flag '{}' requires a value. Usage: {} <value>", flag, flag))
    }

    pub fn usage() -> &'static str {
        "\
Usage: fundb [OPTIONS] [DATABASE]

Options:
  -h, --host <HOST>         Server host (default: localhost)
  -p, --port <PORT>         Server port (default: 5433)
  -d, --database <DB>       Database name (default: fundb)
  -u, --user <USER>         Username (default: fundb)
  -W, --password <PASSWORD> Password
  -c, --command <SQL>       Run single SQL command and exit
  -i, --file <PATH>         Execute SQL statements from a .funsql file
  -f, --format <FMT>        Output format: table (default), json, csv
      --no-color            Disable ANSI color output
  -V, --version             Print version and exit
  -?, --help                Show this help

Meta-commands (in REPL mode):
  \\q, \\quit               Exit
  \\?, \\help               Show help
  \\status                  Show connection info
  \\format <table|json|csv> Switch output format
  \\understand <intent>     Run UNDERSTAND query
  \\causal <from> <to>      Run CAUSAL TRACE query
  \\memory list             List recent memories
  \\memory recall <query>   Recall memories
  \\memory forget <id>      Forget a memory

Examples:
  fundb --host localhost --port 5433
  fundb -c 'SELECT 1'
  fundb -i seed-data.funsql
  fundb --format json -c 'SELECT * FROM documents LIMIT 5'
"
    }
}
