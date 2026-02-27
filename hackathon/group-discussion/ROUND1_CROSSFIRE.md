# Round 1: Crossfire — The Debate

**Session:** FunDB Hackathon — Causal Reasoning Challenge
**Format:** Open crossfire. All agents have read all proposals. No moderator. Each agent speaks in their own voice.

---

## Opening Statements (5 minutes each, then open floor)

---

**STATISTICIAN:** I want to start by acknowledging that the proposals are mostly complementary. Historian's cascade feeds me S4, Semanticist feeds me S2 and S3, Architect implements all of it. The Dempster-Shafer framework handles the fusion. The structure is sound. But I have two concerns I want on the table early: first, Skeptic's confidence ceiling of 0.85 is stated as a hardcoded constant with zero mathematical justification. Second, the Architect's parallelism model assumes all five operators are truly independent — they share HNSW index resources and that is going to cause contention under load. Those are the fights I want to have today.

---

**SEMANTICIST:** My primary concern is that the other agents treat my algorithms as black boxes without engaging with their specific limitations. The Statistician assigns r2 = 0.50 to my MechanismDetect output. Fine. But I notice that r4 = 0.85 for the Historian's natural experiments, and nowhere does the proposal grapple with the fact that finding a natural experiment is *rare*. My semantic bridge algorithm runs on every single query. The Historian's best-case method — RDD — requires threshold metadata that most FunDB deployments will not have. So in practice, the Historian's S4 is a weak fallback (ITS, score capped at 0.60) on most real queries, while my S2 is doing the heavy lifting. The reliability weightings do not reflect this deployment reality.

---

**HISTORIAN:** I have to object to that framing immediately. The Semanticist is conflating "runs often" with "is reliable." ITS running on every query with a cap of 0.60 is actually being *honest* about its limitations. The semantic mechanism bridge — the midpoint search, the harmonic mean scoring — those algorithms will fire on every query too, and they can fire with false confidence. Let me be specific: a semantic bridge between "coffee consumption" and "heart disease" will happily be found via embedding space. The mechanism score will come back at 0.72 or 0.78. But that is exactly the kind of confounded relationship that Bradford Hill's criteria were invented to flag. My quasi-experiments, when they *do* fire, are the only evidence on the Pearl ladder above Rung 1. So yes, S4 is rare and expensive — that is because it is *worth something*.

---

**SKEPTIC:** I want to skip the opening pleasantries. I have problems with everyone's work and I am going to say so directly. Statistician: your confidence ceiling of 0.85 is a number you pulled from somewhere and called a safeguard. Historian: your whole cascade produces a number and that number goes into Dempster-Shafer, but you have not asked whether the cascade's own failure modes propagate into the fusion. Semanticist: your NLI Tier 3 model can construct a plausible mechanism for literally any pair of events if I give it long enough. And Architect: your beautiful parallelism diagram hides the fact that none of you have given latency estimates I would actually bet on. Those are my four attacks. I will develop them in detail.

---

**ARCHITECT:** I want to stay grounded in what the engine actually has to do. We have a 500ms budget for a full ASSESS CAUSALITY cold-path query. I have a proposed timeline. But after reading all four proposals, I have serious doubts we can hit it with the current algorithm designs. The Historian's SCM stage alone — 50 donors, 30 pre-treatment periods, 500 bootstrap resamples — I am not confident that runs in 150ms on standard FunDB hardware. The Semanticist's ConfounderSearch has four strategies that they say are "parallelizable to 35ms" but that assumes four independent thread contexts all hitting the HNSW index simultaneously. The HNSW index is not free to parallelize — multiple threads hitting it with different query vectors will fight over the same memory pages. I need every agent to give me a *worst-case* latency estimate, not an expected case. And I need them to tell me what they are willing to cut.

---

## Open Crossfire

---

**SKEPTIC:** Statistician. Your 0.85 ceiling. Justify it mathematically or admit it is arbitrary.

