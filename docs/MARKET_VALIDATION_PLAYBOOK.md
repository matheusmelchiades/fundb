# FunDB — Market Validation Playbook

**Versao:** 1.0
**Data:** 2026-03-01
**Status:** Ativo — executar ANTES do lançamento público
**Owner:** A ser definido
**Prazo sugerido:** 3-4 semanas

---

## 0. Contexto e Motivacao

O FunDB é tecnicamente completo (16 crates, 407 testes, motor cognitivo funcionando). Mas **completude técnica não é validação de mercado.** Este documento existe porque precisamos responder uma pergunta antes de investir em go-to-market:

> **Estamos resolvendo uma dor real — algo caro, frágil ou difícil em produção — ou estamos criando mais uma alternativa técnica sem problema forte por trás?**

O [PRODUCT_LAUNCH_PLAN.md](PRODUCT_LAUNCH_PLAN.md) assume que a resposta é sim. Este playbook **testa essa premissa** antes de executar o plano de lançamento.

---

## 1. Hipoteses a Validar

Cada hipótese abaixo precisa ser **confirmada ou invalidada** com evidência concreta (conversas, dados, comportamento). Não basta intuição.

### H1 — A dor multi-banco é real e cara

**Hipotese:** Times de AI em produção operam 3-5 bancos de dados (vetor + documento + grafo + cache + time-series), e a complexidade de manter consistência entre eles gera custo operacional significativo (>20% do tempo de engenharia).

**Como validar:**
- Entrevistar 10+ ML Platform Engineers / AI Engineers em empresas com AI em produção
- Perguntas-chave:
  - "Quantos bancos de dados seu pipeline de AI usa hoje?"
  - "Quanto tempo do seu time é gasto em data plumbing entre eles?"
  - "Já tiveram incidentes por inconsistência entre vector store e document store?"
  - "Quanto pagam mensalmente por esses serviços combinados?"
- **Sinal positivo:** Entrevistados descrevem o problema espontaneamente, sem que você precise explicá-lo
- **Sinal negativo:** "Usamos Postgres pra tudo e tá ok" ou "Não é o nosso maior problema"

**Resultado esperado:** [ ] Confirmada / [ ] Parcialmente confirmada / [ ] Invalidada

---

### H2 — RAG em produção é fragil e ninguem resolve bem

**Hipotese:** Times que operam RAG em produção enfrentam problemas recorrentes de relevância, confiança nos resultados e desperdício de context window — e não existe solução nativa que resolva isso.

**Como validar:**
- Buscar em comunidades (MLOps Slack, r/MachineLearning, r/LocalLLaMA, Discord de LangChain/LlamaIndex) posts sobre:
  - "RAG quality in production"
  - "context window optimization"
  - "confidence scoring for retrieval"
  - "RAG hallucination reduction"
- Postar provocações diretas (sem mencionar FunDB):
  - "How do you handle confidence scoring in your RAG pipeline?"
  - "What's your strategy for optimizing context window usage in production RAG?"
  - "Has anyone solved the stale embedding vs. updated document problem?"
- **Sinal positivo:** Threads com alto engajamento, respostas detalhando soluções improvisadas, frustração visível
- **Sinal negativo:** Pouca resposta, ou "rerankers resolvem isso" como consenso

**Resultado esperado:** [ ] Confirmada / [ ] Parcialmente confirmada / [ ] Invalidada

---

### H3 — pgvector é percebido como insuficiente

**Hipotese:** Times que usam PostgreSQL + pgvector reconhecem que é uma solução improvisada e gostariam de algo melhor — mas não mudam porque o custo de troca é alto demais para soluções atuais.

**Como validar:**
- Entrevistar 5+ times que usam pgvector em produção
- Perguntas-chave:
  - "Por que escolheram pgvector ao invés de Pinecone/Weaviate?"
  - "O que não funciona bem?"
  - "Se existisse algo com as mesmas queries SQL mas com [feature X], vocês migrariam?"
  - "Qual o maior problema que enfrentam com a solução atual?"
- **Sinal positivo:** "Escolhemos por familiaridade, mas estamos batendo em limites de [X]"
- **Sinal negativo:** "pgvector resolve tudo que precisamos, não temos reclamação"

