"""
FunDB Python client.

Synchronous usage (via psycopg2):
    conn = FunDBConnection.connect(host="localhost", port=5433, database="fundb")
    result = conn.execute("SELECT * FROM documents")
    conn.understand("find papers about attention")
    conn.close()

Context manager:
    with FunDBConnection.connect(...) as conn:
        result = conn.execute("SELECT 1")

Async usage (via asyncpg):
    conn = await AsyncFunDBConnection.connect(...)
    result = await conn.execute(...)
"""
from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Dict, Iterator, List, Optional, Sequence, Union

from .causal import CausalPath, CausalEdge
from .exceptions import ConnectionError, QueryError

# ── Result types ──────────────────────────────────────────────────────────────

@dataclass
class Column:
    name: str
    type_oid: int = 25  # text

@dataclass
class QueryResult:
    columns: List[Column]
    rows: List[List[Any]]
    command_tag: str
    row_count: int = 0

    def rows_as_dicts(self) -> List[Dict[str, Any]]:
        """Return rows as list of dicts keyed by column name."""
        col_names = [c.name for c in self.columns]
        return [dict(zip(col_names, row)) for row in self.rows]

    def __iter__(self) -> Iterator[Dict[str, Any]]:
        return iter(self.rows_as_dicts())

    def __len__(self) -> int:
        return len(self.rows)

    def scalar(self) -> Any:
        """Return the first cell of the first row, or None."""
        if self.rows and self.rows[0]:
            return self.rows[0][0]
        return None

# ── Synchronous connection ────────────────────────────────────────────────────

class FunDBConnection:
    """
    Synchronous FunDB connection using psycopg2.

    FunDB speaks the PostgreSQL wire protocol, so standard PG clients work.
    """
    def __init__(self, _pg_conn: Any):
        self._conn = _pg_conn
        self._cursor = _pg_conn.cursor()

    @classmethod
    def connect(
        cls,
        host: str = "localhost",
        port: int = 5433,
        database: str = "fundb",
        user: str = "fundb",
        password: str = "",
    ) -> "FunDBConnection":
        """
        Open a synchronous connection.

        Requires psycopg2: pip install psycopg2-binary
        """
        try:
            import psycopg2
        except ImportError:
            raise ConnectionError("psycopg2 not installed. Run: pip install psycopg2-binary")
        try:
            pg_conn = psycopg2.connect(
                host=host, port=port, dbname=database, user=user, password=password
            )
            pg_conn.autocommit = True
            return cls(pg_conn)
        except Exception as e:
            raise ConnectionError(f"Failed to connect to FunDB at {host}:{port}: {e}")

    def execute(self, query: str, params: Optional[Dict[str, Any]] = None) -> QueryResult:
        """Execute a FunQL query and return results."""
        try:
            if params:
                # Convert :param_name style to %(param_name)s for psycopg2
                pg_query = query
                for key in params:
                    pg_query = pg_query.replace(f":{key}", f"%({key})s")
                self._cursor.execute(pg_query, params)
            else:
                self._cursor.execute(query)

            columns = []
            if self._cursor.description:
                columns = [Column(name=desc[0], type_oid=desc[1] or 25)
                           for desc in self._cursor.description]

            rows = self._cursor.fetchall() if self._cursor.description else []
            tag = self._cursor.statusmessage or "OK"
            return QueryResult(columns=columns, rows=[list(r) for r in rows],
                               command_tag=tag, row_count=len(rows))
        except Exception as e:
            raise QueryError(str(e), query=query)

    def understand(self, intent: str, collection: Optional[str] = None) -> QueryResult:
        """Execute a semantic UNDERSTAND query."""
        q = f"UNDERSTAND \"{intent}\""
        if collection:
            q += f" IN COLLECTION {collection}"
        return self.execute(q)

    def trace_causality(
        self,
        from_id: str,
        to_id: str,
        max_depth: int = 5,
        min_strength: float = 0.3,
    ) -> List[CausalPath]:
        """Trace causal paths between two nodes."""
        query = (
            f"SELECT * FROM events "
            f"TRACE CAUSALITY FROM '{from_id}' TO '{to_id}' "
            f"MAX_DEPTH {max_depth} "
            f"MIN_STRENGTH {min_strength} "
            f"RETURN causal_path, total_strength"
        )
        result = self.execute(query)
        # Parse causal path rows into CausalPath objects
        paths = []
        for row_dict in result.rows_as_dicts():
            path_json = row_dict.get("causal_path", "{}")
            if isinstance(path_json, str):
                try:
                    data = json.loads(path_json)
                    nodes = data.get("nodes", [])
                    edges = [
                        CausalEdge(
                            source_id=e.get("source_id", ""),
                            target_id=e.get("target_id", ""),
                            strength=e.get("strength", 0.0),
                        )
                        for e in data.get("edges", [])
                    ]
                    paths.append(CausalPath(
                        nodes=nodes, edges=edges,
                        total_strength=float(row_dict.get("total_strength", 0.0)),
                        min_stability=1.0,
                    ))
                except (json.JSONDecodeError, TypeError):
                    pass
        return paths

    def feedback(
        self,
        query_id: str,
        used: Sequence[str],
        ignored: Sequence[str],
    ) -> None:
        """Record learning-to-rank feedback for a query."""
        # Stub: would call the LTR endpoint via HTTP or a special FunQL command
        pass

    def memory(self, agent_id: str) -> "AgentMemory":
        """Get an AgentMemory helper for this connection."""
        from .memory import AgentMemory
        return AgentMemory(self, agent_id)

    def close(self) -> None:
        """Close the connection."""
        self._cursor.close()
        self._conn.close()

    def __enter__(self) -> "FunDBConnection":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()