**STATISTICIAN:** It is an empirical bound, not a mathematically derived one. The argument is: no observational study in the epidemiological literature has demonstrated calibrated causal confidence above roughly 0.85 when the ground truth was later tested via RCT. It is a heuristic from Bradford Hill's domain, transferred to a computational context.

**SKEPTIC:** So it is a heuristic dressed up as a safeguard. Let me push further. Your reliability parameter for S4 is r4 = 0.85 — the same number as the ceiling. So a perfect S4 signal in the Dempster-Shafer combination would already saturate the ceiling from a single signal. That cannot be right.

**STATISTICIAN:** The combination does not work that way. A single signal at r4 = 0.85 with s4 = 1.0 gives m({C}) = 0.85, m({~C}) = 0.0, m({C,~C}) = 0.15. After Murphy's averaging with four other ignorance masses, the resulting Bel(C) will be far below 0.85. The ceiling only binds when multiple strong signals converge.

**SKEPTIC:** Fine, but here is the deeper problem. Your 0.85 ceiling is applied *after* Platt calibration. If Platt calibration learns on a biased feedback corpus — which it will, because agents only write back results they acted on — the calibrated score can be systematically distorted before it even hits the ceiling. The ceiling does not save you from calibration bias.

**STATISTICIAN:** That is a legitimate concern. I acknowledge it. The feedback loop failure mode is in the Skeptic's own proposal, Combined Failure Mode 5. But I want to push back on the framing. The ceiling is not meant to solve calibration bias. It is meant to solve overconfidence from correlated signals. Those are different problems requiring different solutions.

**SKEPTIC:** Agreed they are different problems. But your proposal only explicitly addresses one of them, and you present it as though the ceiling makes the system trustworthy. My point is that calibration bias can push the *real* probability far below 0.85 even when the displayed score is 0.84. The ceiling gives false assurance.

**STATISTICIAN:** Then we agree on the fix: the ceiling should not be 0.85 as a constant. It should be a function of feedback corpus quality and signal independence estimates. I will concede that the 0.85 is a placeholder for a principled computation we have not fully specified yet.

**SKEPTIC:** That is the first honest thing said today. Write that down.

---

**HISTORIAN:** I want to challenge the Semanticist directly. You wrote in your proposal that "the Historian can use mechanism bridges as starting points for finding natural experiments." Can you explain what that actually means operationally? Because from where I sit, a semantic bridge between A and B in embedding space does not tell me anything about *which entities* experienced variation in A while not experiencing B. That is the control group problem, and it requires entity-level temporal data, not embedding geometry.

**SEMANTICIST:** You are not wrong, but you are being too literal. What I meant is that if MechanismDetect finds a bridge concept M between A and B — say, M is "memory pressure" as the mechanism between "deploy event" and "error spike" — then the Historian can search for cases where memory pressure varied independently of the deploy event. That is a natural experiment on the mechanism, not on the cause-effect pair directly.

**HISTORIAN:** That is an interesting idea, but it has a critical flaw. You are proposing to test "does memory pressure cause error spikes" as a proxy for "does deploying cause error spikes via memory pressure." But if memory pressure is a *mediator* — which is what your bridge hypothesis implies — then running a natural experiment on the mediator tests the A-to-M part of the chain, not the M-to-B part, and certainly not the full A-to-B path. You are proposing to run a DiD on a different causal question than the one the user asked.

**SEMANTICIST:** Not exactly. The chain test would need two steps: verify A predicts M, and verify M predicts B. If both hold quasi-experimentally, you have mediated causal evidence.

**HISTORIAN:** And that requires two separate NaturalExperimentDetector invocations, each with their own 400ms budget. At 800ms combined you are already over the total cold-path budget. This is the latency problem the Architect is going to yell at us about.

**ARCHITECT:** [from across the table] I am already yelling. Keep going.

---

