# Phase 3A fidelity confidence gate

Status: **adopted design; tooling incomplete; gate not yet met**
Date: 2026-07-31
Scope: Ironclad A0 strict seed-start parity against the pinned target game
Related: [`verification.md`](verification.md), [`verification_status.md`](verification_status.md), [`../../PROJECT_OVERVIEW.md`](../../PROJECT_OVERVIEW.md), [`../../tools/communication/REPAIR_LOOP.md`](../../tools/communication/REPAIR_LOOP.md)

## The answer in plain English

The project's headline number is:

> **6,605 consecutive valid full-run trials with zero simulator-to-real
> fidelity failures.**

For a predeclared frozen test distribution, that result supports the following
exact frequentist statement:

> If the simulator's true full-run failure probability under that distribution
> were at least 1 in 1,000, the probability of observing all 6,605 runs clean
> would be at most 0.135% (one-sided 3σ).

When that result is accompanied by all of the non-statistical gates in this
document—zero known in-scope failures, a green permanent regression corpus, and
targeted evidence for rare or excluded mechanics—the project may use this
engineering conclusion:

> **We have high confidence that the simulator has full fidelity within the
> declared Ironclad A0 scope for the pinned builds.**

“Full fidelity” in that sentence is an engineering confidence judgment, not a
mathematical proof over every reachable state. The statistical result by itself
only bounds failure probability under the declared test distribution. It does
not cover omitted commands, ignored fields, future builds, A20, other
characters, or a future agent's potentially adversarial policy.

## 1. Authoritative Phase 3A exit

Phase 3A passes only when **all** of these gates are green in one
behaviorally-frozen epoch:

1. **Known-failure gate:** no unresolved reproducible simulator-to-real
   divergence remains in the enabled Ironclad A0 scope. Unsupported or
   unobservable behavior must be listed explicitly and narrows the scope of any
   fidelity claim.
2. **Regression gate:** every permanent and targeted fidelity regression
   verifies clean through its required endpoint with zero unexpected diffs,
   unsupported actions, ignored tails, duplicate dispositions, or unresolved
   transient assertions.
3. **Prospective G2 gate:** 6,605 consecutive valid full-run trials drawn from
   the frozen Phase 3A test distribution complete with zero fidelity failures
   and zero unresolved post-start collection outcomes.
4. **Coverage gate:** collector exclusions, comparison exclusions, and rare
   high-risk mechanics have a reviewed manifest. No action may be excluded
   merely because it has exposed a simulator divergence. Gameplay-affecting
   exclusions require targeted source-backed and trace-backed evidence or must
   remain explicitly outside the claim.
5. **Reporting gate:** the machine-readable epoch report described in §8 is
   complete and identifies the exact artifacts, sampling policy, seeds,
   outcomes, exclusions, and regression/coverage reports.

G2 is the simple answer to “how many clean runs?” It is deliberately not the
whole answer to “why should we trust the simulator?”

## 2. Formal statistical claim

### 2.1 Quantity being bounded

Let a complete trajectory \(\tau\) be drawn from the frozen Phase 3A test
distribution \(D_{\mathrm{P3A}}\). Define

\[
p_D = \Pr_{\tau \sim D_{\mathrm{P3A}}}
\left[
  \text{strict seed-start replay does not complete cleanly}
\right].
\]

The unit of analysis is one sampled real-game run, not one state or decision.
The run is a pass only if its entire recorded trajectory reaches a real terminal
state and verifies cleanly. An early parity divergence is a failed trial even
though collection or replay may stop before the game reaches terminal.

This \(p_D\) is distribution-specific. It is not the probability of a bug under
every human, search, or RL policy, and it is not the fraction of the abstract
state space implemented correctly.

### 2.2 Zero-failure upper bound

For \(n\) independent trials with zero failures, the one-sided
Clopper–Pearson upper confidence bound is

\[
p_{\mathrm{ub}}(n,\alpha) = 1-\alpha^{1/n}.
\]

For a one-sided 3σ tail,

\[
\alpha = 1-\Phi(3) \approx 0.001349898.
\]

Choosing \(p_\star=0.001\) gives

\[
n =
\left\lceil
  \frac{\ln(\alpha)}{\ln(1-p_\star)}
\right\rceil
= 6605,
\]

and

\[
p_{\mathrm{ub}}(6605,\alpha) \approx 0.000999913 < 0.001.
\]

