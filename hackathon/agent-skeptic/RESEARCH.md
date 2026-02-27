# Agent Skeptic -- Research: Failure Modes in Causal Reasoning

**Role:** Adversarial Thinker / Red Team
**Mission:** Catalog every known way causal inference can fail, with emphasis on failure modes relevant to an AI-native database that fuses multiple imperfect signals.

---

## 1. Foundational Philosophical Limitations

### 1.1 Hume's Problem of Induction

David Hume demonstrated in 1739 that causation is never directly observed -- we only observe constant conjunction. No amount of "A before B" observations logically entails "A causes B." This is not a historical curiosity; it is an unsolved problem in epistemology that places a hard ceiling on any automated causal system.

**Implication for FunDB:** The system can never achieve certainty about causal claims from observational data alone. Any confidence score of 1.0 on an inferred causal edge is philosophically dishonest.

### 1.2 Underdetermination of Theory by Data

For any finite dataset, there exist infinitely many causal structures that are consistent with the observed data (Quine, 1951). This means FunDB's confounder search can never be "complete" -- there is always an unobserved variable that could explain the pattern.

**Implication for FunDB:** The system must never claim "no confounders exist." It can only claim "no confounders were found among the variables we examined."

### 1.3 The Problem of Background Assumptions

All causal inference methods require untestable assumptions. Pearl's do-calculus requires faithfulness, the causal Markov condition, and correct graph specification. Rubin's potential outcomes framework requires SUTVA (Stable Unit Treatment Value Assumption) and ignorability. Granger causality requires stationarity.

**Implication for FunDB:** Every causal claim should disclose which assumptions were relied upon, because the user (or agent) has no way to verify them from the data alone.

---

## 2. Catalog of Statistical Fallacies

### 2.1 Simpson's Paradox

A trend that appears in several groups of data reverses when the groups are combined. Classic example: UC Berkeley gender bias case (1973) -- overall admissions appeared to favor men, but department-level analysis showed a slight favor toward women. Women applied disproportionately to more competitive departments.

**Relevance:** FunDB's co-occurrence and correlation signals will produce opposite conclusions depending on which level of aggregation is queried. A query that does not control for the grouping variable will get the wrong answer.

### 2.2 Berkson's Bias (Collider Bias)

When you condition on a common effect of two variables, it creates a spurious association between them. Example: Among hospitalized patients, diabetes and a broken leg appear negatively correlated -- not because they protect against each other, but because both are reasons for hospitalization, and conditioning on "is hospitalized" creates the bias.

**Relevance:** FunDB's confounder search might inadvertently condition on a collider by filtering the dataset (e.g., only looking at records that match certain criteria), introducing spurious causal relationships.

### 2.3 Survivorship Bias

Analyzing only the entities that "survived" a selection process leads to false causal conclusions. Classic example: Abraham Wald's WWII bomber analysis -- bullet holes on returning planes are in non-critical areas precisely because planes hit in critical areas did not return.

**Relevance:** FunDB's data only contains records that were written. Churned users, failed experiments, deleted records, and crashed systems leave no trace. Any causal inference drawn from survivors will be biased.

### 2.4 Ecological Fallacy

Inferring individual-level causation from aggregate data. Example: Countries with higher chocolate consumption have more Nobel laureates, but this does not mean eating chocolate makes individuals smarter.

**Relevance:** FunDB may store aggregated metrics (daily counts, averages) alongside individual events. Granger causality on aggregate time series does not imply individual-level causation.

### 2.5 Post Hoc Ergo Propter Hoc

"After, therefore because of." The most ancient causal fallacy. A rooster crowing before sunrise does not cause the sun to rise.

**Relevance:** FunDB's temporal precedence signal is fundamentally an instance of testing for this fallacy. Without additional signals, temporal precedence alone is worth very little.

### 2.6 Regression to the Mean

Extreme observations tend to be followed by less extreme ones, purely due to random variation. If FunDB observes a spike followed by a decline, it may attribute the decline to an intervention that happened in between, when the decline was inevitable.

**Relevance:** Any "natural experiment" where the treatment was triggered by an extreme observation (e.g., "we changed the config because error rates were high") will suffer from this.

### 2.7 Multiple Comparisons Problem (P-Hacking)

If you test 20 hypotheses at p < 0.05, you expect 1 false positive by chance. FunDB's automatic confounder search and natural experiment detection will test many hypotheses simultaneously.

**Relevance:** Without correction (Bonferroni, Benjamini-Hochberg, or similar), the system will generate spurious causal claims proportional to the number of variables it searches.

