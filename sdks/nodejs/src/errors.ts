export class FunDBError extends Error {
  constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = 'FunDBError';
    // Maintains proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class ConnectionError extends FunDBError {
  constructor(message: string, cause?: unknown) {
    super(message, cause);
    this.name = 'ConnectionError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class QueryError extends FunDBError {
  constructor(
    message: string,
    public readonly query?: string,
    cause?: unknown,
  ) {
    super(message, cause);
    this.name = 'QueryError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class CausalError extends FunDBError {
  constructor(message: string, cause?: unknown) {
    super(message, cause);
    this.name = 'CausalError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}
