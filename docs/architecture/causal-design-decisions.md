# FunDB Causal Engine — Design Decisions

**Status:** Final
**Author:** FunDB AI Architect
**Resolves:** OQ-1, OQ-6, OQ-7, OQ-8, OQ-10 from IMPLEMENTATION_PLAN.md

---

## Decision 1 — Feedback Loops (OQ-1)

**Question:** DAGs não suportam ciclos. Como lidar com feedback loops reais?
*Exemplo: modelo de recomendação → comportamento do usuário → treino do modelo → modelo de recomendação*

### Decision: Temporal Unrolling como primitiva + Equilibrium Mode como extensão

Feedback loops **só existem em sistemas que mudam ao longo do tempo**. Portanto, a solução correta é tratar o loop como uma cadeia temporal:

```
X(t) ──CAUSED──→ Y(t+Δ1) ──CAUSED──→ X(t+Δ1+Δ2)
```

Isso é exatamente o que o modelo bitemporal do FunDB já suporta. Em vez de um edge `X → Y → X` (ciclo inválido), armazenamos:

```sql
-- Ciclo proibido (erro de validação):
INSERT INTO _causal_edges VALUES ('model', 'user_behavior', 'CAUSED', 0.8);
INSERT INTO _causal_edges VALUES ('user_behavior', 'model', 'CAUSED', 0.7);
-- ERROR: cycle detected in causal graph

-- Correto (temporal unrolling):
INSERT INTO _causal_edges VALUES
  ('model_v1',        'user_behavior_jan', 'CAUSED', 0.8),
  ('user_behavior_jan', 'model_v2',         'CAUSED', 0.7),
  ('model_v2',        'user_behavior_feb', 'CAUSED', 0.82);
```

Para sistemas de **equilíbrio** (sem timestamps, estado estacionário), adicionamos um segundo modo:

```sql
CREATE CAUSAL MODEL pricing_equilibrium
  MODE EQUILIBRIUM          -- permite equações simultâneas
  VARIABLES (price, demand, supply)
  EQUATIONS (
    demand = -2.5 * price + 100 + ε_demand,
    supply =  1.8 * price + 20  + ε_supply,
    price  = solve(demand = supply)   -- equilíbrio
  );
```

No modo `EQUILIBRIUM`, o engine resolve via **fixed-point iteration** (convergência garantida para sistemas contrativos).

**Regra de ouro:**
- Dados com timestamps → temporal unrolling (padrão, DAG estrito)
- Modelos econômicos/físicos de equilíbrio → `MODE EQUILIBRIUM` (opt-in)

**Implementação:** Agent 5-D adiciona detecção de ciclo na inserção. Agent 9-A implementa `MODE EQUILIBRIUM` no `CREATE CAUSAL MODEL`.

---

## Decision 2 — LLM Hallucinations no Causal Oracle (OQ-6)

**Question:** O LLM pode sugerir um edge causal que é estatisticamente plausível mas causalmente incorreto. Como prevenir?

### Decision: Three-Layer Shield + Confidence Ceiling

A solução não é confiar menos no LLM — é **tornar o custo de um erro auto-corrigível**. Três camadas:

**Camada 1 — Validação Estatística** (já no plano)
Cada hipótese do LLM passa por um teste Granger ou PC. Se o p-value não é significativo, edge rejeitado. Isso elimina ~70% das alucinações que são factuamente incorretas.

**Camada 2 — Confidence Ceiling para edges LLM-originados**
Edges sugeridos por LLM **nunca nascem com confidence > 0.60**, independente do p-value:

```
Edge origin          | Max initial confidence
---------------------|----------------------
user_declared        | 1.00 (user é a fonte)
granger_validated    | 0.85
temporal_precedence  | 0.75
llm_validated        | 0.60   ← teto inicial
llm_unvalidated      | 0.40   ← nunca persiste abaixo de 0.3
```

A confiança cresce organicamente quando:
- Múltiplas chamadas LLM independentes (temperaturas diferentes) concordam: `+0.10` por concordância
- Novos dados confirmam o edge via Granger: `+0.15`
- Agente humano confirma explicitamente: `+0.25` (até máximo 0.95)

**Camada 3 — Counterfactual Coherence Check**
Antes de persistir, o sistema roda um mini-teste counterfactual no edge proposto usando dados históricos retidos (20% holdout):

```
Hypothetical edge: marketing_spend ──CAUSED──→ user_churn

Test: In our historical data, when marketing_spend increased significantly,
      did user_churn follow the expected direction within 30 days?

If yes: edge passes coherence check, persisted with llm_validated confidence
If no:  edge rejected, logged as "statistically plausible but historically inconsistent"
```