The least ambiguous way to communicate the result is:

> Under the null hypothesis \(p_D \ge 0.001\), the chance of this fixed
> zero-failure result is at most 0.00135.

Do not phrase the frequentist result as a posterior probability that \(p_D\) is
below the threshold.

### 2.3 Operating behavior

The zero-failure rule is intentionally demanding:

| true \(p_D\) | chance that 6,605 trials are all clean |
|---:|---:|
| \(10^{-3}\) | 0.135% |
| \(10^{-4}\) | 51.7% |
| \(10^{-5}\) | 93.6% |

The gate has a 95% chance of passing only when the true failure rate is roughly
below \(7.8\times10^{-6}\). That conservatism is acceptable here because every
observed in-scope fidelity defect is expected to be repaired rather than
accepted as a routine failure allowance.

The choices \(p_\star=0.001\) and one-sided 3σ are project risk decisions, not
facts derived from the size of Slay the Spire's state space.

## 3. Frozen Phase 3A test distribution

The default distribution \(D_{\mathrm{P3A}}\) is a balanced random stress test
of the pinned Ironclad A0 target. It is frozen before a counting epoch begins.

### 3.1 Run-mode mixture

Each trial independently selects one of these modes with equal probability:

| mode | starting HP | terminal requirement | purpose |
|---|---:|---|---|
| natural | ordinary Ironclad A0 HP | any genuine game terminal, including early death | death, healing, low-HP, potion-pressure, and natural combat lines |
| deep | 10,000 verification HP | any genuine game terminal | sustained late-act and long-run coverage |

The equal weighting is an adopted engineering compromise: natural mode covers
ordinary health-dependent behavior while deep mode applies sustained pressure
to later-act mechanics. Changing these weights defines a new distribution and
therefore a new epoch.

Every terminal natural-HP run counts, regardless of floor. Conditioning natural
runs on reaching floor 50 would silently discard the low-HP behavior this mode
exists to test.

The deep mode is an artificial distribution. Setting both current and maximum
HP to 10,000 changes percentage-HP, healing, threshold, event, and encounter
behavior; it is useful for coverage but is not natural play.

The epoch report must show results and exposure counts separately for natural
and deep trials even though the primary G2 bound applies to their declared
50/50 mixture. If the project wants the same \(p<0.001\), one-sided 3σ claim
separately for each mode, it must collect 6,605 clean trials in each mode.

### 3.2 Action policy

At every stable target-game decision, select uniformly from the frozen
fidelity-eligible command set produced from the target observation. Pure state
polls and commands that do not advance gameplay are not policy actions.

“Fidelity-eligible” must not be shorthand for silently removing difficult
behavior. The policy manifest must inventory every command filter and classify
it as:

- non-gameplay protocol machinery;
- target behavior that cannot currently be observed or settled;
- intentionally out of project scope; or
- a temporary coverage defect that blocks the full-scope gate.

A command known to produce a simulator divergence may never be filtered out to
protect the streak. It remains a failure until repaired. Observation-limited
gameplay may be excluded from the random policy only if the limitation is
explicit in the final claim and the behavior has targeted evidence where
possible.

The current `random_fidelity_collector.js` policy is **not yet eligible** for a
full-scope counting epoch: it filters several accepted gameplay commands,
including commands associated with known divergence or unsettled observations.
Its existing traces remain valuable discovery and regression evidence, but
they do not constitute the prospective G2 batch described here.

### 3.3 Seed and independence protocol

Before collection:

1. Generate and record one campaign master seed.
2. Use domain-separated deterministic generation to produce independent-looking
   run-mode choices, game seeds, and action-policy seeds.
3. Predeclare the complete ordered holdout bank.
4. Freeze target profile and unlock state.
5. Do not use the holdout bank for simulator development before the epoch.

Game seed and policy seed must not be the same sequential integer with different
formatting. Reproducibility is required, but consecutive deterministic seed
enumeration alone does not establish the i.i.d. model used by the bound.

When a holdout trial reveals a bug, promote it to permanent regression evidence,
repair the bug, create a new behavioral epoch, and generate a fresh holdout bank.

### 3.4 Artifact and configuration lock

The epoch identity must use content hashes, not only a repository commit:

- target game build and ModTheSpire;
- every loaded mod JAR, including CommunicationMod and verification bootstrap;
- collector source and policy/exclusion configuration;
- verifier binary and simulator/core/content artifacts;
- target profile, unlocks, language, character, and ascension;
- starting-HP mixture and terminal/max-action settings;
- comparison and observability exclusion manifest; and
- trace schema and external-RNG metadata contract.

