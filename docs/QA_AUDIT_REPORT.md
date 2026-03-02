# FunDB — QA Audit Report & Action Items

> **Auditor:** QA Senior Engineer
> **Data:** 2026-03-02
> **Branch:** `feat/insert-pipeline-and-vscode-extension`
> **Rust:** 1.93.1 · **Plataforma:** macOS Darwin 25.3.0 (aarch64)

---

## Status Geral: 🟡 Parcial

O projeto compila, **462 testes passam** consistentemente (zero flaky em 3 runs), mas existem **3 bugs bloqueantes** no pipeline de query e o servidor HTTP REST não está ativo no binário de produção.

---

## Sumário Executivo

| Área | Status | Resumo |
|------|--------|--------|
| Build debug | 🟢 | Compila com 14 warnings |
| Build release | 🟢 | Binários gerados OK (3.0MB server, 1.6MB cli) |
| Clippy `-D warnings` | 🔴 | 12 erros em 3 crates (`fundb-cli`, `fundb-learning`, `fundb-protocol`) |
| `cargo fmt --check` | 🔴 | Diffs em 6 crates |
| Testes unitários | 🟢 | 462/462 pass, 0 fail, 0 skip, 0 flaky |
| Testes integração | 🟢 | 107/107 pass |
| PG Wire Protocol | 🟢 | INSERT, SELECT, LIMIT funcionam via CLI |
| WHERE (data fields) | 🔴 | Não filtra — retorna todas as rows |
| ORDER BY | 🔴 | Completamente ignorado (stub) |
| HTTP REST API | 🔴 | Servidor HTTP não é iniciado no binário |
| Docker single-node | 🟡 | Container roda mas health check falha |

---

## Bugs Encontrados

### 🔴 Blocker (3)

#### BUG-001: WHERE não filtra campos de dados do usuário
- **Localização:** `crates/fundb-executor/src/operators/filter.rs:120-176`
- **Reprodução:** `SELECT * FROM t WHERE age > 25` retorna rows com age=20
- **Causa raiz:** `FilterOperator.eval_binary()` retorna `true` (passthrough) para qualquer coluna não reconhecida. Apenas `_confidence`, `_collection` e `_tenant` são avaliadas. Campos do usuário são armazenados em MessagePack e nunca são desserializados para comparação.
- **Impacto:** Toda query com WHERE em campos de dados retorna resultado incorreto.

#### BUG-002: ORDER BY completamente ignorado
- **Localização:** `crates/fundb-executor/src/lib.rs:141-145`
- **Reprodução:** `SELECT * FROM t ORDER BY age` retorna na ordem de inserção
- **Causa raiz:** Pattern match descarta `order_by` com `_`:
  ```rust
  LogicalPlan::Sort { input, order_by: _ } => {
      self.execute_plan(*input).await
  }
  ```
- **Impacto:** Toda query com ORDER BY retorna resultado não-ordenado.

#### BUG-003: HTTP REST API não é inicializado
- **Localização:** `crates/fundb-server/src/main.rs`
- **Reprodução:** `curl http://localhost:8080/health` → connection reset
- **Causa raiz:** `main.rs` só inicia o PG wire listener na porta 5433. O `HttpServer` de `http.rs` nunca é instanciado. A porta 8080 é exposta no Docker mas não servida.
- **Impacto:** REST API inteira indisponível. Docker health check falha.

### 🟠 Critical (2)

#### BUG-004: CREATE TABLE retorna `SELECT 0`
- **Reprodução:** `CREATE TABLE t (name TEXT)` → `SELECT 0` (sem DDL handler)
- **Causa raiz:** Não há `LogicalPlan::CreateTable` nem handler. Collections são criadas implicitamente no INSERT.

#### BUG-005: Docker container "unhealthy"
- **Reprodução:** `docker ps` mostra status `(unhealthy)` mesmo com servidor funcional
- **Causa raiz:** Docker healthcheck testa TCP port 5433 mas container reporta unhealthy. Provavelmente timing issue ou falta de HTTP health endpoint.

### 🟡 Major (4)

#### BUG-006: Clippy falha com `-D warnings`
- CI define `RUSTFLAGS: "-D warnings"` → **CI quebraria** na branch atual.
- 12 erros: doc comments (`fundb-cli`), dead code (`fundb-learning`), byte str (`fundb-protocol`).

#### BUG-007: Formatação inconsistente
- `cargo fmt --check` falha em 6 crates.

#### BUG-008: 11+ `panic!()` no optimizer
- `crates/fundb-optimizer/src/rules.rs` usa `panic!` em pattern matches inesperados.
- Qualquer plano inesperado causa crash do servidor em produção.