**Resultado esperado:** [ ] Confirmada / [ ] Parcialmente confirmada / [ ] Invalidada

---

### H4 — Agent memory é um problema emergente sem solução

**Hipotese:** Times construindo agentes autônomos (com frameworks como CrewAI, AutoGen, LangGraph) precisam de memória persistente e pesquisável, e as soluções atuais (Redis, SQLite, hacks ad-hoc) são inadequadas.

**Como validar:**
- Buscar issues/discussões em repos de frameworks de agentes:
  - LangChain memory modules
  - CrewAI memory
  - AutoGen conversations
  - LangGraph checkpointing
- Entrevistar 5+ devs que constroem agentes autônomos
- Perguntas-chave:
  - "Como seu agente lembra de interações passadas?"
  - "Qual a solução de memória que você usa? Está satisfeito?"
  - "O que acontece quando o agente precisa de contexto de 30 dias atrás?"
- **Sinal positivo:** "Estamos usando Redis com hacks terríveis" ou "Não temos boa solução"
- **Sinal negativo:** "LangChain Memory resolve" ou "Não precisamos de memória persistente"

**Resultado esperado:** [ ] Confirmada / [ ] Parcialmente confirmada / [ ] Invalidada

---

### H5 — Causal reasoning e confidence scoring têm demanda real

**Hipotese:** Existe um segmento (regulados, fintech, health) onde proveniência de dados, confidence scoring e rastreabilidade causal são **requisitos de compliance**, não nice-to-haves.

**Como validar:**
- Pesquisar regulações:
  - EU AI Act — requisitos de explicabilidade e auditoria
  - FDA guidelines para AI em healthcare
  - Regulações financeiras (SR 11-7, SS1/23) sobre model risk management
- Entrevistar 3+ times em empresas reguladas que usam AI
- Perguntas-chave:
  - "Como vocês auditam as decisões do modelo?"
  - "Conseguem responder 'por que o modelo recomendou X' para um regulador?"
  - "Quanto tempo/custo gastam em compliance de AI?"
- **Sinal positivo:** "Gastamos meses construindo audit trails ad-hoc" ou "Regulação X exige isso e não temos solução boa"
- **Sinal negativo:** "Compliance de AI não é prioridade ainda" ou "Usamos MLflow e resolve"

**Resultado esperado:** [ ] Confirmada / [ ] Parcialmente confirmada / [ ] Invalidada

---

## 2. Segmentos-Alvo para Investigacao

Priorizar a validação nestes perfis, ordenados por probabilidade de dor intensa:

| Prioridade | Segmento | Por que investigar | Onde encontrar |
|------------|----------|-------------------|----------------|
| **P0** | ML Platform Engineers em empresas com RAG em produção (>10M docs) | Maior dor operacional multi-banco, problemas de consistência visíveis | MLOps Community Slack, Hacker News, LinkedIn |
| **P0** | Devs construindo agentes autônomos em produção | Agent memory é problema emergente sem solução, FunDB tem capability única | Discord de LangChain, CrewAI, AutoGen; r/LocalLLaMA |
| **P1** | AI teams em empresas reguladas (fintech, health, govtech) | Compliance força necessidade de proveniência e auditoria | LinkedIn, conferências de AI in Finance/Healthcare |
| **P1** | Times usando pgvector que bateram em limites | Candidatos naturais para migração dado PG wire compatibility | PostgreSQL community, Stack Overflow, r/PostgreSQL |
| **P2** | AI startups early-stage (<10 engenheiros) | Querem stack simples, não querem operar 5 bancos | Y Combinator network, Indie Hackers, Twitter/X AI community |

---

## 3. Plano de Execucao

### Semana 1 — Pesquisa Secundaria (Desk Research)

**Objetivo:** Mapear o cenário antes de falar com pessoas.

