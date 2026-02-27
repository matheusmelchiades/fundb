# FunDB — Contributing Guide (Parallel Agent Edition)

## Branch Strategy

### Rules (mandatory for parallel agent work)
1. `main` — always compiles, all tests pass. Never push directly.
2. One branch per Story. One agent per branch. No exceptions.
3. Branches are short-lived. Merge when Story DoD is complete.
4. Merge via squash: one clean commit per story on `main`.

### Branch naming
```
feat/story-{epic}-{n}-{short-name}

Examples:
  feat/story-1-1-funrecord
  feat/story-1-2-codec
  feat/story-1-3-funql-lexer
  feat/story-1-4-pg-wire
  feat/story-2-1-memtable
  feat/story-3-2-hnsw
  feat/story-5-4-causal-tier1
```

### Starting a story (agent instructions)
```bash
git checkout main
git pull origin main
git checkout -b feat/story-{epic}-{n}-{short-name}
# ... implement ...
git add crates/<your-crate>/
git commit -m "feat(story-{epic}-{n}): <description>"
# open PR → squash merge to main when DoD passes
```

---

## Conflict Prevention Rules

The single most important rule: **each story owns exactly one crate**.

| Story | Crate | Files agent may touch |
|-------|-------|-----------------------|
| STORY-1-1 | `crates/fundb-core` | `src/record.rs`, `src/types.rs`, `src/lib.rs` |
| STORY-1-2 | `crates/fundb-core` | `src/codec.rs`, `src/page.rs` |
| STORY-1-3 | `crates/fundb-sql` | `src/lexer.rs`, `src/parser.rs`, `src/ast.rs`, `src/grammar.lalrpop` |
| STORY-1-4 | `crates/fundb-protocol` | `src/pg_wire.rs`, `crates/fundb-server/src/main.rs` |
| STORY-2-1 | `crates/fundb-storage` | `src/memtable.rs` |
| STORY-2-2 | `crates/fundb-storage` | `src/wal.rs` |
| STORY-2-3 | `crates/fundb-storage` | `src/sstable.rs`, `src/block_cache.rs` |
| STORY-2-4 | `crates/fundb-storage` | `src/mvcc.rs`, `src/lib.rs` |
| STORY-3-1 | `crates/fundb-indexes` | `src/btree.rs` |
| STORY-3-2 | `crates/fundb-indexes` | `src/hnsw.rs`, `src/pq.rs` |
| STORY-3-3 | `crates/fundb-indexes` | `src/graph.rs` |
| STORY-3-4 | `crates/fundb-indexes` | `src/temporal.rs` |
| STORY-3-5 | `crates/fundb-indexes` | `src/causal_dag.rs`, `src/confidence.rs` |
| STORY-4-1 | `crates/fundb-sql` | `src/binder.rs`, `src/logical_plan.rs` |
| STORY-4-2 | `crates/fundb-optimizer` | `src/rules.rs`, `src/lib.rs` |
| STORY-4-3 | `crates/fundb-executor` | `src/operators/`, `src/batch.rs`, `src/lib.rs` |
| STORY-4-4 | `crates/fundb-storage` | `src/lsm.rs`, `src/compaction.rs` |
| STORY-5-1 | `crates/fundb-cognitive` | `src/confidence.rs` |
| STORY-5-2 | `crates/fundb-cognitive` | `src/context.rs` |
| STORY-5-3 | `crates/fundb-cognitive` | `src/contradiction.rs` |
| STORY-5-4 | `crates/fundb-causal` | `src/tier1.rs` |
| STORY-5-5 | `crates/fundb-cognitive` | `src/memory.rs` |
| STORY-6-1 | `crates/fundb-semantic` | `src/lib.rs`, `src/tier1.rs`, `src/tier2.rs` |
| STORY-6-2 | `crates/fundb-causal` | `src/tier2.rs`, `src/granger.rs`, `src/llm_oracle.rs` |
| STORY-6-3 | `crates/fundb-learning` | `src/ltr.rs` |
| STORY-6-4 | `crates/fundb-optimizer` | `src/cost.rs`, `src/statistics.rs` |
| STORY-7-1 | `crates/fundb-protocol` | `src/pg_wire.rs` (extends stub from 1-4) |
| STORY-7-2 | `crates/fundb-protocol` | `src/grpc.rs`, `src/rest.rs` |
| STORY-7-3 | `sdks/python/` | all python files |
| STORY-7-4 | `sdks/go/` | all go files |
| STORY-8-1 | `crates/fundb-raft` | `src/lib.rs`, `src/node.rs`, `src/log.rs` |
| STORY-8-2 | `crates/fundb-cluster` | `src/sharding.rs` |
| STORY-8-3 | `crates/fundb-cluster` | `src/distributed_query.rs` |
| STORY-9-1 | `crates/fundb-causal` | `src/tier3.rs`, `src/scm.rs`, `src/discovery.rs` |
| STORY-10-1 | `crates/fundb-server` | `src/security.rs`, `src/rbac.rs` |
| STORY-10-2 | `crates/fundb-server` | `src/metrics.rs`, `src/tracing.rs` |
| STORY-10-3 | `tests/benchmarks/` | all benchmark files |