# ── Async connection ──────────────────────────────────────────────────────────

class AsyncFunDBConnection:
    """
    Asynchronous FunDB connection using asyncpg.

    Example:
        conn = await AsyncFunDBConnection.connect(host="localhost", port=5433)
        result = await conn.execute("SELECT * FROM documents")
        await conn.close()
    """
    def __init__(self, _asyncpg_conn: Any):
        self._conn = _asyncpg_conn

    @classmethod
    async def connect(
        cls,
        host: str = "localhost",
        port: int = 5433,
        database: str = "fundb",
        user: str = "fundb",
        password: str = "",
    ) -> "AsyncFunDBConnection":
        """Open an async connection. Requires asyncpg."""
        try:
            import asyncpg
        except ImportError:
            raise ConnectionError("asyncpg not installed. Run: pip install asyncpg")
        try:
            conn = await asyncpg.connect(
                host=host, port=port, database=database, user=user, password=password
            )
            return cls(conn)
        except Exception as e:
            raise ConnectionError(f"Failed to connect to FunDB at {host}:{port}: {e}")

    async def execute(self, query: str, *args: Any) -> QueryResult:
        """Execute a query asynchronously."""
        try:
            rows = await self._conn.fetch(query, *args)
            if not rows:
                return QueryResult(columns=[], rows=[], command_tag="OK", row_count=0)
            columns = [Column(name=k) for k in rows[0].keys()]
            data = [list(r.values()) for r in rows]
            return QueryResult(columns=columns, rows=data,
                               command_tag=f"SELECT {len(data)}", row_count=len(data))
        except Exception as e:
            raise QueryError(str(e), query=query)

    async def understand(self, intent: str, collection: Optional[str] = None) -> QueryResult:
        q = f"UNDERSTAND \"{intent}\""
        if collection:
            q += f" IN COLLECTION {collection}"
        return await self.execute(q)

    async def close(self) -> None:
        await self._conn.close()

    async def __aenter__(self) -> "AsyncFunDBConnection":
        return self

    async def __aexit__(self, *args: Any) -> None:
        await self.close()
