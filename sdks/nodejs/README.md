# @fundb/client - Node.js SDK for FunDB

Official Node.js/TypeScript client for [FunDB](https://fundb.io), an AI-native cognitive database.

## Installation

```bash
npm install @fundb/client
```

## Quick start

```typescript
import { connect } from '@fundb/client';

const conn = await connect({ host: 'localhost', port: 5433 });
const result = await conn.execute('SELECT * FROM documents LIMIT 5');
console.log(result.rowsAsDicts());
await conn.close();
```

## Connection options

```typescript
const conn = await connect({
  host: 'localhost',   // default: 'localhost'
  port: 5433,          // default: 5433
  database: 'fundb',   // default: 'fundb'
  user: 'fundb',       // default: 'fundb'
  password: 'secret',
  ssl: false,          // default: false
});
```

Using `Symbol.asyncDispose` (requires TypeScript 5.2+ / `using` keyword):

```typescript
await using conn = await connect({ host: 'localhost' });
const result = await conn.execute('SELECT 1');
// conn.close() is called automatically on scope exit
```

## Methods

### `execute(query, params?)`

Run SQL against FunDB.

```typescript
const result = await conn.execute(
  'SELECT * FROM documents WHERE id = $1',
  ['abc-123'],
);
console.log(result.rows);
console.log(result.rowsAsDicts());
console.log(result.scalar()); // first column of first row
```

### `understand(intent, contextLimit?)`

Perform a semantic / natural-language query.

```typescript
const result = await conn.understand('find recent purchase orders', 10);
console.log(result.rowsAsDicts());
```

### `traceCausality(fromId, toId, maxDepth?)`

Trace the causal path between two records.

```typescript
const path = await conn.traceCausality('uuid-a', 'uuid-b', 5);
console.log(path.totalStrength);
console.log(path.toMermaid());
console.log(path.toJson());
```

### `feedback(recordId, signal)`

Send a learning-to-rank signal for a record.

```typescript
await conn.feedback('uuid-a', 'promoted');
// signal: 'used' | 'ignored' | 'promoted' | 'demoted'
```

### `conn.memory.remember(content, options?)`

Store a memory and return its ID.

```typescript
const memId = await conn.memory.remember('User prefers dark mode', {
  memoryType: 'semantic',
  importance: 0.9,
  tags: ['preferences', 'ui'],
});
```

### `conn.memory.recall(query, limit?, weights?)`

Retrieve memories ranked by relevance.

```typescript
const memories = await conn.memory.recall('user preferences', 5, {
  semantic: 0.6,
  recency: 0.2,
  importance: 0.2,
});
for (const m of memories) {
  console.log(m.content, m.score);
}
```

### `conn.memory.forget(memoryId)`

Delete a memory by ID.

```typescript
await conn.memory.forget(memId);
```

### `conn.close()`

Close the underlying connection.

```typescript
await conn.close();
```

## Error handling

All errors extend `FunDBError` for easy catching:

```typescript
import { FunDBError, ConnectionError, QueryError, CausalError } from '@fundb/client';

try {
  await conn.execute('INVALID SQL');
} catch (err) {
  if (err instanceof QueryError) {
    console.error('Query failed:', err.message, 'SQL:', err.query);
  } else if (err instanceof FunDBError) {
    console.error('FunDB error:', err.message);
  }
}
```

## License

Apache-2.0
