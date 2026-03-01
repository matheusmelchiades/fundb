# FunDB — Plano de Produto para Lançamento Público

**Versão:** 1.0
**Data:** 2026-03-01
**Autor:** Product Owner
**Status:** Draft — aguardando aprovação

---

## 1. Visão do Produto

### O Problema

O ecossistema de IA atual opera com infraestrutura de dados fragmentada:

| Necessidade | Solução Atual | Dor |
|-------------|---------------|-----|
| Vetores / embeddings | Pinecone, Weaviate, Qdrant | Banco separado, sem relação com dados estruturados |
| Grafos de conhecimento | Neo4j, Neptune | Outro banco, outro modelo, outra query language |
| Documentos / JSON | MongoDB, PostgreSQL JSONB | Sem confiança nativa, sem proveniência |
| Time-series / logs | InfluxDB, TimescaleDB | Sem causalidade, sem versionamento bitemporal |
| Memória de agentes | Redis, hacks ad-hoc | Sem recall semântico, sem decay, sem feedback loop |

**Resultado:** engenheiros de IA gastam 40-60% do tempo em data plumbing — integrando 3-5 bancos, mantendo consistência entre eles, e perdendo metadados cognitivos no caminho.

### A Tese do FunDB

> **Um único banco que pensa como IA** — vetores, grafos, documentos, time-series e memória de agentes unificados sob um motor cognitivo que rastreia confiança, proveniência, contexto e causalidade como cidadãos de primeira classe.

### Proposta de Valor (Elevator Pitch)

> *"FunDB é o banco de dados cognitivo para a era da IA. Ao invés de integrar 5 bancos diferentes, conecte seus agentes a um único motor que entende semântica, rastreia confiança, otimiza para context windows e raciocina sobre causalidade. É PostgreSQL wire-compatible — seus tools existentes já funcionam."*

---

## 2. Público-Alvo (por prioridade)

### P0 — Early Adopters (Lançamento)

| Persona | Descrição | Job-to-be-done |
|---------|-----------|----------------|
| **AI Engineer** | Constrói pipelines RAG, agentes autônomos, chatbots | Precisa de retrieval com confiança + contexto otimizado para LLM |
| **ML Platform Engineer** | Opera infraestrutura de dados para times de ML | Quer reduzir complexidade operacional (1 banco vs 5) |
| **AI Startup Founder** | Time pequeno, precisa mover rápido | Quer uma stack de dados completa sem DevOps overhead |

### P1 — Growth Phase

| Persona | Descrição | Job-to-be-done |
|---------|-----------|----------------|
| **Data Scientist** | Análise causal, experimentação | Precisa de causal reasoning nativo sem sair do banco |
| **Backend Engineer** | Adiciona IA a aplicações existentes | Quer PG-compatible mas com superpoderes cognitivos |

### P2 — Escala

| Persona | Descrição |
|---------|-----------|
| **Enterprise Architect** | Precisa de multi-tenancy, RBAC, audit logging |
| **DevOps / SRE** | Opera cluster em produção, precisa de observability |

---

## 3. Estado Atual — Audit de Product Readiness

### O que está pronto (Engineering Complete)