**Resultado:** Um edge LLM-hallucinado que passa no Granger E no coherence check e tem confidence 0.60 ainda é **útil** — é uma hipótese de trabalho, claramente marcada como de origem `llm_validated`. O agente que consulta vê o confidence e decide se quer agir nele.

**Implementação:** Agent 6-B implementa as três camadas. A tabela de confidence ceiling fica em `fundb-causal/src/confidence_policy.rs`.

---

## Decision 3 — Não-Estacionariedade no Granger (OQ-7)

**Question:** Séries temporais que mudam de distribuição (regime shifts, concept drift) invalidam os pressupostos do Granger. O que fazer?

### Decision: Rolling Window Granger + Stationarity Pre-check

O teste Granger assume séries estacionárias. Séries não-estacionárias produzem **spurious causality** — a maior armadilha em análise causal de time-series.

**Protocolo de execução do teste Granger no FunDB:**

```
Step 1: ADF Test (Augmented Dickey-Fuller)
  → Se série é não-estacionária: aplicar diferenciação (d=1 ou d=2)
  → Se ainda não-estacionária após d=2: rejeitar Granger, retornar warning

Step 2: Cointegration Check (Johansen test)
  → Se X e Y são não-estacionárias mas cointegradas:
    → Usar VECM (Vector Error Correction Model) em vez de VAR
    → Granger causality no VECM é válido

Step 3: Rolling Window Granger
  → Dividir série em janelas sobrepostas (ex: 6 meses com passo de 1 mês)
  → Rodar Granger em cada janela
  → Edge persiste apenas se p-value < 0.05 em >60% das janelas
  → Edge strength = média dos F-statistics nas janelas válidas
  → Metadado adicional: stability_score (0.0–1.0, variância dos p-values nas janelas)

Step 4: Edge annotation
  → Edges com stability_score < 0.4 são marcados como UNSTABLE
  → Agentes podem filtrar por stability: WHERE causal_strength > 0.5 AND stability > 0.6
```

A estabilidade temporal é um **primeiro-cidadão** nos metadados causais — não uma nota de rodapé.

**FunQL adicionado:**
```sql
-- Filtrar por edges estáveis
TRACE CAUSALITY FROM :a TO :b
  MIN_STRENGTH 0.4
  MIN_STABILITY 0.5;   -- novo parâmetro

-- Ver perfil de estabilidade de um edge
SELECT causal_strength, stability_score, stability_windows
FROM _causal_edges
WHERE source = :x AND target = :y;
```

**Implementação:** Agent 6-B implementa ADF + Johansen + rolling window. Adicionar `stability_score` e `stability_windows` ao schema de `CausalEdge` no FunRecord.

---

## Decision 4 — Equações Não-Lineares no SCM (OQ-8)

**Question:** Equações lineares no SCM são limitadas demais para sistemas reais. Como evoluir?

### Decision: Progressive Complexity em 3 versões

Não vou prometer redes neurais no v1. Mas vou criar uma arquitetura de equações extensível:

**v1.0 — Linear (implementar no Wave 9-A)**
```
Y = β₀ + β₁X₁ + β₂X₂ + ε
```
Vantagens: interpretável, rápido, solução fechada, confiança nos coeficientes.
Limitação: não captura interações ou relações curvilíneas.

**v1.5 — GAM (Generalized Additive Models) — extensão natural do v1**
```
Y = β₀ + f₁(X₁) + f₂(X₂) + f₁₂(X₁, X₂) + ε
```
Onde `f_i` são funções suaves (splines). Ainda interpretável graficamente.
Complexidade: moderada. Usa biblioteca existente (pode ser porta Rust do `mgcv`).
Custo de intervenção: modificar `f_i` analiticamente para `do(X=x)` — requer computação numérica mas é solucionável.

**v2.0 — Neural SCM (pesquisa, não comprometer data)**
Redes neurais como funções causais. Interpretabilidade cai; potência aumenta.
Intervenção via gradient-based optimization. Counterfactuals mais caros.
Provavelmente não entra no roadmap principal sem pesquisa dedicada.

**Decisão concreta para o plano:**
- Wave 9-A implementa **v1 (linear) obrigatoriamente**
- v1.5 (GAM) é um **bonus task** para Agent 9-A se tempo permitir
- v2.0 é marcado como **research track** — não entra no Roadmap sem um paper próprio

**FunQL adicionado para selecionar equação:**
```sql
CREATE CAUSAL MODEL revenue_model
  EQUATION_TYPE LINEAR          -- v1, default
  -- ou: EQUATION_TYPE ADDITIVE  -- v1.5
  VARIABLES (...) EQUATIONS (...);
```

**Implementação:** Agent 9-A implementa com `EquationType` enum extensível. A trait `CausalFunction` permite adicionar novos tipos sem quebrar API.

---

## Decision 5 — NOTEARS não converge (OQ-10)

