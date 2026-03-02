/// Output rendering for query results.
///
/// Supports three formats: Table (psql-style ASCII), JSON, and CSV.
/// Optionally wraps output in ANSI escape codes for colour.
use crate::args::OutputFormat;
use crate::connection::QueryResponse;

// ---------------------------------------------------------------------------
// ANSI helpers
// ---------------------------------------------------------------------------

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

pub struct Renderer {
    pub no_color: bool,
    pub format: OutputFormat,
}

impl Renderer {
    pub fn new(no_color: bool, format: OutputFormat) -> Self {
        Self { no_color, format }
    }

    /// Render a full QueryResponse according to the current format.
    pub fn render(&self, response: &QueryResponse) -> String {
        if let Some(ref err) = response.error {
            return self.render_error(err);
        }

        let mut out = String::new();

        if let Some(ref notice) = response.notice {
            out.push_str(&self.render_notice(notice));
            out.push('\n');
        }

        match self.format {
            OutputFormat::Table => out.push_str(&self.render_table(response)),
            OutputFormat::Json => out.push_str(&self.render_json(response)),
            OutputFormat::Csv => out.push_str(&self.render_csv(response)),
        }

        out
    }

    // -----------------------------------------------------------------------
    // Table rendering (psql-style)
    // -----------------------------------------------------------------------

    fn render_table(&self, response: &QueryResponse) -> String {
        let columns = &response.columns;
        let rows = &response.rows;

        // If there are no columns and no rows, just return the command tag.
        if columns.is_empty() {
            if !response.command_tag.is_empty() {
                return self.render_ok(&response.command_tag);
            }
            return String::new();
        }

        // Compute column widths: max(header_len, max_value_len)
        let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        let mut out = String::new();

        // Header row
        let header: String = columns
            .iter()
            .enumerate()
            .map(|(i, col)| format!(" {:width$} ", col, width = widths[i]))
            .collect::<Vec<_>>()
            .join("|");

        if self.no_color {
            out.push_str(&header);
        } else {
            out.push_str(BOLD);
            out.push_str(&header);
            out.push_str(RESET);
        }
        out.push('\n');

        // Separator row: ---+---+---
        let separator: String = widths
            .iter()
            .map(|w| "-".repeat(w + 2))
            .collect::<Vec<_>>()
            .join("+");
        out.push_str(&separator);
        out.push('\n');

        // Data rows
        for row in rows {
            let line: String = (0..columns.len())
                .map(|i| {
                    let cell = row.get(i).map(String::as_str).unwrap_or("");
                    format!(" {:width$} ", cell, width = widths[i])
                })
                .collect::<Vec<_>>()
                .join("|");
            out.push_str(&line);
            out.push('\n');
        }

        // Row count footer
        let row_count = rows.len();
        let footer = if row_count == 1 {
            "(1 row)".to_string()
        } else {
            format!("({} rows)", row_count)
        };
        out.push_str(&footer);
        out.push('\n');

        // Command tag (if present and not just SELECT)
        if !response.command_tag.is_empty() {
            out.push_str(&self.render_ok(&response.command_tag));
            out.push('\n');
        }

        out
    }

    // -----------------------------------------------------------------------
    // JSON rendering
    // -----------------------------------------------------------------------

    fn render_json(&self, response: &QueryResponse) -> String {
        let columns = &response.columns;
        let rows = &response.rows;

        // Build JSON manually to avoid serde dependency on the output layer
        // (serde is available but keeping this self-contained).
        let col_json: Vec<String> = columns
            .iter()
            .map(|c| format!("\"{}\"", escape_json(c)))
            .collect();

        let rows_json: Vec<String> = rows
            .iter()
            .map(|row| {
                let cells: Vec<String> = (0..columns.len())
                    .map(|i| {
                        let cell = row.get(i).map(String::as_str).unwrap_or("null");
                        if cell == "NULL" {
                            "null".to_string()
                        } else {
                            format!("\"{}\"", escape_json(cell))
                        }
                    })
                    .collect();
                format!("[{}]", cells.join(", "))
            })
            .collect();

        let mut json = String::new();
        json.push_str("{\"columns\": [");
        json.push_str(&col_json.join(", "));
        json.push_str("], \"rows\": [");
        json.push_str(&rows_json.join(", "));
        json.push_str("]}");
        json.push('\n');

        json
    }

    // -----------------------------------------------------------------------
    // CSV rendering
    // -----------------------------------------------------------------------

    fn render_csv(&self, response: &QueryResponse) -> String {
        let columns = &response.columns;
        let rows = &response.rows;
        let mut out = String::new();

        // Header
        let header: String = columns
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&header);
        out.push('\n');

        // Data rows
        for row in rows {
            let line: String = (0..columns.len())
                .map(|i| {
                    let cell = row.get(i).map(String::as_str).unwrap_or("");
                    csv_escape(cell)
                })
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&line);
            out.push('\n');
        }

        out
    }

    // -----------------------------------------------------------------------
    // Colour helpers
    // -----------------------------------------------------------------------

    pub fn render_error(&self, msg: &str) -> String {
        if self.no_color {
            format!("ERROR: {}", msg)
        } else {
            format!("{}ERROR:{} {}", RED, RESET, msg)
        }
    }

    pub fn render_notice(&self, msg: &str) -> String {
        if self.no_color {
            format!("NOTICE: {}", msg)
        } else {
            format!("{}NOTICE:{} {}", YELLOW, RESET, msg)
        }
    }

    pub fn render_ok(&self, tag: &str) -> String {
        if self.no_color {
            tag.to_string()
        } else {
            format!("{}{}{}", GREEN, tag, RESET)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape a string for embedding inside a JSON double-quoted string.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Escape a field for CSV output (RFC 4180).
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
