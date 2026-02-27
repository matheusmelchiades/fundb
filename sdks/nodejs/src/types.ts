export interface Column {
  name: string;
  typeOid: number;
}

export interface QueryResult {
  columns: Column[];
  rows: unknown[][];
  commandTag: string;
  rowCount: number;
  rowsAsDicts(): Record<string, unknown>[];
  scalar(): unknown;
}

export interface CausalEdge {
  sourceId: string;
  targetId: string;
  causalType: string;
  strength: number;
  stabilityScore?: number;
}

export interface CausalPath {
  nodes: string[];
  edges: CausalEdge[];
  totalStrength: number;
  minStability: number;
  toMermaid(): string;
  toJson(): string;
}

export type MemoryType = 'episodic' | 'semantic' | 'procedural' | 'working';
export type FeedbackSignal = 'used' | 'ignored' | 'promoted' | 'demoted';

export interface MemoryResult {
  id: string;
  content: string;
  memoryType: MemoryType;
  importance: number;
  score: number;
  createdAt: number;
}

export interface RecallWeights {
  semantic?: number;
  recency?: number;
  importance?: number;
}

export interface RememberOptions {
  memoryType?: MemoryType;
  importance?: number;
  tags?: string[];
}

export interface ConnectOptions {
  host?: string;
  port?: number;
  database?: string;
  user?: string;
  password?: string;
  ssl?: boolean;
}