| # | Acao | Entregavel | Tempo |
|---|------|------------|-------|
| 1.1 | Levantar discussões sobre dor de RAG em produção nas comunidades listadas | Documento com 20+ links de threads relevantes, categorizados por tipo de dor | 2 dias |
| 1.2 | Analisar issues abertas em LangChain, LlamaIndex, CrewAI relacionadas a memory/retrieval/consistency | Lista de 10+ issues com volume de engagement | 1 dia |
| 1.3 | Pesquisar regulações de AI (EU AI Act, FDA, regulações financeiras) e extrair requisitos de auditoria/explicabilidade | Resumo de 1-2 páginas com requisitos mapeáveis para capabilities do FunDB | 1 dia |
| 1.4 | Levantar alternativas diretas e como se posicionam (Pinecone, Weaviate, Qdrant, pgvector, SurrealDB, TigerGraph) | Tabela comparativa: features, pricing, limitações declaradas por usuários | 1 dia |

---

### Semana 2 — Entrevistas de Descoberta (Customer Discovery)

**Objetivo:** Falar com 10-15 pessoas e ouvir, não vender.

| # | Acao | Entregavel | Tempo |
|---|------|------------|-------|
| 2.1 | Recrutar 10-15 entrevistados nos segmentos P0/P1 via comunidades, LinkedIn, rede pessoal | Lista de entrevistados confirmados com data/hora | 2 dias |
| 2.2 | Conduzir entrevistas de 20-30 min usando o roteiro da Seção 4 | Notas estruturadas por entrevista (template na Seção 5) | 3-4 dias |
| 2.3 | Postar 3-5 provocações em comunidades (sem mencionar FunDB) para medir engajamento | Links dos posts + métricas de engajamento (upvotes, replies, qualidade das respostas) | Em paralelo com 2.2 |

**Regras para entrevistas:**
- **NAO mencione o FunDB.** Fale sobre problemas, não sobre soluções.
- **NAO pergunte "você usaria X?"** — isso gera falsos positivos. Pergunte sobre comportamento passado: "como você resolve isso hoje?"
- **Busque dor, não interesse.** "Isso parece legal" não é validação. "Gastei 3 semanas construindo isso na mão" é.

---

### Semana 3 — Sintese e Decisao

**Objetivo:** Consolidar achados e tomar uma decisão informada.

| # | Acao | Entregavel | Tempo |
|---|------|------------|-------|
| 3.1 | Consolidar notas de entrevistas em padrões (dores recorrentes, quotes fortes, objeções, surpresas) | Documento de síntese com seções por hipótese | 1 dia |
| 3.2 | Preencher scorecard de validação (Seção 6) com base em evidências | Scorecard preenchido com decisão Go/No-Go/Pivot por hipótese | 1 dia |
| 3.3 | Definir wedge strategy: qual capability lançar primeiro com base nos achados | Recomendação documentada: "Lançar FunDB posicionado como [X] para [Y]" | 1 dia |
| 3.4 | Atualizar o PRODUCT_LAUNCH_PLAN.md com base nos achados (ou pivotar) | PRODUCT_LAUNCH_PLAN.md revisado ou documento de pivot | 1 dia |

---

### Semana 4 (Opcional) — Teste de Interesse Concreto

**Objetivo:** Validar se interesse declarado se converte em ação.

| # | Acao | Entregavel | Tempo |
|---|------|------------|-------|
| 4.1 | Criar landing page simples: "FunDB — [posicionamento definido na S3]" com waitlist | URL da landing page + analytics configurados | 1 dia |
| 4.2 | Compartilhar com os entrevistados que demonstraram maior dor + nas comunidades | Número de sign-ups na waitlist | 3-5 dias de coleta |
| 4.3 | Oferecer acesso alpha para os 5 mais engajados | Número de pessoas que de fato rodaram `docker compose up` e executaram queries | 3-5 dias |

**Meta:** 5 pessoas que rodam o FunDB e dão feedback técnico real. Se não conseguir 5, a dor não é forte o suficiente para justificar lançamento — ou o posicionamento está errado.

---

## 4. Roteiro de Entrevista

**Duracao:** 20-30 minutos
**Formato:** Conversa aberta, NÃO questionário

### Abertura (2 min)

> "Estou pesquisando como times de AI lidam com infraestrutura de dados em produção. Não estou vendendo nada — quero entender como vocês trabalham hoje. Posso gravar/anotar?"