| Área | Status | Detalhe |
|------|--------|---------|
| Data Model (FunRecord) | **Pronto** | Documento + vetor + grafo + time-series + causal + cognitive metadata |
| Storage Engine (LSM) | **Pronto** | MemTable + WAL + SSTable + Compaction + MVCC bitemporal |
| Indexes | **Pronto** | B+Tree, HNSW, SPO Graph, Temporal, Causal DAG |
| Query Engine (FunQL) | **Pronto** | Parser + Binder + Optimizer (rule + cost) + Executor (Volcano) |
| Confidence & Provenance | **Pronto** | Propagation algebra, contradiction detection, auto-update |
| Context-Aware Retrieval | **Pronto** | WITHIN CONTEXT com MMR, token budget, coherence |
| Causal Reasoning (Tier 1-3) | **Pronto** | Explicit → Granger → SCM/do-calculus/counterfactuals |
| Semantic Interface (Tier 1-2) | **Pronto** | Rule-based + ML classifier |
| Agent Memory | **Pronto** | REMEMBER, RECALL BY, weighted scoring |
| Learning-to-Rank | **Pronto** | LambdaMART, feedback loop |
| Distribution (Raft + Sharding) | **Pronto** | Leader election, consistent hash, distributed queries |
| PostgreSQL Wire Protocol | **Pronto** | `psql` e `pgAdmin` funcionam |
| REST API | **Pronto** | /health, /query, /understand, /causal_trace |
| SDKs | **Pronto** | Node.js, Python, Go |
| CLI | **Pronto** | REPL interativo + single-command + JSON/CSV/table output |
| Docker | **Pronto** | Single-node + 3-node cluster via compose profiles |
| Tests | **407/407 (100%)** | Unit + integration + property-based |
| Security | **Pronto** | Multi-tenant RBAC + audit logging |
| Observability | **Pronto** | Prometheus metrics + OpenTelemetry tracing |

### O que falta para lançar (Gaps Críticos)

| # | Gap | Tipo | Impacto | Esforço |
|---|-----|------|---------|---------|
| G1 | **README / Landing page do repo** | Docs | Ninguém entende o produto sem isso | 1-2 dias |
| G2 | **Quickstart guide** (5 min → rodando) | Docs | Barreira #1 de adoção | 2-3 dias |
| G3 | **FunQL Language Reference** completa | Docs | Usuário não sabe a sintaxe | 3-4 dias |
| G4 | **SDK tutorials** com exemplos reais | Docs | SDKs existem mas sem guia | 2-3 dias |
| G5 | **Benchmarks publicáveis** | Eng | Sem números, sem credibilidade | 3-5 dias |
| G6 | **End-to-end integration tests** reais (não mock) | QA | 407 unit tests, mas precisa de smoke tests e2e | 3-4 dias |
| G7 | **Error messages amigáveis** | UX | Erros crípticos afastam early adopters | 2-3 dias |
| G8 | **Website / docs site** (mdbook ou docusaurus) | Docs | Precisa de um lugar bonito para documentação | 3-5 dias |
| G9 | **Cookbooks** (RAG pipeline, Agent memory, Causal analysis) | Docs | Mostram o "porquê" do produto | 4-5 dias |
| G10 | **CHANGELOG + Release versioning** | Proc | Sem versionamento formal, parece alpha | 1 dia |
| G11 | **Crates.io / npm / PyPI publish** | Dist | SDKs precisam ser instaláveis | 2-3 dias |
| G12 | **Docker Hub image pública** | Dist | `docker pull fundb` precisa funcionar | 1 dia |
| G13 | **CI/CD pipeline** (GitHub Actions) | Eng | Sem CI, PRs quebram sem ninguém saber | 2-3 dias |
| G14 | **Licença open-source** | Legal | Sem licença = ninguém pode usar | 1 hora |
| G15 | **Security audit básico** | Sec | SQL injection, auth bypass, etc. | 2-3 dias |

---

## 4. Estratégia de Go-to-Market

### Posicionamento

```
Categoria:  AI-Native Database
Tagline:    "The database that thinks like AI"
Alternativa a: Pinecone + Neo4j + MongoDB + InfluxDB + Redis (5 bancos → 1)
Diferencial: Cognitive metadata (confiança, causalidade, contexto) como first-class citizens
Compatível com: PostgreSQL wire protocol (ferramentas existentes funcionam)
```

### Modelo de Licenciamento (Recomendado)

| Tier | Licença | Inclui |
|------|---------|--------|
| **FunDB Community** | AGPL-3.0 ou BSL (Business Source License) | Tudo que existe hoje. Single-node e cluster. |
| **FunDB Cloud** (futuro) | SaaS | Managed service, auto-scaling, backups, SLA |
| **FunDB Enterprise** (futuro) | Commercial | SSO/SAML, advanced audit, dedicated support, SLA |