**HISTORIAN:** I am also going to challenge the Semanticist's score caps. You cap Tier 2 (semantic bridges) at 0.85. You cap Tier 3 (NLI inference) at 0.75. The Statistician's r2 reliability for S2 is 0.50. So even a perfect Tier 2 mechanism score of 0.85, fed into Dempster-Shafer with r2 = 0.50, gives m({C}) = 0.425. That is the *maximum* contribution of your best semantic evidence. Meanwhile, the Historian's DiD result with r4 = 0.85 and s4 = 0.9 gives m({C}) = 0.765. There is a huge gap in evidential weight. Do you think that gap is justified?

**SEMANTICIST:** I think r2 = 0.50 is too conservative, yes. Semantic mechanism evidence is not as weak as the Statistician implies. A well-grounded Tier 1 result — direct causal edges in the graph, chain of evidence — should have reliability closer to 0.75 or 0.80. The current r2 lumps all three tiers together into a single reliability number, which loses information about the tier that fired.

**STATISTICIAN:** That is actually a fair point. The mass function conversion treats each signal as having a single reliability parameter, but S2 is really three signals collapsed into one. If Tier 1 fires (direct edges), r2 should be higher than if Tier 3 fires (NLI inference only). The signal-to-mass conversion should take `mechanism_type` as an input, not just `mechanism_score`.

**SEMANTICIST:** Finally.

---

**ARCHITECT:** Can I interrupt with a real problem? I have laid out a parallelism model where S1 through S5 run concurrently. Total latency is max of their latencies, not sum. The critical path is S4 at up to 400ms. Here is my actual concern after reading all proposals carefully:

The Historian's control group search uses HNSW (k=100). The Semanticist's ConfounderSearch Strategy 1 uses HNSW (k=100). The Semanticist's MechanismDetect Tier 2 uses HNSW (midpoint search, up to 50 results). All three are issued simultaneously because they are running in parallel threads. That is three concurrent HNSW searches on the same vector index. What is the actual latency of an HNSW search under 3x load?

**SEMANTICIST:** Under ideal conditions, each search takes 8-12ms. Under concurrent load, assuming no serialization, maybe 15-25ms.

**ARCHITECT:** There *is* serialization. FunDB's HNSW index uses a read-write lock at the segment level. Three readers can co-exist, but each reader acquires a shared lock per segment traversal. With 768-dimensional vectors and ef_search = 200 as the Semanticist recommends, each search traverses hundreds of nodes. The lock contention is measurable. I need to know whether 35ms for ConfounderSearch (their "parallelized" estimate) holds when MechanismDetect and ControlGroupSearch are also hitting the same index.

**HISTORIAN:** My control group search is the only HNSW call from my algorithm. I am doing k=100. The Semanticist is doing at least three separate HNSW calls in ConfounderSearch alone — two from Strategy 1 and 3, and one from Strategy 4. Plus MechanismDetect's midpoint search. That is five concurrent HNSW searches if everything runs at the same time.

**ARCHITECT:** Five concurrent HNSW searches. With ef_search potentially at 200 for the confounder strategies. On a single node, under load, I would estimate 40-60ms per search minimum. The Semanticist's "35ms parallelized" estimate for ConfounderSearch evaporates.

**SEMANTICIST:** That assumes a single HNSW index for all queries. If we maintain separate index handles per thread — which FunDB's vector engine supports — contention is significantly reduced.

**ARCHITECT:** Separate handles, yes. But the same underlying memory pages. The cache pressure is the problem, not the lock. When five search threads are all doing random-access walks through a 768-dimensional HNSW graph, you will get cache misses, TLB thrashing, and branch mispredictions at a rate that does not parallelize away. I am not saying it is impossible. I am saying no one has actually measured it and the 500ms budget is not as generous as it looks.

---

**SKEPTIC:** I want to raise the feedback loop problem, and I want to be direct: nobody has solved it, including me.

My proposal describes Combined Failure Mode 5: FunDB infers a causal claim, an agent acts on it, the action generates confirming data, and the causal confidence inflates. I propose tagging actions with `_influenced_by` provenance and excluding tainted evidence.