### Bloco 1 — Contexto (5 min)

- O que vocês estão construindo com AI?
- Está em produção ou ainda em desenvolvimento?
- Qual o tamanho do time trabalhando nisso?

### Bloco 2 — Stack de Dados (5 min)

- Quais bancos de dados vocês usam no pipeline de AI?
- Como decidiram usar esses bancos? (escolha ativa vs. herança)
- Quanto tempo do time é gasto mantendo essa infra de dados?

### Bloco 3 — Dores (10 min)

- Qual o maior problema de infra de dados que vocês enfrentam hoje?
- Já tiveram incidentes em produção causados por problemas de dados? (exemplos concretos)
- Como vocês garantem que os dados que o modelo usa estão corretos e atualizados?
- Se vocês pudessem mudar UMA coisa na infra de dados, o que seria?

### Bloco 4 — Retrieval / RAG (5 min) — se aplicavel

- Como funciona o pipeline de retrieval de vocês?
- Como medem a qualidade dos resultados?
- O que fazem quando o modelo recebe contexto irrelevante ou desatualizado?
- Já tentaram otimizar o uso de context window? Como?

### Bloco 5 — Agentes (5 min) — se aplicavel

- Vocês constroem agentes autônomos? Com qual framework?
- Como o agente lembra de interações passadas?
- O que acontece quando o agente precisa de contexto de semanas atrás?

### Encerramento (2 min)

> "Muito obrigado. Posso voltar a falar com você quando tivermos algo para mostrar?"

**IMPORTANTE:** Anotar não apenas o que dizem, mas **como** dizem. Frustração, suspiros, "ah, isso é um pesadelo" são sinais mais fortes que respostas racionais.

---

## 5. Template de Notas de Entrevista

```
## Entrevista #[N]
**Data:** YYYY-MM-DD
**Nome:** [nome ou anonimizado]
**Cargo:** [cargo]
**Empresa:** [empresa ou segmento]
**AI em producao?** Sim / Nao / Parcialmente

### Stack de dados atual
- Bancos usados: [lista]
- Dor principal declarada: [texto]
- Tempo gasto em data plumbing: [estimativa]

### Dores identificadas
1. [dor] — Intensidade: Alta / Media / Baixa
2. [dor] — Intensidade: Alta / Media / Baixa

### Quotes relevantes
> "[quote exata]"

### Relacao com hipoteses
- H1 (multi-banco): Confirma / Neutro / Contradiz — [porque]
- H2 (RAG fragil): Confirma / Neutro / Contradiz — [porque]
- H3 (pgvector insuficiente): Confirma / Neutro / Contradiz — [porque]
- H4 (agent memory): Confirma / Neutro / Contradiz — [porque]
- H5 (compliance/causal): Confirma / Neutro / Contradiz — [porque]

### Sinal de compra
- Descreve o problema espontaneamente? Sim / Nao
- Ja tentou resolver? Sim (como: [X]) / Nao
- Pagaria por solucao? Sim / Talvez / Nao
- Testaria um alpha? Sim / Talvez / Nao

### Surpresas ou insights nao esperados
- [insight]
```

---

## 6. Scorecard de Validacao

> **Status:** Parcialmente preenchido — Semana 1 (Desk Research) concluída em 2026-03-01.
> Faltam as entrevistas (Semana 2) para veredicto final. Os achados abaixo são preliminares, baseados exclusivamente em pesquisa secundária.

### Por Hipotese