**Recomendação:** Lançar como **Apache-2.0** ou **AGPL-3.0** para maximizar adoção. BSL se quiser proteger contra cloud providers.

### Canais de Distribuição

| Canal | Ação | Prioridade |
|-------|------|------------|
| **GitHub** | Repo público, README matador, GitHub Releases | P0 |
| **Docker Hub** | `docker pull fundb/fundb:latest` | P0 |
| **Crates.io** | `cargo install fundb-cli` | P1 |
| **PyPI** | `pip install fundb` | P0 |
| **npm** | `npm install @fundb/client` | P0 |
| **Hacker News** | Launch post: "Show HN: FunDB — The cognitive database for AI" | P0 |
| **Reddit** | r/rust, r/MachineLearning, r/LocalLLaMA | P0 |
| **Twitter/X** | Thread técnica demonstrando os diferenciais | P1 |
| **Dev.to / Medium** | Artigo "Why we built a database that reasons about causality" | P1 |
| **Discord** | Comunidade para early adopters | P0 |

---

## 5. Roadmap de Lançamento

### Fase 1 — "Ready to Ship" (Semanas 1-3)

> **Objetivo:** Repo público com experiência de primeira execução impecável.

| Semana | Entregável | Owner |
|--------|------------|-------|
| S1 | G14: Definir licença (Apache-2.0 recomendado) | PO |
| S1 | G1: README completo (visão, quickstart, features, arquitetura visual) | PO + Eng |
| S1 | G10: CHANGELOG.md + semantic versioning (v0.1.0) | Eng |
| S1 | G13: CI/CD — GitHub Actions (test + build + lint) | Eng |
| S1 | G12: Publicar imagem Docker Hub | Eng |
| S2 | G2: Quickstart guide — 3 caminhos: Docker / cargo / psql | Eng |
| S2 | G3: FunQL Language Reference (todas as cláusulas com exemplos) | Eng |
| S2 | G7: Audit de error messages — todas user-facing legíveis | Eng |
| S2 | G11: Publish SDKs (npm + PyPI) | Eng |
| S3 | G6: Smoke tests end-to-end (Docker → connect → query → result) | QA |
| S3 | G5: Benchmark suite publicável (vector search, point read, write throughput) | Eng |
| S3 | G15: Security audit básico (OWASP top 10 para databases) | Sec |

### Fase 2 — "Developer Experience" (Semanas 4-6)

> **Objetivo:** Conteúdo que convence e retém early adopters.

| Semana | Entregável | Owner |
|--------|------------|-------|
| S4 | G8: Docs site (mdbook ou docusaurus) hospedado em GitHub Pages | Eng |
| S4 | G4: SDK tutorials — Python (RAG pipeline), Node.js (agent memory), Go (causal analysis) | Eng |
| S5 | G9: Cookbook #1 — RAG Pipeline com FunDB (end-to-end com LangChain/LlamaIndex) | Eng |
| S5 | G9: Cookbook #2 — Agent Memory persistente | Eng |
| S6 | G9: Cookbook #3 — Causal Analysis para dados de negócio | Eng |
| S6 | Grafana dashboard templates para operadores | Eng |

### Fase 3 — "Public Launch" (Semana 7)

> **Objetivo:** Barulho coordenado.

| Dia | Ação |
|-----|------|
| D-7 | Soft launch: compartilhar com 20-30 AI engineers para feedback |
| D-3 | Fixar bugs críticos do soft launch |
| D-1 | Preparar posts (HN, Reddit, Twitter) |
| **D-Day** | GitHub public + HN "Show HN" + Reddit posts + Twitter thread |
| D+1 | Discord community ativo para suporte |
| D+7 | Publicar primeiro blog post ("Why we built FunDB") |
| D+14 | Publicar benchmark comparison vs Pinecone + pgvector |

