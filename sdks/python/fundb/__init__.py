"""
FunDB Python SDK — connect to FunDB, run FunQL queries, use cognitive features.

Quick start:
    from fundb import connect
    conn = connect()
    result = conn.execute("SELECT * FROM documents LIMIT 10")
    for row in result:
        print(row)

Async:
    from fundb import async_connect
    conn = await async_connect()
    result = await conn.execute("SELECT 1")
"""
from .client import FunDBConnection, AsyncFunDBConnection, QueryResult, Column
from .causal import CausalPath, CausalEdge
from .memory import AgentMemory, RecallWeights, RememberOptions, MemoryRecord
from .exceptions import FunDBError, ConnectionError, QueryError, CausalError

__version__ = "0.1.0"
__all__ = [
    "FunDBConnection", "AsyncFunDBConnection", "QueryResult", "Column",
    "CausalPath", "CausalEdge",
    "AgentMemory", "RecallWeights", "RememberOptions", "MemoryRecord",
    "FunDBError", "ConnectionError", "QueryError", "CausalError",
    "connect", "async_connect",
]

def connect(
    host: str = "localhost",
    port: int = 5433,
    database: str = "fundb",
    user: str = "fundb",
    password: str = "",
) -> FunDBConnection:
    """Open a synchronous connection to FunDB."""
    return FunDBConnection.connect(host=host, port=port, database=database,
                                   user=user, password=password)

async def async_connect(
    host: str = "localhost",
    port: int = 5433,
    database: str = "fundb",
    user: str = "fundb",
    password: str = "",
) -> AsyncFunDBConnection:
    """Open an async connection to FunDB."""
    return await AsyncFunDBConnection.connect(host=host, port=port,
                                              database=database, user=user,
                                              password=password)