| Hipotese | Evidencias a Favor | Evidencias Contra | Forca do Sinal (1-5) | Veredicto Preliminar |
|----------|-------------------|-------------------|-----------------------|-----------|
| H1 — Dor multi-banco | Tiger Data quantifica "7 DBs = 7 coisas que quebram às 3AM." Reliability math: 3 sistemas a 99.9% = 99.7% = 26h downtime/ano. SurrealDB levantou $23M validando essa tese. VentureBeat cobriu explicitamente o problema. Cloudflare descreve RAG como "patchwork of moving parts." | Movimento "just use Postgres" forte e ganhando tração. Managed services (Pinecone, Weaviate Cloud) abstraem complexidade. Times sofisticados resolveram com bom DevOps. Composable stack é padrão aceito. | **3.5** | [x] Weak — Dor real mas sendo endereçada por consolidação PostgreSQL e managed services. Janela pode estar se fechando. |
| H2 — RAG frágil | **Sinal mais forte.** DeepMind LIMIT study prova ceiling matemático em single-vector embeddings (<20% recall em queries complexas). HNSW degrada silenciosamente com escala. 70% dos sistemas RAG sem framework de avaliação. Embedding staleness custa $12K/mês para re-indexar 1TB. 30+ threads documentados com frustração forte. Compounding failure: 0.95³ = 0.86 (falha 1 em 7). | Hybrid search (BM25+vector) melhora qualidade significativamente. Reranking é "highest value 5 lines of code." RAG funciona bem para bases estáticas e escopo limitado. Long context windows (1M+ tokens) podem reduzir relevância do RAG. Tooling de avaliação melhorando (60% dos novos deploys incluem evals vs 30% em 2025). | **4** | [x] Go — Evidência muito forte e consistente. Problema é real, fundamental (não apenas engenharia), e workarounds são paliativos. |
| H3 — pgvector insuficiente | Performance cai em 10M+ vetores. Sem GPU acceleration. Index build lento e memory-intensive. Query planner não otimizado para vetores. Dimensão limitada perto de 2000 dims. pgvectorscale não disponível no RDS. | pgvectorscale atinge 471 QPS a 99% recall em 50M vetores. Movimento "just use Postgres" ganhando momentum. Para <10M vetores (maioria dos use cases), pgvector é competitivo. Simplicidade operacional supera diferença de performance para maioria. | **2.5** | [x] Weak — pgvector é suficiente para 60-70% dos use cases. FunDB deve mirar nos 30-40% que precisam de mais (grafo, causal, confidence), não competir em vector search puro. |
| H4 — Agent memory | LangChain **depreciou sistema inteiro** de memória em v0.3.x (admitiu falha arquitetural). AutoGen Memory Proposal aberto há 15+ meses sem resolução. 26 issues documentadas com padrões claros: defaults não-production-ready, serialização silenciosamente corrompe dados, token management não resolvido. Startups dedicadas (Mem0, Zep, Letta) emergiram. Workshop ICLR 2026 sobre agent memory. CrewAI memória hardcoded em OpenAI + SQLite, inutilizável em produção customizada. | LangGraph checkpointing funciona (com bugs). Integrações Mem0/Zep existem como workarounds. Muitas issues são de experimentadores, não produção. Baixo engagement individual (0-4 reactions) sugere dor fragmentada. O problema pode ser inerentemente difícil demais para resolver no nível do banco. | **3.5** | [x] Weak-to-Go — Dor genuína e validada estruturalmente (deprecação de sistemas, startups emergindo), mas mercado ainda emergente com poucos agentes em produção real. Risco: memory layers (Mem0, Zep) podem vencer vs. database-level solution. |
| H5 — Compliance/causal | EU AI Act (Aug 2026): multas até 35M EUR / 7% faturamento global. Artigos 10, 12, 19 exigem proveniência, logging automático, retenção. FDA PCCP exige rollback e data management. SR 11-7 backtesting mapeia diretamente para bitemporal MVCC. NIST AI 600-1 recomenda data provenance para GenAI. | Regulações focam em **modelo**, não dados. Toolchains existentes (MLflow + Splunk + Credo AI) resolvem "good enough." Training data provenance != inference data provenance. Compradores regulados são ultra-conservadores (SOC2, track record). Startup DB sem credibilidade para regulados. SEC recuou em regras de AI. | **2.5** | [x] Weak — Regulações criam demanda por **capabilities**, mas não por **novo banco de dados**. Melhor como selling point secundário ("compliance-friendly") do que como posicionamento primário. Credibilidade com regulados requer 2-3 anos de track record. |

### Sintese Preliminar (Desk Research Only)

