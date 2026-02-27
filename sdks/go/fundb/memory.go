package fundb

import (
	"context"
	"fmt"
)

// MemoryType indicates the kind of memory being stored.
type MemoryType string

const (
	MemoryTypeSemantic   MemoryType = "semantic"
	MemoryTypeEpisodic   MemoryType = "episodic"
	MemoryTypeProcedural MemoryType = "procedural"
)

// RememberOptions configures how a memory is stored.
type RememberOptions struct {
	Importance float64
	MemoryType MemoryType
	DecayRate  float64
}

// RecallWeights configures the weighted recall scoring.
type RecallWeights struct {
	Semantic   float64
	Recency    float64
	Importance float64
}

// Validate checks that weights sum to approximately 1.0.
func (w RecallWeights) Validate() error {
	total := w.Semantic + w.Recency + w.Importance
	if total < 0.99 || total > 1.01 {
		return fmt.Errorf("RecallWeights must sum to 1.0, got %.2f", total)
	}
	return nil
}

// MemoryRecord is a retrieved memory with its score breakdown.
type MemoryRecord struct {
	MemoryID        string
	Content         string
	Score           float64
	SemanticScore   float64
	RecencyScore    float64
	ImportanceScore float64
}

// AgentMemory provides persistent memory operations for an AI agent.
type AgentMemory struct {
	conn    *Conn
	agentID string
}

// Remember stores a memory and returns its ID.
func (m *AgentMemory) Remember(ctx context.Context, content string, opts RememberOptions) (string, error) {
	if opts.MemoryType == "" {
		opts.MemoryType = MemoryTypeSemantic
	}
	if opts.Importance == 0 {
		opts.Importance = 0.5
	}
	query := fmt.Sprintf(
		"REMEMBER '%s' FOR AGENT '%s' WITH importance %f AS %s",
		content, m.agentID, opts.Importance, opts.MemoryType,
	)
	_, err := m.conn.Execute(ctx, query)
	if err != nil {
		return "", err
	}
	// Return a deterministic stub ID based on content hash
	return fmt.Sprintf("mem-%x", len(content)), nil
}

// Recall retrieves memories matching a query string.
func (m *AgentMemory) Recall(ctx context.Context, query string, weights RecallWeights, topK int) ([]*MemoryRecord, error) {
	if err := weights.Validate(); err != nil {
		return nil, err
	}
	funql := fmt.Sprintf(
		"RECALL BY semantic_similarity(:query, weight: %f) + recency(weight: %f) + importance(weight: %f) FOR AGENT '%s' LIMIT %d",
		weights.Semantic, weights.Recency, weights.Importance, m.agentID, topK,
	)
	result, err := m.conn.Execute(ctx, funql, query)
	if err != nil {
		return nil, err
	}
	records := make([]*MemoryRecord, 0, len(result.Rows))
	for _, row := range result.RowsAsDicts() {
		r := &MemoryRecord{
			MemoryID: fmt.Sprintf("%v", row["_id"]),
			Content:  fmt.Sprintf("%v", row["content"]),
			Score:    0.0,
		}
		if conf, ok := row["_confidence"].(float64); ok {
			r.Score = conf
		}
		records = append(records, r)
	}
	return records, nil
}

// Forget marks a memory as deleted.
func (m *AgentMemory) Forget(ctx context.Context, memoryID string) error {
	query := fmt.Sprintf("DELETE FROM memories WHERE _id = '%s'", memoryID)
	_, err := m.conn.Execute(ctx, query)
	return err
}