### 2.8 Omitted Variable Bias

If a true confounder is not in the database, no statistical method can detect or correct for it. The estimated causal effect will be biased by the correlation between the treatment, the omitted variable, and the outcome.

**Relevance:** FunDB can only search for confounders among variables it has. If the real confounder is "the CEO sent a company-wide email" and that event is not logged in FunDB, the system will never find it.

---

## 3. Historical Cases Where Causal Inference Went Wrong

### 3.1 Hormone Replacement Therapy (HRT)

Observational studies throughout the 1980s-90s suggested HRT reduced cardiovascular risk in postmenopausal women. The WHI randomized trial (2002) showed the opposite: HRT slightly *increased* cardiovascular risk. The confounder was socioeconomic status -- women who took HRT were wealthier, exercised more, had better diets.

**Lesson:** Even with large datasets and "plausible mechanisms," observational causation can be completely wrong when unmeasured confounders exist.

### 3.2 The Pellagra Controversy

For decades, pellagra was believed to be caused by an infectious agent because it clustered in communities. Joseph Goldberger (1914) demonstrated it was caused by niacin deficiency. The "clustering" was due to poverty and shared diets.

**Lesson:** Strong co-occurrence and temporal patterns can arise from a shared upstream cause. FunDB's co-occurrence signal would have confidently pointed to "infection."

### 3.3 The MMR Vaccine and Autism

Andrew Wakefield's 1998 study claimed a causal link between the MMR vaccine and autism. The temporal signal was strong (vaccines given around the age autism is first diagnosed). The "mechanism" was plausible-sounding (gut-brain connection). Dozens of subsequent studies found zero causal relationship. The paper was retracted for fraud.

**Lesson:** All five of FunDB's proposed signals can align and still be wrong. Temporal precedence, plausible mechanism, apparent absence of confounders, apparent "natural experiments" (countries with different vaccine schedules), and "source consensus" (many papers citing the original) all pointed in the wrong direction.

### 3.4 The Replication Crisis

Between 2011-2015, large-scale replication projects found that 60-70% of published psychology findings failed to replicate (Open Science Collaboration, 2015). The causes: p-hacking, small samples, publication bias, and flexible analyses.

**Lesson:** "Source consensus" (many published papers agreeing) is not reliable when all sources share the same biases. FunDB must consider whether its sources are truly independent.

### 3.5 Google Flu Trends

Google Flu Trends used search query data to predict flu prevalence. It worked well from 2008-2011, then dramatically overpredicted flu activity in 2012-2013. The cause: distributional shift -- media coverage of flu changed search behavior without changing actual flu rates.

**Lesson:** A causal model trained on historical data will fail when the data-generating process changes. FunDB's natural experiments from 3 years ago may not apply today.

### 3.6 The Lancet Iraq Mortality Study (2006)

The study estimated 654,965 excess deaths in Iraq using cluster sampling. Subsequent analysis showed the confidence interval was enormous, the sampling methodology had flaws, and the point estimate was likely inflated by 2-3x. Yet the headline number was widely cited as if it were precise.

**Lesson:** Point estimates without properly calibrated uncertainty ranges are dangerous. FunDB must not return causal strength as a single number without honest confidence intervals.

---

## 4. Known Attacks on Statistical and ML Methods

### 4.1 Data Poisoning

An adversary injects carefully crafted data points to shift the distribution and manipulate model outputs. In FunDB's context: inserting fake events with specific timestamps and values to create spurious temporal precedence or Granger causality.

**Paper reference:** Biggio et al. (2012), "Poisoning Attacks against Support Vector Machines."

### 4.2 Concept Drift Exploitation

If the causal model is learned from historical data, an adversary (or natural process) can shift the data distribution so the model's causal claims become stale. The model continues making confident claims that are no longer valid.

**Paper reference:** Gama et al. (2014), "A survey on concept drift adaptation."

### 4.3 Manipulation of Instrumental Variables

In natural experiment detection, the system looks for "instruments" -- variables that affect the cause but not the effect directly. An adversary who understands this can introduce variables that appear to be valid instruments but violate the exclusion restriction.

### 4.4 Sybil Attacks on Source Consensus

If the consensus signal counts "number of independent sources that agree," an adversary can create many non-independent sources that all cite each other, inflating consensus without adding genuine evidence. This is analogous to citation rings in academia.

### 4.5 Causal Graph Manipulation via Selective Insertion