### Fase 4 — "Growth" (Mês 2-3)

| Entregável | Descrição |
|------------|-----------|
| Kubernetes Helm chart | Deploy em cluster k8s com 1 comando |
| FunDB Playground | Sandbox web para testar queries sem instalar |
| LangChain integration | `FunDBVectorStore` nativo no LangChain |
| LlamaIndex integration | `FunDBReader` + `FunDBRetriever` |
| CrewAI / AutoGen integration | Memory provider nativo |
| Terraform provider | Infra-as-code para FunDB Cloud (futuro) |
| Embedded ONNX (Semantic Tier 3) | NLU nativa sem dependência de API externa |

---

## 6. Métricas de Sucesso (KPIs)

### Lançamento (30 dias)

| Métrica | Target | Justificativa |
|---------|--------|---------------|
| GitHub stars | 1.000+ | Validação de interesse |
| Docker pulls | 500+ | Gente testando de verdade |
| Discord members | 200+ | Comunidade nascente |
| SDK installs (pip + npm) | 300+ | Desenvolvedores integrando |
| Issues abertos | 50+ | Engajamento real (bugs = gente usando) |
| Contributors externos | 5+ | Começo de comunidade |

### Growth (90 dias)

| Métrica | Target |
|---------|--------|
| GitHub stars | 5.000+ |
| Monthly active Docker pulls | 2.000+ |
| Production deployments reportados | 10+ |
| Blog posts / tutorials da comunidade | 5+ |
| Conference talks aceitos | 2+ |

---

## 7. Riscos e Mitigações

| # | Risco | Probabilidade | Impacto | Mitigação |
|---|-------|---------------|---------|-----------|
| R1 | **"Too complex"** — produto faz muita coisa, confunde usuário | Alta | Alto | README com 3 use cases claros. Quickstart focado em 1 feature por vez |
| R2 | **Performance insuficiente** vs bancos especializados (Pinecone para vetores) | Média | Alto | Publicar benchmarks honestos. Posicionar como "bom o suficiente + unificado" |
| R3 | **Sem comunidade** — lança e ninguém aparece | Média | Alto | Soft launch com 20-30 people. Conteúdo educacional (causal reasoning é nicho atraente) |
| R4 | **Bugs em produção** — 407 unit tests mas pouco e2e | Média | Alto | Fase 1 inclui smoke tests e2e + security audit |
| R5 | **Rust = barreira de contribuição** | Média | Médio | SDKs em Python/Node.js/Go. Contribuições em docs/cookbooks são acessíveis |
| R6 | **Competição** — pgvector, Weaviate, etc. evoluem rápido | Baixa | Médio | Causal reasoning + confidence são diferenciais únicos. Ninguém mais faz isso |
| R7 | **Licença errada** — afasta empresas ou permite clone por cloud provider | Média | Alto | Decisão de licença na Semana 1. Consultar comunidade open-source |

---

## 8. Competitive Positioning

```
                    Multimodal Data ──────────────────────►
                    (vetores + grafos + docs + TS)
                    │
                    │    ┌─────────────────────────────────┐
                    │    │                                 │
          Cognitive │    │           FunDB                 │
          Features  │    │   (confiança, causalidade,      │
                    │    │    contexto, semântica, LTR)    │
                    │    │                                 │
                    │    └─────────────────────────────────┘
                    │
                    │    ┌──────────┐
                    │    │ SurrealDB│   ┌──────────┐
                    │    │(multimod)│   │ TigerGraph│
                    │    └──────────┘   │(graph+vec)│
                    │                   └──────────┘
                    │
                    │  ┌────────┐ ┌────────┐ ┌────────┐
                    │  │Pinecone│ │Weaviate│ │ Qdrant │
                    │  │(vector)│ │(vec+BM)│ │(vector)│
                    │  └────────┘ └────────┘ └────────┘
                    │
                    │  ┌────────┐ ┌────────┐ ┌──────────┐
                    │  │ Neo4j  │ │Postgres│ │InfluxDB  │
                    │  │(graph) │ │(+pgvec)│ │(TS only) │
                    │  └────────┘ └────────┘ └──────────┘
                    │
                    └───────────────────────────────────────►
```