`source_version: working-tree` is not a valid counting-epoch identity. A
documentation-only repository commit does not reset an epoch when all
behavior-affecting hashes remain identical; an uncommitted behavioral change
does reset it.

## 4. Trial outcome contract

A trial begins after the intended run has started and the collector has observed
the first stable in-game state for the predeclared seed and configuration.

### Pass

A trial passes only when:

- the target reaches a genuine terminal state;
- the immutable full trace is available;
- strict seed-start parity is clean through EOF;
- every applicable action has exactly one disposition; and
- there are zero unexpected diffs, unsupported transitions, ignored tails,
  duplicate dispositions, and unresolved transient assertions.

Early target death is a valid terminal outcome. A max-action cutoff is not.

### Fidelity failure

Any simulator-to-real mismatch, unsupported in-scope transition, comparison
boundary, or action-integrity failure after trial start fails G2 immediately.
The resulting trace need not be terminal to count as the failed trial.

### Infrastructure outcomes

- A launch or bridge failure before trial start is logged and retried using the
  same predeclared trial. It neither passes nor fails.
- A target crash, collector hang, accepted-command settlement timeout, verifier
  crash, trace-integrity problem, or max-action cutoff after trial start is
  **unresolved**, not silently excluded. Counting stops until it is classified
  and resolved.
- A post-start outcome may be declared external infrastructure only with
  recorded evidence that it is independent of the sampled gameplay path.
  If accepted, retry the same predeclared trial; otherwise it remains a gate
  blocker.
- A sampled seed may not be skipped after repeated post-start failures.

This prevents gameplay-triggered collection problems from disappearing as
“missing” trials and biasing the observed fidelity rate downward.

## 5. Prospective counting and epoch resets

The allowed simple protocol is:

```text
freeze artifacts, distribution, policy manifest, and ordered holdout bank
valid_clean_trials = 0

for trial in the predeclared bank:
    collect and verify the trial
    if failure:
        FAIL epoch; retain evidence; repair; start a fresh epoch
    if unresolved after trial start:
        BLOCK epoch until classified and resolved
    if invalid before trial start:
        retry the same trial
    if full terminal clean pass:
        valid_clean_trials += 1
    if valid_clean_trials == 6605:
        PASS prospective G2
```

“6,605 in a row” means 6,605 consecutive **valid trials in one frozen
behavioral epoch**, with no intervening fidelity failure. It does not mean
scanning an adaptive history for its longest green streak.

Do not:

- count discovery traces used to choose or repair implementation;
- restart after a failure and report only the best historical streak;
- pool different behavioral builds or undeclared distributions;
- count clean non-terminal prefixes as full-run successes;
- replace a failed holdout seed with a more convenient one; or
- continue beyond a predeclared stopping point until the result looks favorable.

If a future project needs continuous monitoring rather than a fixed batch, it
must adopt and document an appropriate sequential test or confidence sequence.

## 6. Coverage and regression evidence

Random full runs detect defects in proportion to their probability under the
test policy. Rare but important mechanics can remain almost invisible even in
6,605 runs. G2 therefore complements rather than replaces:

| evidence | role |
|---|---|
| `permanent_traces/*.jsonl` | regression lock for known witnesses and durable clean coverage |
| `open_failures/` and repair fingerprints | truth about known unresolved divergence |
| targeted fidelity regressions | rare mechanics, excluded actions, and high-risk combinations |
| source-backed local tests | narrow mechanics that full traces cannot isolate cleanly |
| prospective G2 batch | bound on residual full-run failure mass under \(D_{\mathrm{P3A}}\) |

The coverage report should at least summarize exposure by act, floor range,
room/screen type, encounter, event, shop/reward action family, card/relic/potion
mechanic tags, and natural/deep mode. Exposure counts are diagnostics and
minimum-coverage checks; they are not substitutes for the trajectory-level
statistical bound.

Known comparison exclusions must be versioned and visible in the report. A
field that the verifier does not compare is not validated by a clean streak.

## 7. What a pass authorizes

After every §1 gate passes, the project may state:

