mod args;
mod commands;
mod connection;
mod display;
mod repl;

use args::CliArgs;
use connection::FunDbConn;
use display::Renderer;
use repl::Repl;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Parse CLI arguments (manual parsing — no clap)
    let cli = CliArgs::parse()?;

    // 2. Set up tracing.
    //    Default level is INFO; override with FUNDB_LOG env var (e.g. FUNDB_LOG=debug).
    let env_filter = tracing_subscriber::EnvFilter::try_from_env("FUNDB_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_writer(std::io::stderr) // logs go to stderr; query output to stdout
        .init();

    // 3. Connect to the FunDB server
    tracing::info!(
        host = %cli.host,
        port = cli.port,
        database = %cli.database,
        user = %cli.user,
        "Connecting to FunDB"
    );

    let mut conn = FunDbConn::connect(
        &cli.host,
        cli.port,
        &cli.database,
        &cli.user,
        cli.password.as_deref(),
    )
    .await?;

    let renderer = Renderer::new(cli.no_color, cli.output_format.clone());

    // 4. If --command / -c was given, execute it and exit
    if let Some(ref sql) = cli.command {
        // Ensure the statement ends with a semicolon
        let sql = if sql.trim().ends_with(';') {
            sql.clone()
        } else {
            format!("{};", sql)
        };

        match conn.simple_query(&sql).await {
            Ok(response) => {
                let rendered = renderer.render(&response);
                print!("{}", rendered);
                if !rendered.ends_with('\n') {
                    println!();
                }
            }
            Err(e) => {
                eprintln!("{}", renderer.render_error(&e.to_string()));
                conn.close().await.ok();
                std::process::exit(1);
            }
        }

        conn.close().await.ok();
        return Ok(());
    }

    // 4b. If --file / -i was given, execute each statement from the file
    if let Some(ref path) = cli.file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read '{}': {}", path, e))?;

        // Split on semicolons, filter out empty segments
        let statements: Vec<&str> = content
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let total = statements.len();
        println!("Executing {} statements from {}", total, path);

        let mut ok_count = 0;
        let mut err_count = 0;

        for (i, stmt) in statements.iter().enumerate() {
            // Strip comment-only lines
            let clean: String = stmt
                .lines()
                .filter(|line| !line.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();

            if clean.is_empty() {
                continue;
            }

            let sql = format!("{};", clean);
            match conn.simple_query(&sql).await {
                Ok(response) => {
                    let rendered = renderer.render(&response);
                    let tag = rendered.trim();
                    if !tag.is_empty() {
                        println!("  [{}/{}] OK: {}", i + 1, total, tag.lines().next().unwrap_or(""));
                    } else {
                        println!("  [{}/{}] OK", i + 1, total);
                    }
                    ok_count += 1;
                }
                Err(e) => {
                    eprintln!("  [{}/{}] ERROR: {}", i + 1, total, e);
                    err_count += 1;
                }
            }
        }

        println!("\nDone: {} succeeded, {} failed", ok_count, err_count);
        conn.close().await.ok();

        if err_count > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    // 5. Enter the interactive REPL
    let mut repl = Repl::new(conn, renderer);
    repl.run().await?;

    Ok(())
}