An adversary who understands FunDB's confounder search can prevent the system from finding a confounder by simply not logging it. If the confounder is never inserted into the database, the search will never find it, and the spurious causal claim will stand.

### 4.6 Timestamp Manipulation

If causal inference relies on temporal ordering, an adversary who controls the timestamps of inserted events can fabricate any temporal precedence pattern they want. Backdating events is trivially easy when the writer controls the timestamp field.

---

## 5. LLM/AI-Specific Failure Modes

### 5.1 Hallucinated Mechanisms

When the semantic mechanism signal relies on an LLM to evaluate whether "A causing B is plausible," the LLM may produce confident, detailed, and entirely fabricated explanations. LLMs are known to confabulate plausible-sounding causal chains that have no basis in reality.

**Example:** An LLM asked "Could increased server memory cause improved user satisfaction?" might produce: "Yes, increased memory reduces swap usage, which decreases latency, which improves page load times, which is a known driver of user satisfaction." This chain sounds reasonable but each link may be unsubstantiated for the specific system.

### 5.2 Distributional Shift in Embeddings

Semantic similarity between "cause" and "effect" embeddings depends on the embedding model. If the model is updated or the domain shifts, previously valid semantic connections may become invalid (or vice versa).

### 5.3 Overconfidence Calibration

Neural networks (including those used for embedding and mechanism evaluation) are notoriously poorly calibrated -- they assign high confidence to wrong answers. If FunDB uses model-generated confidence scores without recalibration, the system will systematically overstate its confidence.

**Paper reference:** Guo et al. (2017), "On Calibration of Modern Neural Networks."

### 5.4 Automation Bias

When an AI agent consumes FunDB's causal claims, it is likely to trust them without scrutiny. The more detailed and structured the output (paths, strengths, mechanisms), the more likely the downstream agent is to treat the output as ground truth. This is especially dangerous because the consumer is also an AI system that cannot apply common-sense skepticism.

### 5.5 Circular Reasoning in Multi-Agent Systems

If Agent A asks FunDB "why did X happen?", gets answer "because of Y", then writes "Y caused X" into FunDB as a confirmed fact, a future query will find both the inferred and the "confirmed" causal edge, doubling the apparent evidence. This is a feedback loop that can amplify initially weak signals into apparently strong causal claims.

---

## 6. Hard Limits That Cannot Be Engineered Away

1. **No observational data can substitute for a randomized experiment.** FunDB will never achieve the epistemic certainty of an RCT from database queries alone.

2. **Confounders that are not in the database cannot be found.** No algorithm can detect the influence of a variable that was never measured or recorded.

3. **Causal direction cannot always be determined from observational data.** Even with perfect data, some causal structures are Markov-equivalent and cannot be distinguished without interventional data or domain knowledge.

4. **The faithfulness assumption can fail.** Two causal paths can cancel each other out, creating zero observed correlation between genuinely causally linked variables.

5. **Non-stationarity invalidates time-series methods.** If the causal structure changes over time, methods that assume a fixed structure (Granger causality, co-occurrence patterns) will produce wrong answers without any detectable signal that they are wrong.

6. **Finite data means finite statistical power.** Rare causal relationships that are real may never reach significance in the available data. Common but weak causal relationships may be detected but are practically useless.

---

## 7. Key References

- Pearl, J. (2009). *Causality: Models, Reasoning, and Inference.* Cambridge University Press.
- Rubin, D.B. (1974). "Estimating causal effects of treatments in randomized and nonrandomized studies." *Journal of Educational Psychology.*
- Granger, C.W.J. (1969). "Investigating causal relations by econometric models and cross-spectral methods." *Econometrica.*
- Hernán, M.A. & Robins, J.M. (2020). *Causal Inference: What If.* Chapman & Hall/CRC.
- Open Science Collaboration (2015). "Estimating the reproducibility of psychological science." *Science.*
- Ioannidis, J.P.A. (2005). "Why Most Published Research Findings Are False." *PLoS Medicine.*
- Simpson, E.H. (1951). "The interpretation of interaction in contingency tables." *JRSS Series B.*
- Spirtes, P., Glymour, C., & Scheines, R. (2000). *Causation, Prediction, and Search.* MIT Press.
- Peters, J., Janzing, D., & Schölkopf, B. (2017). *Elements of Causal Inference.* MIT Press.
- Guo, C. et al. (2017). "On Calibration of Modern Neural Networks." *ICML.*
- Biggio, B. et al. (2012). "Poisoning Attacks against Support Vector Machines." *ICML.*