#### BUG-009: 337 chamadas `.unwrap()` no codebase
- Risco de panic não-controlado em produção. Pontos críticos:
  - `fundb-executor/src/lib.rs:180` — `rmp_serde::to_vec().unwrap_or_default()`
  - `fundb-storage/src/mvcc.rs:207` — `.expect("poisoned")`
  - `fundb-cluster/src/sharding.rs:166` — `.expect("shard has no owner")`

### 🔵 Minor (2)

#### BUG-010: 14 compiler warnings
- Unused imports, dead code, unused mut vars em 8 crates.

#### BUG-011: StubHandler dead code
- `crates/fundb-server/src/stub_handler.rs` — struct nunca construída.

---

## Testes Automatizados — Detalhamento

| Crate | Unit | Integration | DocTests | Total | Cobertura |
|-------|------|-------------|----------|-------|-----------|
| fundb-causal | 31 | 14 | 0 | 45 | C |
| fundb-cluster | 20 | 10 | 0 | 30 | C+ |
| fundb-cognitive | 55 | 13 | 0 | 68 | C |
| fundb-core | 20 | 0 | 2 | 22 | C+ |
| fundb-executor | 8 | 0 | 0 | 8 | D |
| fundb-indexes | 34 | 11 | 0 | 45 | C |
| fundb-learning | 8 | 0 | 0 | 8 | F |
| fundb-optimizer | 17 | 0 | 0 | 17 | D |
| fundb-protocol | 33 | 8 | 0 | 41 | C |
| fundb-raft | 12 | 0 | 0 | 12 | D |
| fundb-semantic | 14 | 0 | 0 | 14 | D |
| fundb-server | 42 | 11 | 0 | 53 | D* |
| fundb-sql | 65 | 14 | 0 | 79 | C |
| fundb-storage | 44 | 12 | 0 | 56 | B- |
| **fundb-cli** | **0** | **0** | **0** | **0** | **F** |
| **TOTAL** | **403** | **93** | **2** | **462** | — |

> \* fundb-server tem 42 testes mas todos testam o StubHandler, não o FunDBHandler real.

**Property-based testing:** `proptest` está nas workspace deps mas **zero** `proptest!` macros no código.
**Benchmarks:** 2 arquivos criterion existem (`query_bench.rs`, `storage_bench.rs`) mas **não rodam no CI**.
**Flaky tests:** 0 em 3 runs consecutivos.
**Testes ignorados:** 0 `#[ignore]` encontrados.

---

## Action Items

### P0 — Blockers (corrigir imediatamente)

- [ ] **FIX-001:** Implementar FilterOperator para campos de dados MessagePack
  - Desserializar `FunRecord.data` no `eval_binary()` para campos não-builtin
  - Suportar comparações: `>`, `<`, `>=`, `<=`, `=`, `!=` para strings e números
  - Arquivo: `crates/fundb-executor/src/operators/filter.rs`
  - Testes: queries `WHERE age > 25`, `WHERE name = 'Alice'`, `WHERE age = NULL`

- [ ] **FIX-002:** Implementar Sort operator
  - Substituir stub por sort in-memory sobre `RecordBatch`
  - Suportar: ASC/DESC, multi-column sort, campos de dados MessagePack
  - Arquivo: `crates/fundb-executor/src/lib.rs:141-145`
  - Testes: `ORDER BY age ASC`, `ORDER BY age DESC`, `ORDER BY name, age`

- [ ] **FIX-003:** Iniciar HttpServer no binário do servidor
  - Adicionar `tokio::spawn(HttpServer::new("0.0.0.0:8080").run())` no `main.rs`
  - Arquivo: `crates/fundb-server/src/main.rs`
  - Testes: `curl http://localhost:8080/health` → `{"status":"ok","version":"0.1.0"}`

### P1 — Critical (corrigir antes de release)

- [ ] **FIX-004:** Corrigir todos os 12 erros de clippy
  - `fundb-cli`: converter doc comments para inner (`//!`), usar `strip_prefix()`/`strip_suffix()`/`split_once()`, byte strings
  - `fundb-learning`: adicionar `Default` impl para `LtrRanker`, marcar `query_id` com `_`
  - `fundb-protocol`: usar byte str notation
  - Validar: `cargo clippy --workspace --all-targets -- -D warnings`

- [ ] **FIX-005:** Corrigir formatação
  - Executar: `cargo fmt --all`
  - Validar: `cargo fmt --all --check`

- [ ] **FIX-006:** Corrigir Docker healthcheck
  - Opção A: Após FIX-003, usar `curl -f http://localhost:8080/health` no healthcheck
  - Opção B: Usar query PG wire como healthcheck: enviar bytes de startup protocol

- [ ] **FIX-007:** Limpar 14 compiler warnings
  - Remover unused imports em `fundb-core`, `fundb-optimizer`, `fundb-causal`
  - Remover unused variables em `fundb-raft`, `fundb-indexes`
  - Remover `StubHandler` de `fundb-server/src/stub_handler.rs`

