# Frozen Final Candidate

Frozen before opening validation.

- Candidate: complete-turn beam v6 with explicit terminal tuple, critical-start survival focus, and Vulnerable-aware incoming damage.
- Planner source SHA-256: `A54CD079DE1258417284D7B23467F1D5777D98DE2C9BA7A2D6B6B83CF2AEFDB4`
- Planner diff SHA-256: `7ED1906DDD976A8648217BFA3A29435013DBAB7982E404F04F0EA02959A673DB`
- Evaluator binary SHA-256: `6232920BCC279783C82E869C1F53B5CF0260EA66C97F20C19538760947D02B8F`
- Development manifest FNV-64: `9e8cfdc0d1cc6681`
- Configuration: depth 100, width 300, 100,000 evaluator-counted transitions, 10,000 ms hard timeout.
- Incumbent: 115 wins, 0 losses, 9 nonterminal, p95 2,574 ms.
- Candidate reproduction: 123 wins, 1 loss, 0 nonterminal, p95 3,055 ms.
- Paired gates: 8 gained wins, no new loss/timeout on an incumbent win, minimum common-win HP delta -2, mean common-win HP delta +4.061, 11 improved lineages, p95 ratio 1.187.
- The sole loss starts at 4 HP with two incoming 5-damage attacks and no survivable legal turn; the incumbent left it nonterminal.
- Reproduction had zero per-root differences in outcome, terminal resources, action count, transition count, or budget status.
- First report SHA-256: `ACA2E3766319DF4DBAB92332F09BD5547B4B9282ABEA0F0C2604DB50733B996E`.
- Clean reproduction SHA-256: `6E6604FFA8536E4C5D5AC46AA8E7F7C531C261E1C69D948DD8D0C312DB4515F7`.
- Correctness gate: the candidate-created corpus benchmark failure was fixed; the focused corpus benchmark passes. The three unrelated pre-existing failures remain the permitted fingerprint.

No planner, evaluator, simulator, verifier, manifest, scoring, or methodology changes are permitted after this point. Validation is run exactly once. If it passes, held-out is run exactly once. No tuning follows either sealed result.
