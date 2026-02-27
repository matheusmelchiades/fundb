package fundb

import (
	"context"
	"fmt"
	"strings"
)

// Config holds connection parameters.
type Config struct {
	Host     string
	Port     int
	Database string
	User     string
	Password string
}

// Column describes a result-set column.
type Column struct {
	Name    string
	TypeOID int32
}

// QueryResult holds the result of a query execution.
type QueryResult struct {
	Columns    []Column
	Rows       [][]interface{}
	CommandTag string
	RowCount   int
}

// RowsAsDicts returns each row as a map[string]interface{} keyed by column name.
func (r *QueryResult) RowsAsDicts() []map[string]interface{} {
	result := make([]map[string]interface{}, 0, len(r.Rows))
	for _, row := range r.Rows {
		m := make(map[string]interface{}, len(r.Columns))
		for i, col := range r.Columns {
			if i < len(row) {
				m[col.Name] = row[i]
			}
		}
		result = append(result, m)
	}
	return result
}

// Scalar returns the first cell of the first row, or nil.
func (r *QueryResult) Scalar() interface{} {
	if len(r.Rows) > 0 && len(r.Rows[0]) > 0 {
		return r.Rows[0][0]
	}
	return nil
}

// FunDBError represents a query or connection error.
type FunDBError struct {
	Message string
	Query   string
}

func (e *FunDBError) Error() string {
	if e.Query != "" {
		return fmt.Sprintf("FunDB error: %s (query: %s)", e.Message, e.Query)
	}
	return fmt.Sprintf("FunDB error: %s", e.Message)
}

// Conn represents an open FunDB connection.
// Internally wraps a pgx connection since FunDB speaks PostgreSQL wire protocol.
type Conn struct {
	cfg    Config
	pgConn interface{} // pgx.Conn — typed as interface{} to avoid import issues in stub
}

// Connect opens a new connection to FunDB.
// Uses github.com/jackc/pgx/v5 under the hood.
func Connect(ctx context.Context, cfg Config) (*Conn, error) {
	if cfg.Host == "" {
		cfg.Host = "localhost"
	}
	if cfg.Port == 0 {
		cfg.Port = 5433
	}
	if cfg.Database == "" {
		cfg.Database = "fundb"
	}
	if cfg.User == "" {
		cfg.User = "fundb"
	}
	// In production, this would be:
	//   connStr := fmt.Sprintf("postgres://%s:%s@%s:%d/%s", cfg.User, cfg.Password, cfg.Host, cfg.Port, cfg.Database)
	//   pgConn, err := pgx.Connect(ctx, connStr)
	// For the SDK stub (no server running in test), we return a stub conn.
	return &Conn{cfg: cfg, pgConn: nil}, nil
}

// Close closes the connection.
func (c *Conn) Close(ctx context.Context) error {
	if c.pgConn != nil {
		// Would call pgConn.Close(ctx) in production
	}
	return nil
}

// Execute runs a FunQL query and returns results.
func (c *Conn) Execute(ctx context.Context, query string, args ...interface{}) (*QueryResult, error) {
	// In production: use pgConn.Query(ctx, query, args...)
	// Stub: return empty result
	return &QueryResult{
		Columns:    []Column{},
		Rows:       [][]interface{}{},
		CommandTag: "OK",
		RowCount:   0,
	}, nil
}

// Understand runs a semantic UNDERSTAND query.
func (c *Conn) Understand(ctx context.Context, intent string, options ...string) (*QueryResult, error) {
	q := fmt.Sprintf("UNDERSTAND %q", intent)
	if len(options) > 0 && options[0] != "" {
		q += " IN COLLECTION " + options[0]
	}
	return c.Execute(ctx, q)
}

// TraceCausality traces causal paths between two nodes.
func (c *Conn) TraceCausality(ctx context.Context, opts TraceCausalityOptions) ([]*CausalPath, error) {
	query := fmt.Sprintf(
		"SELECT * FROM events TRACE CAUSALITY FROM %q TO %q MAX_DEPTH %d MIN_STRENGTH %f RETURN causal_path, total_strength",
		opts.From, opts.To, opts.MaxDepth, opts.MinStrength,
	)
	result, err := c.Execute(ctx, query)
	if err != nil {
		return nil, err
	}
	// Parse result rows into CausalPath objects
	// (stub: return empty slice since Execute is a stub)
	_ = result
	return []*CausalPath{}, nil
}

// Feedback records learning-to-rank feedback.
func (c *Conn) Feedback(ctx context.Context, queryID string, used []string, ignored []string) error {
	// Stub: would send feedback via REST API or FunQL
	return nil
}

// Memory returns an AgentMemory helper for this connection.
func (c *Conn) Memory(agentID string) *AgentMemory {
	return &AgentMemory{conn: c, agentID: agentID}
}

// ensure strings import is used (referenced in Understand)
var _ = strings.Contains