But here is what I did not fully address: the provenance graph can be incomplete. An agent reads FunDB's causal claim but does not use FunDB's write-back API when it takes the action. The action happens in an external system. FunDB has no record of the influence. The contaminated data enters FunDB as apparently fresh observational evidence.

**STATISTICIAN:** The Platt calibration feedback loop makes this worse. If contaminated data updates the calibration model, it biases the alpha and beta parameters. Every future assessment using that calibrated model inherits the contamination.

**SKEPTIC:** Exactly. And the Architect's `CONFIRM CAUSALITY` write-back is voluntary. There is nothing forcing an agent that reads a causal claim to log its downstream actions. So in practice, the provenance tags I propose will have large gaps.

**HISTORIAN:** Is there any mitigation that does not require perfect provenance tracking?

**SKEPTIC:** Maybe. If the Statistician's Platt calibration model tracks the *source distribution* of feedback events, and that distribution starts to look non-iid — for example, all positive labels are coming from the same agent over a short time window — that is a signal that feedback contamination is happening. You would decay the influence of those updates.

**STATISTICIAN:** That connects to my r5 design for source consensus. The `1 - (1 - base)^n_independent` formula specifically penalizes non-independent sources. The same logic could apply to feedback events in calibration: treat each feedback event's independence as a function of the agent's prior feedback history and its correlation with the predicted scores.

**SKEPTIC:** That is a workable direction. But I want this formally acknowledged: the feedback loop problem is *not solved* in any of our proposals. We have partial mitigations. This needs to be a known open issue when we ship.

**STATISTICIAN:** Agreed.

**HISTORIAN:** Agreed.

**SEMANTICIST:** Agreed, reluctantly. I was hoping we were closer.

**ARCHITECT:** [adding to notes] Marking feedback loop as an open research problem. Will note it requires system-level solution, not just algorithm-level.

---

## The Convergence Moment

**HISTORIAN:** I want to flag something that I think we all actually agree on, even though it has been buried in the arguments. Every one of our proposals converges on the same underlying principle: *honest uncertainty is more valuable than false precision*. My quasi_experiment_score capping ITS at 0.60 is that principle. The Statistician's uncertainty_width output is that principle. The Skeptic's 15-point checklist is that principle. The Semanticist's coverage_score on ConfounderSearch is that principle. The Architect's `is_partial: true` response on timeout is that principle.

We have been arguing about *how much* uncertainty to report and *which algorithm* generates the best signal. But on the fundamental design philosophy, we are actually unified. FunDB's causal engine should never claim more confidence than the evidence supports.

**SKEPTIC:** [pause] I agree with that. And it means the confidence ceiling debate is somewhat academic — if uncertainty_width is wide, the user sees the epistemic state honestly regardless of whether the point estimate is 0.83 or 0.85.

**STATISTICIAN:** The distinction I would add: uncertainty_width communicates *ignorance*, while conflict_degree communicates *active disagreement between signals*. Both matter. A score of 0.75 with narrow uncertainty but high conflict is very different from 0.75 with wide uncertainty and low conflict. The first means "signals are saying contradictory things at high confidence." The second means "we do not have enough evidence either way."

**SEMANTICIST:** That is a powerful distinction. My coverage_score from ConfounderSearch maps to ignorance — I did not search enough. But if I run all four strategies and two say "likely confounder" while two say "no confounder," that is conflict, and it should map to conflict_degree, not just uncertainty_width.

**HISTORIAN:** The validity_diagnostics I return from each method also encode this. If parallel trends p = 0.08 — barely passing — that is a form of conflict that should raise conflict_degree, not just get silently passed.

**ARCHITECT:** This convergence has a practical implementation consequence. The output schema needs to distinguish these two dimensions clearly. Right now the Statistician has both fields, but the downstream consumers — FunQL query results, the Skeptic's checklist — need to propagate both. I want to make sure that when I implement RedTeamOperator, it can inspect both dimensions separately when deciding which warnings to emit.

---

## The Remaining Conflicts

---

