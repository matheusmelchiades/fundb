class FunDBError(Exception):
    """Base exception for all FunDB errors."""

class ConnectionError(FunDBError):
    """Raised when connection to FunDB fails."""

class QueryError(FunDBError):
    """Raised when a query fails."""
    def __init__(self, message: str, query: str = ""):
        super().__init__(message)
        self.query = query

class CausalError(FunDBError):
    """Raised for causal query errors."""
