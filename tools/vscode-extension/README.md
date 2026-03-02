# FunDB for Visual Studio Code

> The cognitive database for the AI era — right in your editor.

FunDB is an AI-native database with built-in support for vectors, graphs, temporal data, causal inference, and agent memory. This extension brings first-class FunSQL language support and database connectivity to VS Code.

## Features

### FunSQL Language Support

- **Syntax highlighting** for all FunSQL keywords, including AI-native extensions
- Supports `.funsql` and `.fsql` file extensions
- Custom file icon for FunSQL files
- Bracket matching, auto-closing, and comment toggling (`--` and `/* */`)

**Highlighted keywords include:**
- Standard SQL: `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `JOIN`, etc.
- Vectors: `_vector`, `<->` (similarity distance)
- Temporal: `AS OF SYSTEM TIME`, `AS OF VALID TIME`
- Graph: `TRAVERSE`, `->` (edge operator)
- Causal: `TRACE CAUSALITY`, `COUNTERFACTUAL`, `ESTIMATE EFFECT`, `INTERVENE`
- Agent Memory: `UNDERSTAND`, `REMEMBER`, `RECALL BY`, `FORGET`, `WITHIN CONTEXT`

### Connection Management

- Create, edit, and remove connection profiles
- Profiles stored in VS Code settings (syncs across devices)
- Quick pick selector when multiple profiles exist
- Status bar indicator showing active connection
- Connection protocol: `fundb://user@host:port/database`

### Query Execution

- **Cmd+Enter** (macOS) / **Ctrl+Enter** — Execute the entire file
- **Cmd+Shift+Enter** / **Ctrl+Shift+Enter** — Execute selected text
- Results displayed in a rich panel with sortable columns
- Execution time and row count shown per query
- Error messages with query context

### Results Panel

- Tabular display with branded styling
- NULL values displayed in italic gray
- Export results as **JSON** or **CSV**
- Command tag and row count info

### Sidebar Explorer

- FunDB icon in the Activity Bar
- View all configured connection profiles
- Connect/disconnect with a click
- Right-click context menu: Edit, Remove

## Getting Started

1. Install the extension (`.vsix` file or from marketplace)
2. Open the Command Palette (`Cmd+Shift+P`) and run **FunDB: New Connection**
3. Follow the guided wizard: name, host, port, database, user, password, SSL
4. Create a `.funsql` file and start writing queries
5. Press **Cmd+Enter** to execute

### Default Connection Settings

| Setting  | Default     |
|----------|-------------|
| Host     | `localhost` |
| Port     | `5433`      |
| Database | `fundb`     |
| User     | `fundb`     |
| SSL      | `false`     |

## Commands

| Command                        | Description                     |
|-------------------------------|---------------------------------|
| `FunDB: Connect`              | Connect to a FunDB instance     |
| `FunDB: Disconnect`           | Disconnect from FunDB           |
| `FunDB: New Connection`       | Create a new connection profile |
| `FunDB: Edit Connection`      | Edit an existing profile        |
| `FunDB: Remove Connection`    | Delete a connection profile     |
| `FunDB: Execute Query`        | Run the entire file             |
| `FunDB: Execute Selection`    | Run selected text               |
| `FunDB: Export Results as JSON` | Export last results as JSON   |
| `FunDB: Export Results as CSV`  | Export last results as CSV    |
| `FunDB: Refresh Explorer`     | Refresh the sidebar explorer    |

## Configuration

Settings are under `fundb.*` in your VS Code settings:

```json
{
  "fundb.connections": [
    {
      "name": "local",
      "host": "localhost",
      "port": 5433,
      "database": "fundb",
      "user": "fundb",
      "password": "",
      "ssl": false
    }
  ]
}
```

## Requirements

- FunDB server running (default port 5433)
- VS Code 1.85.0 or later

## About FunDB

FunDB is an AI-native database written in Rust that unifies five data paradigms:

- **Relational** — Standard SQL tables and joins
- **Vector** — Semantic similarity search
- **Graph** — Relationship traversal
- **Temporal** — Time-travel queries (bi-temporal)
- **Causal** — Causal inference and counterfactual analysis

Plus **Agent Memory** — REMEMBER, RECALL, UNDERSTAND for AI agents.

All accessed through **FunSQL**, a superset of SQL with AI-native extensions, over a PostgreSQL-compatible wire protocol.

## License

Apache-2.0