**Hipoteses com sinal forte (Go/Weak-to-Go):** H2 (RAG frágil) e H4 (Agent memory)
**Hipoteses com sinal fraco (Weak):** H1 (Multi-banco), H3 (pgvector), H5 (Compliance)
**Total de sinais fortes:** 1 confirmada (H2), 1 quase-confirmada (H4) = insuficiente para decisão Go final

**Indicacao preliminar: PIVOT**

Com base apenas na desk research, o cenário mais provável é **Pivot** — reposicionar FunDB ao redor das hipóteses H2 e H4, que mostram os sinais mais fortes. Mas essa decisão **não deve ser tomada ainda** — as entrevistas da Semana 2 podem mudar significativamente o quadro, especialmente para H1 e H4.

### Implicacoes para Wedge Strategy (preliminar)

| Opcao | Suporte da Desk Research | Recomendacao |
|-------|--------------------------|-------------|
| A — RAG confiável em produção | **Forte.** H2 é a hipótese mais validada. Context-aware retrieval, confidence scoring e contradiction detection são gaps reais que nenhum concorrente cobre. | **Candidata #1** — Se entrevistas confirmarem, liderar com isso. |
| B — Memória para agentes | **Moderado-forte.** H4 tem evidência estrutural (deprecações, startups). Mas compete com Mem0/Zep e SurrealDB 3.0 já tem agent memory. | **Candidata #2** — Viável se diferenciação (semantic decay, recall ponderado) for percebida como superior. |
| C — Auditoria para regulados | **Fraco.** H5 mostra que compliance é resolvido por outras ferramentas. FunDB não tem credibilidade para enterprise regulado. | **Não liderar com isso.** Usar como argumento secundário. |
| D — PostgreSQL com superpoderes | **Misto.** pgvector "good enough" para maioria. Mas PG wire protocol é diferencial real de adoção. | **Complementar, não primário.** PG compatibility deve ser feature, não posicionamento. |

### Alertas Criticos da Desk Research

1. **SurrealDB 3.0 é competidor direto.** $23M, enterprise customers (Walmart, Verizon, Samsung), 31K GitHub stars, mesma tese multi-model. FunDB precisa de narrativa clara de "por que nós e não SurrealDB."
2. **Confiança e causal reasoning são genuinamente únicos** — nenhum concorrente oferece. Mas exigem educação de mercado (devs não sabem que precisam disso).
3. **Context-aware retrieval é o feature mais imediatamente útil** — prático, demonstrável, resolve dor real de RAG. Pode ser o hook inicial.
4. **O movimento "just use Postgres" é ameaça e oportunidade.** Ameaça: times preferem ficar no PG. Oportunidade: PG wire protocol do FunDB permite "upgrade" sem trocar tooling.

### Decisao Geral

| Cenario | Criterio | Acao |
|---------|----------|------|
| **Go** | 3+ hipóteses confirmadas com sinal forte (4-5), pelo menos 5 entrevistados descrevem a dor espontaneamente | Executar PRODUCT_LAUNCH_PLAN.md com posicionamento ajustado |
| **Pivot** | 1-2 hipóteses confirmadas, outras fracas ou invalidadas | Reposicionar FunDB ao redor das hipóteses confirmadas. Reescrever posicionamento e voltar a validar |
| **No-Go** | 0-1 hipóteses confirmadas, nenhum sinal forte | Pausar lançamento. Considerar: (a) mudar público-alvo, (b) mudar capabilities priorizadas, (c) contribuir capabilities como extensão de banco existente |

> **Status atual: Indicação preliminar de Pivot (H2+H4).** Decisão final pendente de entrevistas (Semana 2).

---

## 7. Candidatos a Wedge Strategy

Com base na análise prévia, estes são os posicionamentos iniciais mais promissores. A validação deve confirmar qual deles gera mais ressonância.

### Opcao A — "RAG confiavel em producao"

**Posicionamento:** "O único banco de dados com retrieval nativo otimizado para LLMs — com confidence scoring, token budget e contradiction detection."

**Capability principal:** `WITHIN CONTEXT` + `_confidence` + contradiction detection

**Publico:** Times com RAG em produção (>1M docs) enfrentando problemas de qualidade