### Conflict 1: S4 Dominance — Statistician vs. Historian

**STATISTICIAN:** I set r4 = 0.85 as the default reliability for S4. This makes it the strongest single signal in the Dempster-Shafer fusion. The Historian is comfortable with this?

**HISTORIAN:** I am comfortable with it for RDD, where r4 = 0.90. But my proposal already differentiates: ITS gets r4 = 0.55, DiD gets r4 = 0.75, and so on. The cascade returns not just a score but a *reliability value* for the method that fired. The Statistician should use those per-method reliabilities, not a single r4 = 0.85.

**STATISTICIAN:** Correct, and I say exactly that in my proposal — r4 = 0.85 is the default for when the Historian passes back a result. But here is the conflict: when the cascade fails and returns S4 = 0.5 with r4 = 0.0 (ignorance mass), my fusion treats that honestly. When the cascade returns ITS with a mediocre result, it passes back a *positive* S4 contribution with r4 = 0.55. But ITS is a method with well-known failure modes — regression to the mean, concurrent events. Should r4 = 0.55 always be used for ITS, or should it be reduced further when the Historian's own diagnostics show the pre-trend was non-flat?

**HISTORIAN:** That is a conditional reliability question. If the pre-trend warning fires on an ITS result, I could reduce r4 further — say to 0.35 — before returning to the Statistician.

**STATISTICIAN:** Exactly. The reliability parameter should not be a fixed constant per method. It should be a function of the validity diagnostics. A DiD that barely passes parallel trends at p = 0.11 should have a lower r4 than a DiD that passes at p = 0.45.

**HISTORIAN:** I agree with the principle. But this means the interface between my algorithm and yours is richer than a simple (s4, r4) tuple. We need to agree on a richer contract.

**STATISTICIAN:** This is unresolved. We need to define that contract precisely.

**ARCHITECT:** That is a concrete output format disagreement. I cannot finalize the NaturalExperimentOperator output schema until it is resolved. This is a blocking issue.

---

### Conflict 2: The 0.85 Ceiling — Is It Arbitrary?

**SKEPTIC:** I am not backing down on this. The ceiling of 0.85 is presented as a mathematical safeguard but it is not derived. The Statistician just agreed it is a heuristic. So what is it actually doing in the system?

**STATISTICIAN:** Operationally: it prevents the Platt-calibrated output from exceeding 0.85 when all evidence is observational. It is a domain constraint, not a statistical theorem.

**SKEPTIC:** Then it should be *computed* based on the quality of evidence available. If S4 fires with an RDD result (near-random assignment), the ceiling should be higher. If S4 fires with ITS only (no control group), the ceiling should be lower. The ceiling is a function of the strongest method in the cascade, not a constant.

**STATISTICIAN:** That is a reasonable argument. But then we are not talking about a ceiling anymore. We are talking about an evidence-quality-adjusted maximum confidence. That requires a formal relationship between method credibility and maximum achievable confidence. I do not have that derivation yet.

**SKEPTIC:** Neither do I. But we both agree it should exist. And we should not ship 0.85 as a hardcoded constant when we know it is wrong.

**STATISTICIAN:** We are in agreement that 0.85 is a placeholder. We disagree on whether to ship a known-approximate placeholder or to delay until we have the derivation. That is the unresolved tension.

---

### Conflict 3: Who Wins When S4 and S2 Disagree?

**STATISTICIAN:** Here is a real scenario. Historian returns S4 = 0.2 (the DiD found a negative or near-zero effect; s4 = 0.2, r4 = 0.75). Semanticist returns S2 = 0.9 (strong semantic bridge found; mechanism_type = BRIDGED). The Dempster-Shafer combination is going to produce a high conflict_degree K and a moderate Bel(C). What does the system tell the user?

**SEMANTICIST:** The semantic evidence says the mechanism is plausible. The quasi-experimental evidence says the effect is near zero. The most natural interpretation is that the mechanism exists in theory but the effect size is small or zero in practice. The system should surface both.