> Phase 3A passed for the pinned Ironclad A0 scope. We observed zero
> simulator-to-real fidelity failures in 6,605 prospectively sampled complete
> runs under the declared balanced natural/deep random distribution. If the
> true failure rate under that distribution were at least 0.1%, this result
> would occur by chance at most 0.135% of the time. Together with zero known
> in-scope failures and green regression and targeted-coverage gates, this gives
> high engineering confidence in full fidelity within the declared scope.

The pass does not authorize:

- a proof that no unreachable or extremely rare bug exists;
- A20, Heart, non-Ironclad, or future-content fidelity claims;
- claims about gameplay omitted by the policy or comparison manifest;
- a bound under human, search, guided, or RL policies; or
- reuse of the conclusion after a behavior-affecting artifact changes.

Before a search or RL policy is trusted, run a separate differential audit on
traces produced by that actual policy. Agents may concentrate probability on
states that random play almost never reaches, so Phase 3A evidence must be
supplemented by continuing on-policy and adversarial verification.

## 8. Required report

A claimed pass must publish a machine-readable report containing at least:

```yaml
schema: 1
gate: phase3a_fidelity_confidence_g2
status: pass
alpha: 0.0013498980316301
p_star: 0.001
n_required: 6605
n_started: 6605
n_clean_terminal: 6605
n_fidelity_failures: 0
n_post_start_unresolved: 0
n_pre_start_retries: <int>

distribution:
  name: D_P3A_balanced_natural_deep
  character: IRONCLAD
  ascension: 0
  mode_probabilities:
    natural: 0.5
    deep_10000_hp: 0.5
  action_policy_hash: <sha256>
  action_exclusion_manifest: <path and sha256>
  ordered_holdout_bank: <path and sha256>
  campaign_master_seed: <recorded seed>

epoch:
  artifact_manifest: <path and sha256>
  comparison_manifest: <path and sha256>
  target_profile_hash: <sha256>
  first_trial_utc: <timestamp>
  last_trial_utc: <timestamp>

supporting_gates:
  open_in_scope_failures: 0
  permanent_corpus_report: <path and sha256>
  targeted_fidelity_report: <path and sha256>
  coverage_report: <path and sha256>
```

Without this report, “we had thousands of clean runs” is useful anecdotal
evidence, not a Phase 3A gate pass.

## 9. Milestones

These zero-failure milestones use the same one-sided 3σ tail:

| milestone | bound under the declared \(D\) | required clean trials | meaning |
|---|---:|---:|---|
| G0 | \(p_D < 0.1\) | 63 | pipeline smoke test |
| G1 | \(p_D < 0.01\) | 658 | weak readiness signal |
| **G2** | **\(p_D < 0.001\)** | **6,605** | statistical component of Phase 3A exit |

G0 and G1 are progress markers, not substitutes for G2 or the non-statistical
gates.

## 10. Current implementation status

The design is adopted, but the current random-fidelity pipeline is a discovery
system rather than a valid G2 evaluator. Before any traces count, the project
still needs to:

- implement artifact/configuration epoch hashing;
- replace linked sequential game/policy seeds with the predeclared holdout-bank
  protocol;
- implement the balanced natural/deep mode selection;
- audit and classify the collector's action filters;
- prevent post-start failures from being skipped;
- enforce the full terminal outcome contract; and
- emit the §8 report.

Existing random traces, including the corpus used for earlier branching
measurements, remain valuable discovery and regression evidence. They are not a
prospective frozen holdout batch and must not be retroactively counted toward
G2.

## 11. Why state-space size does not determine 6,605

Slay the Spire's reachable state/action tree is vastly larger than any feasible
trace corpus. That does not determine the sample size for this claim.

Let \(B\) be the set of failing trajectories. Random testing sees \(B\) with
probability \(p_D=\Pr_D(B)\), regardless of how many abstract states exist.
After \(n\) independent trials, the probability of seeing no member of \(B\) is
\((1-p_D)^n\). The 6,605 count comes from the tolerated trajectory failure rate
and confidence level, not from an attempt to enumerate the game.

A defect with extremely small mass under \(D\) remains invisible to feasible
black-box sampling. Source review, targeted traces, regression tests, coverage
audits, and later on-policy verification exist to address that limitation.

## Change log

| date | change |
|---|---|
| 2026-07-30 | Initial zero-failure Clopper–Pearson proposal. |
| 2026-07-31 | Replaced the single deep filtered-random exit with a combined fidelity confidence gate; corrected the sampling, independence, natural-HP, infrastructure, epoch, and claim-scope contracts. |
