package fundb

import (
	"encoding/json"
	"fmt"
	"strings"
)

// CausalEdge represents a directed causal relationship.
type CausalEdge struct {
	SourceID       string  `json:"source_id"`
	TargetID       string  `json:"target_id"`
	Strength       float64 `json:"strength"`
	StabilityScore float64 `json:"stability_score"`
}

// CausalPath represents a sequence of causal edges connecting two nodes.
type CausalPath struct {
	Nodes         []string     `json:"nodes"`
	Edges         []CausalEdge `json:"edges"`
	TotalStrength float64      `json:"total_strength"`
	MinStability  float64      `json:"min_stability"`
}

// ToDict returns the path as a map for JSON serialization.
func (p *CausalPath) ToDict() map[string]interface{} {
	edges := make([]map[string]interface{}, 0, len(p.Edges))
	for _, e := range p.Edges {
		edges = append(edges, map[string]interface{}{
			"source_id":       e.SourceID,
			"target_id":       e.TargetID,
			"strength":        e.Strength,
			"stability_score": e.StabilityScore,
		})
	}
	return map[string]interface{}{
		"nodes":          p.Nodes,
		"edges":          edges,
		"total_strength": p.TotalStrength,
		"min_stability":  p.MinStability,
	}
}

// ToMermaid returns a Mermaid diagram string for this path.
func (p *CausalPath) ToMermaid() string {
	var sb strings.Builder
	sb.WriteString("graph LR\n")
	for _, edge := range p.Edges {
		src := edge.SourceID
		if len(src) > 8 {
			src = strings.ReplaceAll(src[:8], "-", "_")
		}
		tgt := edge.TargetID
		if len(tgt) > 8 {
			tgt = strings.ReplaceAll(tgt[:8], "-", "_")
		}
		sb.WriteString(fmt.Sprintf("    %s -->|%.2f| %s\n", src, edge.Strength, tgt))
	}
	return sb.String()
}

// ToJSON returns a JSON string representation of the path.
func (p *CausalPath) ToJSON() (string, error) {
	b, err := json.MarshalIndent(p.ToDict(), "", "  ")
	if err != nil {
		return "", err
	}
	return string(b), nil
}

// TraceCausalityOptions configures a causal trace query.
type TraceCausalityOptions struct {
	From         string
	To           string
	MaxDepth     int
	MinStrength  float64
	MinStability float64
}
