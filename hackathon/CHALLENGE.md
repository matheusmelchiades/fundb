# FunDB Hackathon: Solving Causal Reasoning for AI-Native Databases

**Date:** 2026-02-28
**Challenge Owner:** FunDB Core Team
**Time Constraint:** Single session sprint

---

## The Problem

No database in the world can answer: **WHY did something happen?**

FunDB has all the ingredients (vectors, graphs, time-series, confidence, provenance) in one engine. The challenge is: **how do we combine multiple imperfect signals of causality into a unified assessment that is mathematically sound, computationally viable, and useful without requiring a PhD?**

## The 4 Sub-Problems

### SP1 — Signal Fusion: How to weigh causal signals?
If semantic mechanism says "yes" but natural experiment says "maybe not", who wins? We need a framework for combining evidence from temporal patterns, semantic understanding, confounder exclusion, natural experiments, and source consensus.

### SP2 — Automatic Confounder Search
The most critical signal is negative — proving no alternative explanation exists. How do we systematically search for confounders within the database?

### SP3 — Natural Experiment Detection
Given a (cause, effect) pair, how do we automatically find historical situations in the database that function as quasi-experiments?

### SP4 — Query Ergonomics
How do we express all of this in a query that a developer (or AI agent) can actually use without being a statistician?

## Constraints

- Must be computationally viable (query-time, not hours of processing)
- Must work with data already in FunDB (vectors, graphs, time-series, confidence, provenance)
- Must be honest about uncertainty (never claim certainty it doesn't have)
- Must be useful for AI agents as primary consumers
- Must not require the user to pre-specify a causal graph (that's the whole point)

## Agents

| Agent | Expertise | Primary Focus |
|-------|-----------|---------------|
| **Statistician** | Statistical causal inference (Pearl, Rubin, Granger) | SP1: Signal Fusion |
| **Semanticist** | NLP, embeddings, semantic understanding | SP2: Mechanism + Confounder detection via semantics |
| **Historian** | Historical data analysis, quasi-experiments | SP3: Natural Experiment Detection |
| **Skeptic** | Adversarial thinking, finding flaws, edge cases | All: Stress-test every proposal |
| **Architect** | Database internals, query engines, systems design | SP4: Make it queryable and efficient |
| **Team Lead** | Cross-domain synthesis, evaluation | Final: Evaluate and synthesize all proposals |

## Deliverables Per Agent

Each agent must produce in their folder:
1. `RESEARCH.md` — What they researched, key findings, references
2. `PROPOSAL.md` — Their concrete solution to their assigned sub-problem(s)
3. `EXAMPLES.md` — Concrete examples showing their solution in action

## Evaluation Criteria

The Team Lead will evaluate proposals on:
1. **Correctness** — Is it mathematically/logically sound?
2. **Feasibility** — Can it run at query-time in a database engine?
3. **Ergonomics** — Can a developer use it without being a domain expert?
4. **Honesty** — Does it properly communicate uncertainty?
5. **Integration** — Does it work with the other agents' proposals?
