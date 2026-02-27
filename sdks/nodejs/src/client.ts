import { Client as PgClient, DatabaseError } from 'pg';
import {
  Column,
  ConnectOptions,
  FeedbackSignal,
  QueryResult as IQueryResult,
} from './types';
import { AgentMemory } from './memory';
import { CausalPath } from './causal';
import { ConnectionError, QueryError, CausalError } from './errors';

// ---------------------------------------------------------------------------
// QueryResult implementation
// ---------------------------------------------------------------------------

class QueryResultImpl implements IQueryResult {
  readonly columns: Column[];
  readonly rows: unknown[][];
  readonly commandTag: string;
  readonly rowCount: number;

  constructor(
    columns: Column[],
    rows: unknown[][],
    commandTag: string,
    rowCount: number,
  ) {
    this.columns = columns;
    this.rows = rows;
    this.commandTag = commandTag;
    this.rowCount = rowCount;
  }

  rowsAsDicts(): Record<string, unknown>[] {
    return this.rows.map((row) => {
      const dict: Record<string, unknown> = {};
      this.columns.forEach((col, idx) => {
        dict[col.name] = row[idx];
      });
      return dict;
    });
  }

  scalar(): unknown {
    if (this.rows.length === 0) return null;
    return this.rows[0][0];
  }
}

function mapPgResult(pgResult: import('pg').QueryResult): IQueryResult {
  const columns: Column[] = (pgResult.fields ?? []).map((f) => ({
    name: f.name,
    typeOid: f.dataTypeID,
  }));

  const rows: unknown[][] = (pgResult.rows ?? []).map((row) => {
    if (Array.isArray(row)) return row as unknown[];
    // pg returns rows as objects keyed by column name
    return columns.map((col) => (row as Record<string, unknown>)[col.name]);
  });

  const commandTag = pgResult.command ?? '';
  const rowCount = pgResult.rowCount ?? rows.length;

  return new QueryResultImpl(columns, rows, commandTag, rowCount);
}

// ---------------------------------------------------------------------------
// FunDBConnection
// ---------------------------------------------------------------------------

export class FunDBConnection {
  readonly memory: AgentMemory;
  private readonly pgClient: PgClient;

  private constructor(pgClient: PgClient) {
    this.pgClient = pgClient;
    this.memory = new AgentMemory(pgClient);
  }

  static async connect(options?: ConnectOptions): Promise<FunDBConnection> {
    const pgClient = new PgClient({
      host: options?.host ?? 'localhost',
      port: options?.port ?? 5433,
      database: options?.database ?? 'fundb',
      user: options?.user ?? 'fundb',
      password: options?.password,
      ssl: options?.ssl ? { rejectUnauthorized: false } : false,
    });

    try {
      await pgClient.connect();
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new ConnectionError(
          `Failed to connect to FunDB: ${err.message}`,
          err,
        );
      }
      throw new ConnectionError('Failed to connect to FunDB', err);
    }

    return new FunDBConnection(pgClient);
  }

  async execute(query: string, params?: unknown[]): Promise<IQueryResult> {
    try {
      const pgResult = await this.pgClient.query(
        query,
        params as unknown[] | undefined,
      );
      return mapPgResult(pgResult);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new QueryError(`Query failed: ${err.message}`, query, err);
      }
      throw new QueryError('Query failed', query, err);
    }
  }

  async understand(
    intent: string,
    contextLimit = 10,
  ): Promise<IQueryResult> {
    const sql = `UNDERSTAND '${escapeString(intent)}' LIMIT ${contextLimit}`;
    try {
      const pgResult = await this.pgClient.query(sql);
      return mapPgResult(pgResult);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new QueryError(`UNDERSTAND failed: ${err.message}`, sql, err);
      }
      throw new QueryError('UNDERSTAND failed', sql, err);
    }
  }

  async traceCausality(
    fromId: string,
    toId: string,
    maxDepth = 5,
  ): Promise<CausalPath> {
    const sql = `CAUSAL TRACE FROM '${escapeString(fromId)}' TO '${escapeString(toId)}' MAX_DEPTH ${maxDepth}`;
    let pgResult: import('pg').QueryResult;
    try {
      pgResult = await this.pgClient.query(sql);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new CausalError(`CAUSAL TRACE failed: ${err.message}`, err);
      }
      throw new CausalError('CAUSAL TRACE failed', err);
    }

    const row = pgResult.rows[0] as Record<string, unknown> | undefined;
    if (!row) {
      throw new CausalError('CAUSAL TRACE returned no rows');
    }

    // The server may return the path as a JSON column or as individual columns.
    // Try parsing a 'result' or 'path' JSON column first, then fall back to the
    // whole row.
    const rawData: unknown =
      row['result'] ?? row['path'] ?? row['causal_path'] ?? row;

    return CausalPath.fromRaw(rawData);
  }

  async feedback(recordId: string, signal: FeedbackSignal): Promise<void> {
    const sql = `FEEDBACK ON '${escapeString(recordId)}' SIGNAL '${signal}'`;
    try {
      await this.pgClient.query(sql);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new QueryError(`FEEDBACK failed: ${err.message}`, sql, err);
      }
      throw new QueryError('FEEDBACK failed', sql, err);
    }
  }

  async close(): Promise<void> {
    try {
      await this.pgClient.end();
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new ConnectionError(`Failed to close connection: ${err.message}`, err);
      }
      throw new ConnectionError('Failed to close connection', err);
    }
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

export async function connect(
  options?: ConnectOptions,
): Promise<FunDBConnection> {
  return FunDBConnection.connect(options);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function escapeString(value: string): string {
  return value.replace(/'/g, "''");
}