**Ninguém no mercado combina todos estes eixos:**
1. Multi-modal (vetor + grafo + doc + TS) em um único motor
2. Confidence & provenance nativos
3. Causal reasoning (Granger → SCM → counterfactuals)
4. Context-aware retrieval (otimizado para LLM token budgets)
5. Agent memory com decay e recall semântico
6. PostgreSQL wire-compatible

---

## 9. Decisões Pendentes (Precisam de input do Founder)

| # | Decisão | Opções | Recomendação | Urgência |
|---|---------|--------|--------------|----------|
| D1 | **Licença** | Apache-2.0 / AGPL-3.0 / BSL / MIT | Apache-2.0 (max adoção) ou BSL (proteção contra cloud) | **Semana 1** |
| D2 | **Nome final** | FunDB (atual) / outro | FunDB é memorável e curto. Verificar trademark | Semana 1 |
| D3 | **Domínio + website** | fundb.io / fundb.dev / outro | fundb.dev (developer-focused) | Semana 1 |
| D4 | **Organização GitHub** | Pessoal / org `fundb-io` | Criar org `fundb-io` para credibilidade | Semana 1 |
| D5 | **Cloud offering (futuro)** | Self-hosted only / managed cloud | Começar self-hosted, planejar cloud para Mês 4+ | Pode esperar |
| D6 | **Monetização** | Open-core / SaaS / support contracts | Open-core (Community free + Enterprise paid) | Pode esperar |

---

## 10. Definição de "Done" para Lançamento

O FunDB está pronto para lançamento público quando **TODOS** estes critérios forem atendidos:

- [ ] **Licença** definida e arquivo LICENSE no repo
- [ ] **README** completo com: visão, quickstart (< 5 min), features, arquitetura, benchmarks
- [ ] **Quickstart** funciona em 3 caminhos: `docker run`, `cargo install`, `psql` connect
- [ ] **FunQL Reference** publicado (toda a sintaxe documentada com exemplos)
- [ ] **SDKs publicados** no npm e PyPI com READMEs
- [ ] **Docker image** pública no Docker Hub
- [ ] **CI/CD** rodando (tests + build em todo PR)
- [ ] **Benchmarks** publicáveis com números reais
- [ ] **Security audit** básico concluído (sem vulnerabilidades P0)
- [ ] **Smoke tests e2e** passando (Docker → connect → CRUD → cognitive queries)
- [ ] **Error messages** auditadas (todas user-facing são compreensíveis)
- [ ] **CHANGELOG** atualizado
- [ ] **Discord/community** channel criado
- [ ] **1 Cookbook** publicado (RAG pipeline recomendado)
- [ ] **Soft launch** com 20+ AI engineers, feedback incorporado

---

## 11. Resumo Executivo

**FunDB é um produto tecnicamente completo** — o motor está construído, os 407 testes passam, as features cognitivas que nenhum competidor oferece estão implementadas. O que separa o FunDB de um "projeto no GitHub" de um "produto que pessoas usam" são **duas coisas:**

1. **Developer Experience** — README, quickstart, docs, cookbooks, error messages
2. **Distribuição** — Docker Hub, npm, PyPI, CI/CD, comunidade

O roadmap acima resolve ambos em **~7 semanas**, com a Fase 1 (3 semanas) sendo a mais crítica. Após isso, o produto está no mundo e a prioridade muda para **community building e integrations**.

> **A engenharia está 95% feita. O produto está 60% feito. O gap é 100% solucionável.**

---

*Documento gerado em 2026-03-01. Próxima revisão: após decisões D1-D6.*