### P2 — Cobertura de Testes (próximo sprint)

- [ ] **TEST-001:** Criar suite de testes para `fundb-cli` (0% cobertura atual)
  - Testes unitários: args parsing, command parsing, display rendering
  - Testes de integração: conexão, formatos de output, erro de SQL

- [ ] **TEST-002:** Testes para FunDBHandler real (não stub)
  - INSERT → SELECT roundtrip end-to-end via handler
  - Error handling: SQL inválido, collection inexistente, tipos incompatíveis

- [ ] **TEST-003:** Testes de NULL handling em todo o pipeline
  - `INSERT INTO t (name) VALUES (NULL)`
  - `SELECT * FROM t WHERE name IS NULL`
  - NULL em projections, filters, joins

- [ ] **TEST-004:** Testes de unicode e caracteres especiais
  - Emoji em collection names, field names, values
  - Strings vazias, null bytes, RTL text
  - Nomes muito longos (>1000 chars)

- [ ] **TEST-005:** Testes WAL crash recovery avançados
  - Corrupção mid-frame (bytes inválidos no meio de um entry)
  - WAL file vazio / truncado
  - Múltiplas corrupções no mesmo arquivo
  - Recovery com SSTables ausentes

- [ ] **TEST-006:** Testes MVCC concurrency stress
  - GC rodando enquanto leituras ativas
  - N writers concorrentes na mesma key
  - Snapshot isolation sob carga
  - Clock skew (timestamps fora de ordem)

- [ ] **TEST-007:** Testes de large payloads
  - Records >1MB de dados
  - Vectors com >10K dimensões
  - Bulk INSERT de 100K+ records
  - SELECT com result set >100K rows

### P3 — Hardening (médio prazo)

- [ ] **HARDEN-001:** Substituir `panic!` no optimizer por `Result::Err`
  - 11+ pattern matches em `rules.rs` com `panic!` no branch default
  - Migrar para `Result<LogicalPlan, OptimizerError>`
  - Arquivo: `crates/fundb-optimizer/src/rules.rs`

- [ ] **HARDEN-002:** Auditoria de `.unwrap()` — reduzir de 337 para <50
  - Priorizar: paths de execução do servidor (handler, executor, protocol)
  - Aceitar `.unwrap()` em testes e código de inicialização
  - Substituir por `?` operator ou `.unwrap_or_else()`

- [ ] **HARDEN-003:** Implementar property-based testing
  - Parser: qualquer string gera `Ok(Statement)` ou `Err(ParseError)` (nunca panic)
  - Codec: `decode(encode(record)) == record` para qualquer FunRecord
  - Storage: `get(key) == last_insert(key)` após N operações aleatórias

- [ ] **HARDEN-004:** Wiring REST API com query engine real
  - `POST /query` deve executar queries reais (atualmente retorna mock data)
  - `POST /understand` deve usar SemanticEngine
  - `POST /causal/trace` deve usar CausalEngine

- [ ] **HARDEN-005:** Testes de Raft / network partition
  - Simulação de split-brain
  - Leader crash e re-eleição
  - Log replication com followers lentos
  - Snapshot installation com state incompleto

### P4 — Infraestrutura de QA (longo prazo)

- [ ] **INFRA-001:** Adicionar medição de cobertura de código
  - Ferramenta: `cargo llvm-cov` ou `cargo tarpaulin`
  - Target: >80% line coverage
  - Integrar no CI como check (fail se < threshold)

- [ ] **INFRA-002:** Integrar benchmarks no CI
  - Rodar `cargo bench` no CI
  - Comparar com baseline (criterion gera reports)
  - Alertar se regressão de performance >10%

- [ ] **INFRA-003:** Adicionar pre-commit hooks
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - Rejeitar commits com formatação ou warnings

- [ ] **INFRA-004:** Criar test fixtures reutilizáveis
  - Builder pattern para cenários comuns (N records, collection com dados, etc.)
  - Helpers para spin up de servidor em teste
  - Datasets de teste padronizados

- [ ] **INFRA-005:** Testes de SDK
  - Python: `pip install fundb && pytest`
  - Node.js: `npm test`
  - Go: `go test ./...`
  - Cada SDK deve testar: connect, insert, select, error handling

---

## Métricas de Qualidade — Baseline

| Métrica | Atual | Target |
|---------|-------|--------|
| Total de testes | 462 | >700 |
| Testes falhando | 0 | 0 |
| Testes flaky | 0 | 0 |
| Clippy errors | 12 | 0 |
| Compiler warnings | 14 | 0 |
| `.unwrap()` em prod code | ~337 | <50 |
| `panic!()` em prod code | 11+ | 0 |
| Code coverage | desconhecido | >80% |
| Crates com 0 testes | 1 (cli) | 0 |
| Property tests | 0 | >20 |
| Benchmark regressions | N/A | 0 |