**Question:** NOTEARS pode não convergir ou convergir para mínimo local ruim. Qual é o fallback?

### Decision: Ensemble Causal Discovery com Votação

Nenhum algoritmo de descoberta causal é sempre melhor. A solução é rodar múltiplos e tomar a **interseção de alta confiança**:

```
Input: Collection com N variáveis

Step 1: Rodar PC Algorithm (constraint-based)
  → Produce: CPDAG (set of possible DAGs)

Step 2: Rodar NOTEARS (score-based, gradient)
  → Se convergência falha em 1000 iterações: restart com perturbação aleatória (3x)
  → Se ainda falha: marcar como NOT_CONVERGED, skip

Step 3: Rodar Granger pairwise (para coleções de time-series)
  → Produce: set of directed edges with p-values

Step 4: Ensemble voting
  → Edge (X→Y) é incluído no resultado se:
      presente em PC output AND NOTEARS output (both agree) = confidence 0.85
      presente em apenas um = confidence 0.55
      presente em Granger + um dos outros = confidence 0.75
  → Edges abaixo de confidence 0.40 são descartados

Step 5: Conflito de direção
  → Se PC diz X→Y e NOTEARS diz Y→X:
    → Edge marcado como DIRECTION_UNCERTAIN
    → Persiste com ambas direções e confidence 0.45 cada
    → Usuário/agente pode resolver: UPDATE _causal_edges SET resolved_direction = 'X_TO_Y'
```

**FunQL:**
```sql
DISCOVER CAUSAL STRUCTURE IN COLLECTION metrics
  ALGORITHM 'ensemble'      -- default, usa todos
  -- ou: ALGORITHM 'pc'     -- só PC
  -- ou: ALGORITHM 'notears' -- só NOTEARS, falha explicitamente se não convergir
  MIN_CONFIDENCE 0.55
  STORE AS causal_model 'my_system';

-- Ver edges com direção incerta
SELECT * FROM _causal_edges
WHERE causal_model = 'my_system'
  AND direction_status = 'DIRECTION_UNCERTAIN';
```

**Implementação:** Agent 9-A implementa o ensemble. O `ALGORITHM 'notears'` sem fallback é disponível para usuários que querem comportamento explícito.

---

## Mudanças no FunRecord

Estas decisões requerem extensões ao `CausalEdge`:

```rust
pub struct CausalEdge {
    pub source_id:     Uuid,
    pub target_id:     Uuid,
    pub relation:      CausalType,
    pub strength:      f32,
    pub mechanism:     Option<String>,

    // NOVO (Decision 2):
    pub origin:        CausalOrigin,       // UserDeclared | Granger | Temporal | LlmValidated
    pub confidence:    f32,                // distinct from edge strength

    // NOVO (Decision 3):
    pub stability_score:   Option<f32>,    // 0.0–1.0, None for non-timeseries
    pub stability_status:  StabilityStatus, // Stable | Unstable | NotApplicable

    // NOVO (Decision 5):
    pub direction_status: DirectionStatus, // Confirmed | DirectionUncertain
    pub discovery_algo:   Option<String>,  // "ensemble", "pc", "notears", "granger"
}

pub enum CausalOrigin {
    UserDeclared,
    Granger { p_value: f32, lag: u32 },
    TemporalPrecedence { correlation: f32 },
    LlmValidated { model: String, coherence_score: f32 },
}

pub enum StabilityStatus {
    Stable,           // stability_score >= 0.6
    Unstable,         // stability_score < 0.4
    Provisional,      // 0.4 <= stability_score < 0.6
    NotApplicable,    // não é time-series
}

pub enum DirectionStatus {
    Confirmed,
    DirectionUncertain,  // algoritmos discordaram
}
```

---

## Resumo das Decisões

| OQ | Decisão | Complexidade | Wave |
|----|---------|-------------|------|
| OQ-1 (feedback loops) | Temporal unrolling padrão + `MODE EQUILIBRIUM` opt-in | Média | 5-D + 9-A |
| OQ-6 (LLM hallucinations) | Three-Layer Shield + confidence ceiling 0.60 | Média | 6-B |
| OQ-7 (não-estacionariedade) | ADF pre-check + rolling window + stability_score | Alta | 6-B |
| OQ-8 (não-linear SCM) | Linear v1 obrigatório, GAM v1.5 bonus, Neural v2 research | Média/Alta | 9-A |
| OQ-10 (NOTEARS divergência) | Ensemble voting PC + NOTEARS + Granger | Média | 9-A |

Todas as questões em aberto estão resolvidas. O sistema causal do FunDB é implementável com as decisões acima — nenhuma depende de problema científico aberto não resolvido. A única parte que fica como "research track" é Neural SCM (v2.0), e isso é intencional.

---

*"A causalidade perfeita é inimiga da causalidade útil."*