**If two stories touch the same crate:** they MUST be on separate files. Never edit the same file as another active story.

---

## Interface Contract Protocol

Before writing any implementation, the agent MUST create the public interface stub:

```rust
// src/memtable.rs — stub created at branch start
pub struct MemTable;
impl MemTable {
    pub fn insert(&self, key: RecordKey, record: FunRecord) -> Result<()> { todo!() }
    pub fn get(&self, key: &RecordKey) -> Option<FunRecord> { todo!() }
    pub fn range(&self, from: &RecordKey, to: &RecordKey) -> impl Iterator<Item=(RecordKey, FunRecord)> { todo!() }
    pub fn size_bytes(&self) -> usize { todo!() }
    pub fn freeze(self) -> ImmutableMemTable { todo!() }
}
```

Commit the stub **immediately** so dependent agents can start against it.
Tag the commit: `chore(story-2-1): publish interface contract`

Then implement, and the dependent story can merge `main` to pick up the stub.

---

## Commit Format

```
<type>(story-{epic}-{n}): <description>

Types: feat | fix | test | refactor | chore | docs
```

Examples:
```
feat(story-1-1): implement FunRecord with all cognitive fields
test(story-3-2): add recall@10 benchmark for HNSW 100K vectors
fix(story-2-4): correct MVCC snapshot isolation for concurrent reads
chore(story-1-1): publish interface contract stubs
```

---

## PR Checklist (required before merging to main)

- [ ] All Story DoD checkboxes from `docs/BACKLOG.md` pass
- [ ] `cargo test -p <your-crate>` passes with zero failures
- [ ] `cargo clippy -p <your-crate> -- -D warnings` clean
- [ ] No files outside your story's file ownership table above were modified
- [ ] Interface contract (pub API) is stable — if it changed, notify dependent story agents

---

## File Ownership Shortcut

```
Story needs to edit a file already "owned" by another story?
  → STOP. The two stories share a dependency.
  → Check BACKLOG.md interface contract — the API should be enough.
  → If not, raise it as a new STORY or OQ before touching the file.
```

---

## Merging Order for Stories with Shared Crates

`fundb-storage` is shared by stories 2-1, 2-2, 2-3, 2-4 and 4-4.
They are on separate files — but `lib.rs` needs all of them.
**Resolution:** Last story to merge in that sprint is responsible for updating `src/lib.rs` to `pub mod` all modules. Order:

```
2-1 (memtable.rs) → merges → exports pub mod memtable
2-2 (wal.rs)      → merges → exports pub mod wal
2-3 (sstable.rs)  → merges → exports pub mod sstable
2-4 (mvcc.rs)     → merges → exports pub mod mvcc (also updates lib.rs with all four)
```

Same pattern for `fundb-indexes` (stories 3-1 to 3-5) and `fundb-cognitive` (5-1 to 5-3, 5-5).

---

## Quick Reference

```bash
# Start a new story
git checkout main && git pull
git checkout -b feat/story-{epic}-{n}-{name}

# Publish interface contract (do this first!)
# write todo!() stubs → git add → git commit -m "chore(story-N-N): publish interface contract"

# Keep branch up to date (rebase, not merge)
git fetch origin
git rebase origin/main

# Submit for merge
git push origin feat/story-{epic}-{n}-{name}
# open PR → squash merge after DoD passes
```
