export { connect, FunDBConnection } from './client';
export { AgentMemory } from './memory';
export { CausalPath } from './causal';
export { FunDBError, ConnectionError, QueryError, CausalError } from './errors';
export type {
  Column,
  QueryResult,
  CausalEdge,
  MemoryType,
  FeedbackSignal,
  MemoryResult,
  RecallWeights,
  RememberOptions,
  ConnectOptions,
} from './types';