**Por que pode funcionar:**
- Todo time de RAG enfrenta esse problema
- Ninguém resolve nativamente (todos usam rerankers + heurísticas)
- Demonstrável em 5 minutos

**Risco:** Rerankers (Cohere, Jina) podem ser vistos como "bom o suficiente"

---

### Opcao B — "Memoria persistente para agentes"

**Posicionamento:** "O banco de dados que dá memória real para seus agentes — com recall semântico, decay temporal e aprendizado contínuo."

**Capability principal:** `REMEMBER` / `RECALL BY` + semantic decay + Learning-to-Rank

**Publico:** Devs construindo agentes autônomos com CrewAI, AutoGen, LangGraph

**Por que pode funcionar:**
- Mercado de agentes está explodindo (2025-2026)
- Não existe solução dedicada — todo mundo improvisa com Redis/SQLite
- Integração com frameworks de agentes é natural

**Risco:** Mercado ainda emergente, poucos agentes em produção real

---

### Opcao C — "Auditoria nativa de AI para regulados"

**Posicionamento:** "O banco de dados que responde 'por que o modelo decidiu X' — com proveniência, bitemporalidade e rastreabilidade causal nativos."

**Capability principal:** `_confidence` + `_sources` + bitemporal MVCC + causal reasoning

**Publico:** Times de AI em fintech, healthcare, govtech com requisitos de compliance

**Por que pode funcionar:**
- EU AI Act entra em vigor, exigindo explicabilidade
- Dor é regulatória (não opcional — é obrigatório)
- Willingness to pay é alta em regulados

**Risco:** Ciclo de venda longo em enterprise, FunDB v0.1 pode não ter credibilidade para regulados

---

### Opcao D — "PostgreSQL com superpoderes de AI"

**Posicionamento:** "Tudo que pgvector deveria ser — vector search, graph, confidence, context-awareness — com a mesma interface SQL que você já conhece."

**Capability principal:** PG wire protocol + todas as capabilities cognitivas

**Publico:** Times que usam pgvector e bateram em limites

**Por que pode funcionar:**
- Menor barreira de entrada (PG compatible)
- Posicionamento claro contra alternativa conhecida
- Não exige mudar tooling

**Risco:** Comparação direta com PostgreSQL é perigosa (credibilidade, maturidade, ecossistema)

---

## 8. Criterios de Sucesso deste Playbook

Este playbook é bem-sucedido se, ao final da execução:

1. **Cada hipótese tem um veredicto baseado em evidência** — não em opinião
2. **Existe uma wedge strategy definida** — não "FunDB faz tudo para todos"
3. **Existem 5+ pessoas reais** dispostas a testar o alpha — não followers, não likes, pessoas que rodam `docker compose up`
4. **O PRODUCT_LAUNCH_PLAN.md está atualizado** para refletir os achados — ou existe um documento de pivot

Se nenhuma dessas condições for atingida, o playbook precisa ser re-executado com ajustes no público-alvo ou nas hipóteses.

---

## 9. Recursos e Comunidades para Pesquisa

| Recurso | URL / Acesso | Uso |
|---------|-------------|-----|
| MLOps Community Slack | mlops-community.slack.com | Entrevistas + posts de descoberta |
| r/MachineLearning | reddit.com/r/MachineLearning | Posts de provocação sobre RAG/infra |
| r/LocalLLaMA | reddit.com/r/LocalLLaMA | Comunidade técnica de AI, agentes |
| r/PostgreSQL | reddit.com/r/PostgreSQL | Percepção sobre pgvector |
| LangChain Discord | discord.gg/langchain | Issues de memory/retrieval |
| LlamaIndex Discord | discord.gg/llamaindex | Issues de retrieval/RAG |
| Hacker News | news.ycombinator.com | Pesquisa de threads sobre vector DBs, RAG |
| LinkedIn | linkedin.com | Recrutar entrevistados P1 (regulados, enterprise) |
| GitHub Issues | LangChain, LlamaIndex, CrewAI, AutoGen repos | Evidência de dores com memory/retrieval |

---

*Documento criado em 2026-03-01. Próxima revisão: ao final da Semana 3 de execução.*