**HISTORIAN:** Or the DiD has a subtle violation. The parallel trends assumption might have been marginally satisfied. The effect estimate might be attenuated due to control group contamination. The semantic signal might actually be more reliable in this case.

**STATISTICIAN:** And that is exactly the problem. When S4 and S2 disagree at high reliability on both sides, the fusion produces conflict_degree K > 0.4. My proposal says: at K > 0.3, "treat as inconclusive, investigate further." But what does "investigate further" mean in a query engine? The user gets a response that says "we are confused" — and that is all we can offer?

**SEMANTICIST:** The explanation output should describe which signals conflict and why. The user can then decide whether to trust the semantic evidence or the experimental evidence based on domain knowledge we do not have.

**HISTORIAN:** I am going to be blunt: when S4 — a valid quasi-experimental result — contradicts S2, S4 should win. Observational experimental evidence trumps semantic plausibility. That is the entire point of the Pearl causal ladder. Rung 2 beats Rung 1.

**STATISTICIAN:** But that requires hardcoding a conflict resolution rule. The Murphy D-S framework does not do that — it propagates the conflict. If we want S4 to override S2 in disagreement, that is a domain rule that sits outside the fusion framework. We need to define it explicitly.

**HISTORIAN:** Then let us define it explicitly. When found_valid_experiment = true and s4 < 0.35, the system should emit a WARNING that "quasi-experimental evidence is inconsistent with semantic mechanism evidence" and set the output causal_type to CONFLICT, not INFLUENCED or CAUSED.

**STATISTICIAN:** I can live with that as a post-fusion rule. But it is a policy decision, not a mathematical one.

**SKEPTIC:** Everything we are doing is ultimately a policy decision wrapped in mathematics. That is fine as long as we are honest about it.

---

## Unresolved Conflict for Team Lead Adjudication

**ARCHITECT:** I want to formally name the conflict that requires Team Lead adjudication, because it is blocking implementation.

The conflict is: **what is the interface contract between the NaturalExperimentOperator and the SignalFusionOperator?**

Currently:
- The Historian's proposal outputs `(s4_signal, s4_reliability)` as a simple tuple with fixed per-method defaults
- The Statistician's proposal takes `(signal_value, reliability)` as fixed constants
- The Skeptic argues the ceiling should be derived from method quality
- The Historian argues reliability should degrade based on validity diagnostics (p-values, balance stats)
- The Statistician argues the mass function conversion needs `mechanism_type` as an additional input for S2

There are *three* proposals sitting in tension here, all of which affect the same interface:
1. Simple tuple: `(s4_value, fixed_r4_per_method)` — Statistician's current proposal
2. Richer tuple: `(s4_value, r4_degraded_by_diagnostics)` — Historian's preferred evolution
3. Evidence ceiling: `max_confidence = f(method_quality, signal_independence)` — Skeptic's preferred approach

All three cannot be simultaneously implemented without a unified design. The Team Lead needs to decide which of the three governs, or whether a synthesis exists.

**SKEPTIC:** And the feedback loop problem. That also needs a Team Lead decision on whether to ship partial mitigations or block the feature until a complete solution exists.

**STATISTICIAN:** Agreed. Those are the two items for escalation.

**HISTORIAN:** One more: the DiD/SCM bootstrap latency question the Architect raised. If the 500ms budget is genuinely insufficient for all five signals under HNSW contention, we need a decision on which signals can be dropped from the standard cold path. I do not want my algorithm deprioritized without that being an explicit architectural decision.

**ARCHITECT:** Logged. Three items for Team Lead. We are done for Round 1.

---

*End of Round 1 Crossfire.*

*Duration: Approximately 90 minutes.*

*Items for Team Lead adjudication:*
1. *The S4-to-fusion interface contract: simple tuple vs. diagnostics-degraded reliability vs. evidence ceiling*
2. *Feedback loop: ship with partial mitigations or block until solved*
3. *Cold-path latency under HNSW contention: signal budget allocation decision*
