from dataclasses import dataclass, field
from typing import List, Optional
import uuid as _uuid

@dataclass
class RememberOptions:
    importance: float = 0.5
    memory_type: str = "semantic"  # "semantic" | "episodic" | "procedural"
    decay_rate: float = 0.01

@dataclass
class RecallWeights:
    semantic: float = 0.5
    recency: float = 0.3
    importance: float = 0.2

    def validate(self) -> None:
        total = self.semantic + self.recency + self.importance
        if abs(total - 1.0) > 0.01:
            raise ValueError(f"RecallWeights must sum to 1.0, got {total}")

@dataclass
class MemoryRecord:
    memory_id: str
    content: str
    score: float
    semantic_score: float = 0.0
    recency_score: float = 0.0
    importance_score: float = 0.0

class AgentMemory:
    """
    High-level agent memory helper.

    Uses the FunDB connection to store and retrieve memories via FunQL.

    Example:
        memory = AgentMemory(conn, agent_id="my-agent")
        memory_id = memory.remember("The user prefers Python", importance=0.8)
        results = memory.recall("programming language preference", top_k=5)
    """
    def __init__(self, conn: "FunDBConnection", agent_id: str):
        self._conn = conn
        self._agent_id = agent_id

    def remember(self, content: str, opts: Optional[RememberOptions] = None) -> str:
        """Store a memory and return its UUID."""
        if opts is None:
            opts = RememberOptions()
        memory_id = str(_uuid.uuid4())
        query = (
            f"REMEMBER '{content}' FOR AGENT '{self._agent_id}' "
            f"WITH importance {opts.importance} AS {opts.memory_type}"
        )
        self._conn.execute(query)
        return memory_id

    def recall(
        self,
        query: str,
        weights: Optional[RecallWeights] = None,
        top_k: int = 10,
    ) -> List[MemoryRecord]:
        """Recall memories by weighted query."""
        if weights is None:
            weights = RecallWeights()
        weights.validate()
        funql = (
            f"RECALL BY semantic_similarity(:query, weight: {weights.semantic}) "
            f"+ recency(weight: {weights.recency}) "
            f"+ importance(weight: {weights.importance}) "
            f"FOR AGENT '{self._agent_id}' LIMIT {top_k}"
        )
        result = self._conn.execute(funql, params={"query": query})
        return [
            MemoryRecord(
                memory_id=str(row.get("_id", "")),
                content=str(row.get("content", "")),
                score=float(row.get("_confidence", 0.0)),
            )
            for row in result.rows_as_dicts()
        ]

    def forget(self, memory_id: str) -> None:
        """Tombstone a memory by ID."""
        self._conn.execute(f"DELETE FROM memories WHERE _id = '{memory_id}'")
