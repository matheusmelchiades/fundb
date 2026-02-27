import type { Client as PgClient, QueryResult as PgQueryResult } from 'pg';
import {
  MemoryResult,
  MemoryType,
  RecallWeights,
  RememberOptions,
} from './types';
import { QueryError } from './errors';
import { DatabaseError } from 'pg';

export class AgentMemory {
  private readonly pgClient: PgClient;

  constructor(pgClient: PgClient) {
    this.pgClient = pgClient;
  }

  async remember(content: string, options?: RememberOptions): Promise<string> {
    const memoryType: MemoryType = options?.memoryType ?? 'episodic';
    const importance: number = options?.importance ?? 0.8;
    const tags: string[] = options?.tags ?? [];

    const tagsClause =
      tags.length > 0 ? ` TAGS '${tags.map(escapeString).join(',')}'` : '';

    const sql = `REMEMBER '${escapeString(content)}' TYPE '${memoryType}' IMPORTANCE ${importance}${tagsClause}`;

    let result: PgQueryResult;
    try {
      result = await this.pgClient.query(sql);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new QueryError(`Memory remember failed: ${err.message}`, sql, err);
      }
      throw new QueryError('Memory remember failed', sql, err);
    }

    const row = result.rows[0] as Record<string, unknown> | undefined;
    if (!row) {
      throw new QueryError('REMEMBER returned no rows', sql);
    }

    const id = row['id'] ?? row['memory_id'];
    if (typeof id !== 'string') {
      throw new QueryError('REMEMBER returned unexpected row shape (no id)', sql);
    }
    return id;
  }

  async recall(
    query: string,
    limit = 5,
    weights?: RecallWeights,
  ): Promise<MemoryResult[]> {
    const semantic = weights?.semantic ?? 0.5;
    const recency = weights?.recency ?? 0.3;
    const importance = weights?.importance ?? 0.2;

    const sql =
      `RECALL '${escapeString(query)}' LIMIT ${limit} ` +
      `WEIGHTS semantic=${semantic} recency=${recency} importance=${importance}`;

    let result: PgQueryResult;
    try {
      result = await this.pgClient.query(sql);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new QueryError(`Memory recall failed: ${err.message}`, sql, err);
      }
      throw new QueryError('Memory recall failed', sql, err);
    }

    return result.rows.map((row, idx) => parseMemoryResult(row, idx));
  }

  async forget(memoryId: string): Promise<void> {
    const sql = `FORGET '${escapeString(memoryId)}'`;

    try {
      await this.pgClient.query(sql);
    } catch (err) {
      if (err instanceof DatabaseError) {
        throw new QueryError(`Memory forget failed: ${err.message}`, sql, err);
      }
      throw new QueryError('Memory forget failed', sql, err);
    }
  }
}

function escapeString(value: string): string {
  return value.replace(/'/g, "''");
}

function parseMemoryResult(row: unknown, idx: number): MemoryResult {
  if (typeof row !== 'object' || row === null) {
    throw new QueryError(`RECALL row[${idx}] is not an object`);
  }

  const r = row as Record<string, unknown>;

  const id = r['id'];
  const content = r['content'];
  const memoryType = r['memoryType'] ?? r['memory_type'];
  const importance = r['importance'];
  const score = r['score'];
  const createdAt = r['createdAt'] ?? r['created_at'];

  if (typeof id !== 'string') {
    throw new QueryError(`RECALL row[${idx}].id must be a string`);
  }
  if (typeof content !== 'string') {
    throw new QueryError(`RECALL row[${idx}].content must be a string`);
  }
  if (!isMemoryType(memoryType)) {
    throw new QueryError(`RECALL row[${idx}].memoryType has unexpected value: ${String(memoryType)}`);
  }
  if (typeof importance !== 'number') {
    throw new QueryError(`RECALL row[${idx}].importance must be a number`);
  }
  if (typeof score !== 'number') {
    throw new QueryError(`RECALL row[${idx}].score must be a number`);
  }
  if (typeof createdAt !== 'number') {
    throw new QueryError(`RECALL row[${idx}].createdAt must be a number`);
  }

  return { id, content, memoryType, importance, score, createdAt };
}

function isMemoryType(value: unknown): value is MemoryType {
  return (
    value === 'episodic' ||
    value === 'semantic' ||
    value === 'procedural' ||
    value === 'working'
  );
}
