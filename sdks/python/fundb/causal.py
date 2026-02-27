from dataclasses import dataclass, field
from typing import List, Optional
import json

@dataclass
class CausalEdge:
    source_id: str
    target_id: str
    strength: float
    stability_score: float = 1.0

@dataclass
class CausalPath:
    nodes: List[str]
    edges: List[CausalEdge]
    total_strength: float
    min_stability: float

    def to_dict(self) -> dict:
        return {
            "nodes": self.nodes,
            "edges": [
                {"source_id": e.source_id, "target_id": e.target_id,
                 "strength": e.strength, "stability_score": e.stability_score}
                for e in self.edges
            ],
            "total_strength": self.total_strength,
            "min_stability": self.min_stability,
        }

    def to_mermaid(self) -> str:
        lines = ["graph LR"]
        for edge in self.edges:
            src = edge.source_id[:8].replace("-", "_")
            tgt = edge.target_id[:8].replace("-", "_")
            lines.append(f"    {src} -->|{edge.strength:.2f}| {tgt}")
        return "\n".join(lines)

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2)
