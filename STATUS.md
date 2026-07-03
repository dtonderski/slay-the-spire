# Status

## What Exists

### Tooling
- Card-fidelity fix: verified Magnetism/Magnetism+ against `Magnetism.java`
  and `MagnetismPower.java`, then fixed generated-card hand overflow. Source
  applies one Magnetism stack, and at the start of each turn uses
  `MakeTempCardInHandAction` to add one random colorless combat card per stack;
  that action respects the 10-card hand cap and sends overflow to discard. Local
  Magnetism previously pushed generated cards directly into hand and could create
  an 11-card hand. The simulator now keeps the same modeled colorless pool/RNG
  path but routes full-hand generated cards to discard, with focused coverage for
  the overflow case. Remaining caveat: exact source
  `returnTrulyRandomColorlessCardInCombat` pool/order is still represented by the
  local modeled colorless pool. Checks: `cargo fmt` passed; `cargo test -p
  sts_core magnetism_generated_card_overflows_full_hand_to_discard` passed;
  `cargo test -p sts_core --test card_fidelity` passed (30 tests); `cargo
  clippy` passed with existing warnings after setting `PYO3_PYTHON` to the
  bundled Python. Main-workspace `cargo test` is currently blocked by
  `py_sts`/`sts_omni` exiting with `STATUS_DLL_NOT_FOUND`, and the dirty
  main-workspace live replay still fails on local
  `trace-2026-07-02T23-24-13-178Z.jsonl` / manifest state (`steps` 356 vs 335).
  In a temporary clean worktree with only this staged slice applied, the focused
  Magnetism regression passed, card-fidelity passed, and clippy passed with the
  same existing warnings. Committed-corpus replay currently fails even on plain
  `HEAD` before this slice, at trace step 353 of
  `trace-2026-07-02T23-24-13-178Z.jsonl`, with a Wound/discard-pile ordering
  diff unrelated to Magnetism.
- Card-fidelity fix: verified Madness/Madness+ against `Madness.java` and
  `MadnessAction.java`, then fixed eligible-card selection. Source first
  prefers hand cards with `costForTurn > 0`, repeatedly rolling
  `cardRandomRng` over the hand until it hits an eligible card, and only falls
  back to positive printed-cost cards when no positive current-turn-cost cards
  exist. Local Madness previously selected any random hand card and could waste
  the effect on an already-0 current-turn-cost card while a better candidate
  existed. Local Madness now mirrors the source eligibility split, uses
  rejection sampling when `card_random_rng` is present, sets the chosen card's
  combat-long temp cost to 0, and keeps source Exhaust/cost metadata unchanged.
  Checks: `cargo fmt` passed. Main-workspace `cargo test`/`cargo clippy` are
  currently blocked by unrelated dirty `CombatState`/`turn.rs` edits where
  `turn.rs` references `discard_reshuffle_limit` but the dirty `state.rs` shape
  does not expose that field; active live-regression replay in the dirty main
  workspace still fails on local `trace-2026-07-02T23-24-13-178Z.jsonl`. In a
  temporary clean worktree with this slice applied, `cargo test -p sts_core
  --test card_fidelity madness` passed (1 test), `cargo test -p sts_core --test
  card_fidelity` passed (30 tests), `cargo clippy` passed with existing warnings
  after setting `PYO3_PYTHON` to the bundled Python, and `uv run python -m
  unittest python.tests.test_live_regression_traces` passed against the
  committed corpus.
- Card-fidelity audit: verified Jack of All Trades/Jack of All Trades+ against
  `JackOfAllTrades.java` and `MakeTempCardInHandAction.java`. Source is a
  0-cost colorless uncommon Skill with Exhaust that adds one random colorless
  combat card to hand, or two independently rolled cards after upgrade; generated
  cards keep normal cost and overflow to discard when the hand is full. Local
  definitions/effects already matched the generic count/cost/Exhaust/hand-cap
  behavior, so no simulator code fix was needed. Remaining caveat: exact
  source `returnTrulyRandomColorlessCardInCombat` pool/order is still represented
  by the local modeled colorless pool. Checks: `git diff --check -- STATUS.md
  simulator/docs/audit/card_fidelity_audit.md` passed with only existing CRLF
  warnings; `cargo test -p sts_core --test card_fidelity` passed (29 tests).
  Active live-regression replay in the dirty main workspace still fails on
  local `trace-2026-07-02T23-24-13-178Z.jsonl`; in a temporary clean worktree
  with this slice applied, `cargo test -p sts_core --test card_fidelity` passed
  (29 tests) and `uv run python -m unittest
  python.tests.test_live_regression_traces` passed against the committed corpus.
- Card-fidelity audit: verified J.A.X./J.A.X.+ against `JAX.java`. Source is
  a 0-cost colorless Special Skill targeting self with no keywords; play loses
  3 HP, then grants 2 Strength, and upgrade only raises the Strength gain to 3.
  Local definitions/effects already matched the generic combat behavior,
  including card-source HP loss hooks for Blood for Blood/Rupture/relics, so no
  simulator code fix was needed. Remaining blocker: local reward metadata cannot
  express STS Special rarity because `CardRarity` currently only has
  Common/Uncommon/Rare and maps event cards such as J.A.X. to Rare. Checks:
  `git diff --check -- STATUS.md simulator/docs/audit/card_fidelity_audit.md`
  passed with only existing CRLF warnings; `cargo test -p sts_core --test
  card_fidelity` passed (29 tests). Active live-regression replay in the dirty
  main workspace still fails on local
  `trace-2026-07-02T23-24-13-178Z.jsonl`; in a temporary clean worktree with
  this slice applied, `cargo test -p sts_core --test card_fidelity` passed (29
  tests) and `uv run python -m unittest python.tests.test_live_regression_traces`
  passed against the committed corpus.
- Card-fidelity fix: verified Hand of Greed/Hand of Greed+ against
  `HandOfGreed.java` and `GreedAction.java`, then fixed fatal-gold behavior.
  Source deals targeted damage and grants 20/25 gold only when the target dies
  and is not half-dead or a Minion; local previously routed both forms through
  the plain attack path and never awarded gold. The simulator now records
  combat gold gained by Hand of Greed, excludes Minion kills, and transfers the
  gained amount into run gold through the combat wrapper. Checks: `cargo fmt`
  passed; `cargo test -p sts_core --test card_fidelity hand_of_greed` passed
  (3 tests); `cargo test -p sts_core --test card_fidelity` passed (29 tests);
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Active live-regression replay in the dirty main workspace
  currently fails because local `trace-2026-07-02T23-24-13-178Z.jsonl` /
  manifest state expects 302 steps while replay produces 308; in a temporary
  clean worktree with this slice applied, `cargo test -p sts_core --test
  card_fidelity` passed and `uv run python -m unittest
  python.tests.test_live_regression_traces` passed against the committed corpus.
- Card-fidelity fix: verified Forethought/Forethought+ against
  `Forethought.java` and `ForethoughtAction.java`, then fixed draw-pile
  placement. Source moves selected cards to the bottom of the draw pile and sets
  positive-cost moved cards free to play once; local confirm previously pushed
  the selected card to the simulator draw-pile top. Local Forethought now inserts
  the selected zero-cost card at the bottom of the bottom-first draw pile, with
  focused coverage preserving the prior top card as the next draw. Remaining
  blockers: base Forethought still requires another hand card and opens
  selection where source would do nothing or auto-move the only remaining hand
  card, and upgraded Forethought still uses the simulator's single-card
  hand-select model instead of source any-number selection. Checks: `cargo fmt`
  passed in the main workspace; in a temporary clean worktree with this slice
  applied, `cargo test -p sts_core --test card_fidelity` passed (26 tests) and
  `uv run python -m unittest python.tests.test_live_regression_traces` passed.
  Main-workspace `cargo test -p sts_core --test card_fidelity` is currently
  blocked by unrelated dirty `run/map.rs`/`content/monsters.rs` changes that
  remove `target_monster_hp_range_for_game_id`/`target_game_monster_id`.
- Card-fidelity audit: verified Enlightenment/Enlightenment+ against
  `Enlightenment.java` and `EnlightenmentAction.java`. Source costs 0, has no
  keywords, and sets hand cards with `costForTurn > 1` to cost 1 for the turn;
  upgraded form passes `forCombat = true` and additionally makes printed combat
  costs above 1 become 1 for the rest of combat. Local normal and
  Havoc/top-draw paths already use the same turn-only vs combat-long helper and
  exclude the played card, so no simulator code fix was needed. Checks: `git
  diff --check -- STATUS.md simulator/docs/audit/card_fidelity_audit.md` passed
  with only existing CRLF warnings. Active live-regression manifest replay still
  fails on dirty worktree trace
  `verification/corpus/communication_mod/trace-2026-07-02T23-24-13-178Z.jsonl`
  after appended post-baseline rows, with the same final Gremlin Leader combat
  state diff unrelated to Enlightenment; replaying the committed clean copy of
  that same trace from a temp file verified `true` with `trace_exhausted`, 295
  steps, final phase `combat`.
- Card-fidelity fix: verified Discovery/Discovery+ against `Discovery.java` and
  `DiscoveryAction.java`, then fixed upgraded definition fidelity. Source
  Discovery costs 1, Exhausts, opens 3 unique random combat-card choices, and
  makes the selected generated card cost 0 for the turn; upgrade only removes
  Exhaust and updates description, leaving cost at 1. Local Discovery+ previously
  cost 0 and retained Exhaust through shared base keywords. Discovery+ now costs
  1 with no Exhaust keyword, and focused tests cover the definition facts plus
  delayed source-card movement to discard after the Discovery reward closes.
  Checks: `cargo fmt` and `cargo fmt --check` passed; `cargo test -p sts_core
  --test card_fidelity` passed (25 tests); `cargo clippy` passed with existing
  warnings after setting `PYO3_PYTHON` to the bundled Python. Active
  live-regression manifest replay currently fails on dirty worktree trace
  `verification/corpus/communication_mod/trace-2026-07-02T23-24-13-178Z.jsonl`
  after appended post-baseline state rows, with a final Gremlin Leader combat
  state diff unrelated to Discovery; replaying the committed clean copy of that
  same trace from a temp file still verified `true` with `trace_exhausted`, 295
  steps, final phase `combat`. Full `cargo test` still fails only in the
  pre-existing stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: verified Chrysalis/Chrysalis+ against
  `Chrysalis.java` and fixed upgraded free-play generation. Source generates 3
  random combat Skills for base and 5 for upgraded, sets positive-cost copies to
  cost 0 for combat, shuffles them into draw, and Exhausts. Local normal play
  already modeled the count and combat-long zero-cost generated cards, but
  Havoc/top-draw free-play only handled base Chrysalis and generated nothing for
  Chrysalis+. The top-draw path now handles base/upgraded counts together, with
  focused coverage for base generated zero-cost cards and upgraded Havoc/top-draw
  generation. Checks: `cargo fmt` and `cargo fmt --check` passed; `cargo test
  -p sts_core --test card_fidelity` passed (23 tests); active live-regression
  manifest replay passed via `uv run python -m unittest
  python.tests.test_live_regression_traces`; `cargo clippy` passed with existing
  warnings after setting `PYO3_PYTHON` to the bundled Python. Full `cargo test`
  still fails only in the pre-existing stale `milestone6` monster fixture
  expectations: `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit: verified Apotheosis/Apotheosis+ rows against
  `Apotheosis.java` and `ApotheosisAction.java`. Source constructs a 2-cost
  colorless rare Skill with Exhaust, queues `ApotheosisAction`, and upgrade
  only reduces base cost to 1; `ApotheosisAction` upgrades hand, draw, discard,
  and exhaust piles. Local base/upgraded definitions already retain Exhaust and
  local `UpgradeCombatCards` covers the same four piles, so no simulator code
  fix was needed. Checks: `git diff --check -- STATUS.md
  simulator/docs/audit/card_fidelity_audit.md` passed with only existing CRLF
  warnings; active live-regression manifest replay passed via `uv run python -m
  unittest python.tests.test_live_regression_traces`.
- Card-fidelity audit: verified Apparition/Apparition+ rows against
  `Apparition.java`. Source constructs the base card as a 1-cost Colorless
  Special Skill targeting self with Exhaust and Ethereal, applies 1 Intangible,
  and upgrade only removes Ethereal. Local combat definitions and effect queue
  already match the generic play behavior (`GainIntangible { amount: 1 }` and
  Exhaust destination), so no simulator code fix was needed. Remaining blocker:
  local reward metadata cannot express STS Special rarity because `CardRarity`
  currently only has Common/Uncommon/Rare and maps these event cards to Rare.
  Checks: `git diff --check -- STATUS.md
  simulator/docs/audit/card_fidelity_audit.md` passed with only existing CRLF
  warnings; active live-regression manifest replay passed via `uv run python -m
  unittest python.tests.test_live_regression_traces`.
- Card-fidelity fix: corrected Bite audit row and fixed healing fidelity.
  Decompiled `Bite.java` deals 7 damage, then heals fixed `magicNumber` 2
  regardless of unblocked damage; local Bite previously used unblocked-damage
  lifesteal, so a fully blocked Bite healed 0. The simulator now deals damage
  and then heals 2, with focused coverage for blocked-damage healing. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed (21 tests); active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Bandage Up/Bandage Up+ audit rows and fixed
  upgraded healing. Decompiled `BandageUp.java` heals by `magicNumber` (4 base,
  6 upgraded after `upgradeMagicNumber(2)`) and Exhausts; local Bandage Up+
  previously reused the base heal amount 4. The simulator now heals 4/6 for
  base/upgraded forms, with focused tests covering healing and Exhaust behavior.
  Checks: `cargo fmt --check` passed; `cargo test -p sts_core --test
  card_fidelity` passed (20 tests); active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit: corrected Dark Shackles/Dark Shackles+ rows with
  source-backed facts from `DarkShackles.java`. Source applies temporary
  Strength loss by pairing negative Strength with conditional GainStrength only
  when Artifact did not block the debuff; local `ReduceMonsterStrengthThisTurn`
  already models that Artifact gate and restoration abstraction for 9/15
  Strength. No simulator code fix was needed. Checks: `git diff --check`
  passed with only existing CRLF warnings; active live-regression manifest
  replay passed via `uv run python -m unittest
  python.tests.test_live_regression_traces`.
- Card-fidelity audit: corrected Blind/Blind+ rows with source-backed facts
  from `Blind.java`. Source applies 2 Weak to one enemy for base and to all
  enemies after upgrade; local definitions/effects already match at the generic
  simulator level, with Weak amount represented in effect queues rather than
  `CardValues`. No simulator code fix was needed. Checks: `git diff --check`
  passed with only existing CRLF warnings; active live-regression manifest
  replay passed via `uv run python -m unittest
  python.tests.test_live_regression_traces`.
- Card-fidelity fix: corrected Trip/Trip+ audit rows and fixed upgraded target
  metadata. Decompiled `Trip.java` upgrades target from enemy to all-enemy and
  applies 2 Vulnerable to each monster. Local Trip+ effect behavior already
  applied Vulnerable to all living monsters, but the definition and audit row
  advertised target None. Trip+ now uses `TargetRequirement::AllEnemies`, with
  focused legal-action coverage for no selected target and selected-target
  rejection. Checks: `cargo fmt --check` passed; `cargo test -p sts_core --test
  card_fidelity` passed (18 tests); active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Transmutation/Transmutation+ audit rows and
  fixed printed X-cost metadata. Decompiled `Transmutation.java` uses source
  cost `-1`, exhausts, and queues `TransmutationAction` with current
  energy/free-play semantics; local play behavior already spent current energy,
  applied Chemical X bonus uses, generated temp-cost-0 random colorless cards,
  and exhausted, but the definitions still advertised cost 0. Transmutation
  definitions now use `cost: -1`, with focused definition coverage for base and
  upgraded forms. Checks: `cargo fmt --check` passed; `cargo test -p sts_core
  --test card_fidelity` passed (17 tests); active live-regression manifest
  replay passed via `uv run python -m unittest
  python.tests.test_live_regression_traces`; `cargo clippy` passed with
  existing warnings after setting `PYO3_PYTHON` to the bundled Python. Full
  `cargo test` still fails only in the pre-existing stale `milestone6` monster
  fixture expectations: `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Whirlwind/Whirlwind+ audit rows and fixed
  printed X-cost metadata. Decompiled `Whirlwind.java` uses source cost `-1`
  and queues `WhirlwindAction` with current energy/free-play semantics; local
  play behavior already spent current energy plus Chemical X bonus uses, but
  `CardDefinition.cost` was unsigned and flattened Whirlwind to cost 0. Local
  card definitions now support signed costs and Whirlwind uses `cost: -1`, with
  focused definition coverage for base and upgraded forms. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed (16 tests); active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit: corrected Wild Strike/Wild Strike+ rows with
  source-backed facts from `WildStrike.java`. Source deals selected-enemy
  damage, then queues `MakeTempCardInDrawPileAction(new Wound(), 1, true,
  true)`; upgrade adds 5 damage. Local normal play and Havoc/top-draw free-play
  already use generated random-spot Wound insertion, so no simulator code fix
  was needed. Checks: `git diff --check` passed with only existing CRLF
  warnings; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`.
- Card-fidelity fix: corrected Reckless Charge/Reckless Charge+ audit rows and
  fixed the generic simulator Dazed insertion path. Decompiled
  `RecklessCharge.java` deals selected-enemy damage, then queues
  `MakeTempCardInDrawPileAction(new Dazed(), 1, true, true)`; local normal play
  and Havoc/top-draw free-play previously used plain draw-pile insertion, which
  created a non-generated Dazed at a deterministic pile position. The simulator
  now uses generated random-spot draw-pile insertion for both paths, with
  focused tests for base normal play and upgraded top-draw play. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed (15 tests); active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest bridge-control fix: full potion belts no longer wedge the live UI on
  "Waiting for command ack" after selecting a potion reward. The trace client
  now summarizes potion capacity/open slots, the bridge action list disables
  the combat-reward potion choice when `open_potion_slots == 0`, and stale TCP
  pending-command status is ignored once the observed bridge files are stale
  and no command file remains. Checks: `node tools\communication\trace_client.test.js`
  passed; `uv run python -m unittest python.tests.test_bridge_mirror` passed.
- Latest live trace fidelity fix: implemented the source-backed Pain curse
  trigger exposed by `trace-2026-07-02T22-41-59-925Z.jsonl` at trace step 42.
  Decompiled `Pain.java` triggers `LoseHPAction(..., 1)` whenever another card
  is played while Pain is in hand; the simulator now applies hand-card play
  triggers from the normal `PlayCard` transition, so playing Defend with Pain
  in hand drops HP from 62 to 61 like the real game. Checks: `cargo fmt`
  passed; `cargo check -p sts_core --lib` passed;
  `uv run maturin develop --release` passed; strict replay of
  `verification/corpus/communication_mod/trace-2026-07-02T22-41-59-925Z.jsonl`
  passed to trace exhaustion (`verified True`, 43 steps, final phase reward).
- Card-fidelity fix: corrected Flash of Steel/Flash of Steel+ audit rows and
  fixed the generic Havoc/top-draw free-play path for Flash of Steel+. Source
  `FlashOfSteel.java` deals selected-enemy damage, draws 1 card, and upgrades
  damage by 3; normal local play already matched, but the `PlayTopDrawCard`
  branch only recognized base Flash of Steel and omitted upgraded damage/draw.
  The free-play branch now handles both forms, and `card_fidelity.rs` covers
  Havoc playing Flash of Steel+ from the draw pile. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Deep Breath/Deep Breath+ audit rows and fixed
  the generic simulator play path. Decompiled `DeepBreath.java` shuffles the
  discard pile into the draw pile when discard is non-empty, then draws
  `magicNumber` cards (1 base, 2 upgraded); local Deep Breath previously only
  drew cards. The simulator now has a source-backed internal shuffle action
  used before Deep Breath draw in normal play and top-draw/free-play follow-up
  handling, with focused card-fidelity tests for base and upgraded draw counts.
  Checks: `cargo fmt --check` passed after narrow test formatting; `git diff
  --check` passed; `cargo test -p sts_core --test card_fidelity` passed (9
  tests); active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Impatience/Impatience+ audit rows and fixed
  generic simulator play behavior. Decompiled `Impatience.java` queues
  `ConditionalDrawAction(magicNumber, ATTACK)`: the card is playable even with
  Attacks in hand, but draws only when the current hand has no Attack cards.
  Local play previously rejected Impatience when an Attack was in hand, and the
  Havoc/top-draw free-play path drew unconditionally and only covered the base
  form. The simulator now uses a conditional no-Attacks draw action for normal
  and top-draw/free-play paths, with focused tests for playability with an
  Attack in hand and upgraded draw 3 behavior. Checks: `cargo fmt --check`
  passed; `git diff --check` passed; `cargo test -p sts_core --test
  card_fidelity` passed (11 tests); active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Master of Strategy/Master of Strategy+ audit
  rows and fixed the generic Havoc/top-draw free-play path for the upgraded
  form. Decompiled `MasterOfStrategy.java` is a cost 0 Skill with
  `magicNumber=3`, Exhaust, and `DrawCardAction(p, this.magicNumber)`;
  upgrading increases magic by 1. Normal local play already drew 3/4 and
  exhausted, but the top-draw branch only handled the base form. The free-play
  branch now handles Master of Strategy+ and draws 4, with a focused Havoc
  regression test. Checks: `cargo fmt --check` passed; `git diff --check`
  passed; `cargo test -p sts_core --test card_fidelity` passed (12 tests);
  active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity fix: corrected Battle Trance/Battle Trance+ audit rows and
  fixed the generic Havoc/top-draw free-play path. Decompiled
  `BattleTrance.java` draws `magicNumber` cards (3 base, 4 upgraded), then
  applies `NoDrawPower`; normal local play already drew 3/4 and set
  `cannot_draw`, but the top-draw branch omitted both Battle Trance forms. The
  free-play branch now handles base/upgraded draw counts and sets no-draw, with
  a focused Havoc regression test for Battle Trance+. Checks:
  `cargo fmt --check` passed; `git diff --check` passed;
  `cargo test -p sts_core --test card_fidelity` passed (13 tests); active
  live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the Finesse/Finesse+ audit rows
  after source/local re-check showed the simulator already matched decompiled
  `Finesse.java`. Source Finesse is a cost 0 self-targeting Skill with
  `baseBlock=2`, queues `GainBlockAction` then `DrawCardAction(1)`, and
  upgrades block by 2; the artifact now records base/upgraded local behavior
  as 2/4 Block plus draw 1 through the explicit `finesse_queue` instead of
  stale generic draw/block wording. Checks: `cargo fmt --check` passed;
  `git diff --check` passed; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the Shrug It Off/Shrug It Off+
  audit rows after source/local re-check showed the simulator already matched
  decompiled `ShrugItOff.java`. Source Shrug It Off is a cost 1 self-targeting
  Skill with `baseBlock=8`, queues `GainBlockAction` then
  `DrawCardAction(1)`, and upgrades block by 3; the artifact now records
  base/upgraded local behavior as 8/11 Block plus draw 1 through the explicit
  `shrug_it_off_queue` instead of stale generic draw/block wording. Checks:
  `cargo fmt --check` passed; `git diff --check` passed; active
  live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the Bludgeon/Bludgeon+ audit rows
  after source/local re-check showed the simulator already matched decompiled
  `Bludgeon.java`. Source Bludgeon is a cost 3 enemy Attack with
  `baseDamage=32`, queues selected-enemy damage after VFX/Wait actions, and
  upgrades damage by 10; the artifact now records base/upgraded local behavior
  as selected-enemy 32/42 damage through the generic attack queue instead of
  the stale definition-only fallback wording. Checks: `cargo fmt --check`
  passed; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the Sentinel/Sentinel+ audit rows
  after source/local re-check showed the simulator already matched decompiled
  `Sentinel.java`. Source Sentinel is a cost 1 self-targeting Skill with
  `baseBlock=5`, queues `GainBlockAction`, and has `triggerOnExhaust` energy
  refunds of 2/3 for base/upgraded forms; local play uses the generic block
  queue and local exhaust hooks grant the same energy amounts. Checks:
  `cargo fmt --check` passed; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the Good Instincts/Good Instincts+
  audit rows after source/local re-check showed the simulator already matched
  decompiled `GoodInstincts.java`. Source Good Instincts is a cost 0
  self-targeting Skill with `baseBlock=6`, queues `GainBlockAction`, and
  upgrades block by 3; the artifact now records base/upgraded local behavior as
  6/9 Block via the generic block queue instead of the stale definition-only
  fallback wording. Checks: `cargo fmt --check` passed; active live-regression
  manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the Dramatic Entrance audit row
  after source/local re-check showed the simulator's implemented base form
  matched decompiled `DramaticEntrance.java`. Source Dramatic Entrance is cost
  0, all-enemy Attack, `baseDamage=8`, `isMultiDamage=true`, Innate, Exhaust,
  and queues `DamageAllEnemiesAction`; the artifact now records that exact
  source/local base behavior instead of a broad shared-path note about Reaper
  and Immolate. The row also records the remaining blocker that source has an
  upgraded +4 damage form while the local implemented-card list has no separate
  Dramatic Entrance+ content id. Checks: `cargo fmt --check` passed; active
  live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`;
  `cargo clippy` passed with existing warnings after setting `PYO3_PYTHON` to
  the bundled Python. Full `cargo test` still fails only in the pre-existing
  stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest live trace fidelity fix: implemented generic Face Trader support after
  `trace-2026-07-02T22-41-59-925Z.jsonl` showed the simulator entering Scrap
  Ooze while the real game entered Face Trader. Face Trader is now an Act 1/2
  shrine-special candidate in source order, has Continue/Touch/Trade/Leave
  event flow, source-backed Touch gold/damage, deterministic Trade face-relic
  selection via `miscRng.randomLong()` plus Java shuffle, and all five face
  relics are represented/imported in core, verifier, and Python replay
  snapshots. Checks: `cargo fmt` passed; `cargo check -p sts_core --lib`
  passed; `cargo check -p sts_verify --lib` passed;
  `uv run maturin develop --release` passed; strict replay of
  `verification/corpus/communication_mod/trace-2026-07-02T22-41-59-925Z.jsonl`
  passed to trace exhaustion (`verified True`, 19 steps, final phase combat).
  `uv run python -m unittest python.tests.test_self_play` is currently blocked
  by an existing import error for missing `_visible_combat_hand` in
  `sts.self_play`, not by this Face Trader change.
- Card-fidelity audit follow-up: corrected the Twin Strike/Twin Strike+ audit
  rows after source/local re-check showed the simulator already matched
  decompiled `TwinStrike.java`. Source Twin Strike is cost 1, targets one enemy,
  deals 5 damage twice, and upgrades damage by 2; the persistent audit artifact
  now records selected-enemy double hits for base/upgraded forms instead of the
  stale generic random-target repeated-hit description. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed; active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`.
  `cargo clippy` is currently blocked by unrelated unstaged
  `simulator/crates/sts_core/src/combat/transition.rs` edits that call missing
  `apply_hand_card_play_triggers`. Full `cargo test` still fails only in the
  pre-existing stale `milestone6` monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Card-fidelity audit follow-up: corrected the base Pummel audit row after
  source/local re-check showed the simulator already matched decompiled
  `Pummel.java`. Source Pummel sets `baseDamage=2`, `exhaust=true`, and
  `magicNumber=4`, then queues four selected-enemy damage hits; the persistent
  audit artifact now records that exact source/local behavior instead of the
  stale generic random-target repeated-hit description. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed; `cargo clippy` passed with existing warnings after setting
  `PYO3_PYTHON` to the bundled Python; active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` still fails only in the pre-existing stale `milestone6`
  monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest card-fidelity audit follow-up: corrected the Pummel+ audit row after
  source/local re-check showed the simulator already matched decompiled
  `Pummel.java`. Source Pummel sets `baseDamage=2`, `exhaust=true`, and
  `magicNumber=4`; Pummel+ upgrades only the hit count to 5. The persistent
  audit artifact now records Pummel+ as damage 2, Exhaust, five selected-enemy
  hits, instead of the stale no-damage/no-exhaust row. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed; `cargo clippy` passed with existing warnings after setting
  `PYO3_PYTHON` to the bundled Python; active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` still fails only in the pre-existing stale `milestone6`
  monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest card-fidelity audit follow-up: fixed a generic Wound metadata
  mismatch found by comparing local status-card values against decompiled
  source. Decompiled `Wound.java` is an unplayable Status with
  `CardTarget.NONE`, empty `use`, and no base damage; local `WOUND` no longer
  carries synthetic `damage: Some(2)`. The audit artifact records the
  source/database/local facts and fixed verdict. Checks: `cargo fmt --check`
  passed; `cargo test -p sts_core --test card_fidelity` passed; `cargo clippy`
  passed with existing warnings after setting `PYO3_PYTHON` to the bundled
  Python; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` still fails only in the pre-existing stale `milestone6`
  monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest card-fidelity audit follow-up: fixed a generic Sword Boomerang
  definition mismatch found by comparing local targets against decompiled
  constructors. Decompiled `SwordBoomerang.java` uses `CardTarget.ALL_ENEMY`,
  `baseDamage=3`, and queues `AttackDamageRandomEnemyAction` 3 times, with
  Sword Boomerang+ upgrading the hit count to 4; local base and upgraded
  definitions now use `TargetRequirement::AllEnemies` while retaining the
  random-living-enemy effect path. The audit artifact records the source,
  database, local facts, and fixed verdict for both forms. Checks:
  `cargo fmt --check` passed; `cargo test -p sts_core --test card_fidelity`
  passed; `cargo clippy` passed with existing warnings after setting
  `PYO3_PYTHON` to the bundled Python; active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` still fails only in the pre-existing stale `milestone6`
  monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest card-fidelity audit follow-up: fixed a generic Slimed mismatch found
  by comparing local card definitions/effects against decompiled constructors.
  Decompiled `Slimed.java` is cost 1 Status, `CardTarget.SELF`, `exhaust=true`,
  and has an empty `use`; local Slimed now maps that to no monster target,
  no damage value, spend 1 energy, and move only the played card to exhaust
  without generating an extra Slimed in discard. The audit artifact records the
  source/database/local facts and fixed verdict. Checks: `cargo fmt --check`
  passed; `cargo test -p sts_core --test card_fidelity` passed; `cargo clippy`
  passed with existing warnings after setting `PYO3_PYTHON` to the bundled
  Python; active live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` is currently blocked before execution by unrelated
  exhaustiveness errors in the dirty `simulator/crates/sts_core/src/relic/mod.rs`
  / run-state relic mapping for `CultistMask`, `FaceOfCleric`, `GremlinMask`,
  `NlothsMask`, and `SsserpentHead`.
- Latest card-fidelity audit follow-up: fixed a generic Thunderclap definition
  mismatch found while sharpening the audit artifact. Decompiled
  `ThunderClap.java` uses `CardTarget.ALL_ENEMY` for base Thunderclap, matching
  Thunderclap+ and the printed `sts.gg /v1/cards` behavior; local
  `THUNDERCLAP` now uses `TargetRequirement::AllEnemies`, with a regression
  covering both forms. The audit artifact also corrects inert curse local facts
  that were over-reporting zero damage/exhaust and adds a source-backed Pain
  trigger note. Checks: `cargo fmt --check` passed;
  `cargo test -p sts_core --test m32a_matrix thunderclap_definitions_target_all_enemies`
  passed; `cargo clippy` passed with existing warnings after setting
  `PYO3_PYTHON` to the bundled Python; active live-regression manifest replay
  passed via `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` still fails only in the pre-existing stale `milestone6`
  monster fixture expectations:
  `acid_slime_combat_executes_weak_attack_cycle`,
  `gremlin_nob_fixture_has_expected_hp_and_opening_intent`,
  `gremlin_nob_enrage_applies_anger_when_player_plays_skill`,
  `gremlin_nob_enrage_bonus_is_applied_once_to_next_attack`,
  `slime_boss_fixture_has_expected_hp_and_slam_intent`,
  `slime_boss_splits_into_acid_slimes_at_half_hp`, and
  `spike_slime_combat_executes_spit_lick_cycle`.
- Latest bridge-control fix: TCP command guarding no longer wedges forever
  after commands such as `CANCEL` that may leave the game on the same visible
  state and time out waiting for a state-sequence advance. The trace client now
  clears its in-memory in-flight marker after an observed-update timeout when
  no queued command remains, and a regression covers accepting a follow-up
  command at the same state. Check: `node tools\communication\trace_client.test.js`.
- Latest card-fidelity audit slice: added
  `simulator/docs/audit/card_fidelity_audit.md`, covering all 242 local
  `ALL_CARDS` entries with separate base/upgraded rows, decompiled-source path
  facts, trusted `sts.gg /v1/cards` database facts, local simulator
  definition/effect facts, verdicts, and fix/blocker notes. This pass found no
  new high-confidence generic card mismatch and made no simulator behavior
  changes. Checks: `cargo fmt --check` passed; `cargo clippy` passed with
  existing warnings after setting `PYO3_PYTHON` to the bundled Python; active
  live-regression manifest replay passed via
  `uv run python -m unittest python.tests.test_live_regression_traces`.
  Full `cargo test` still has pre-existing stale `milestone6` fixture failures
  around source-backed monster RNG/intent behavior and should not be treated as
  introduced by this documentation-only audit.
- Latest active trace replay follow-up: the extended
  `trace-2026-07-02T20-50-14-856Z.jsonl` now strict-replays to exhaustion
  (`verified=True`, `steps=122`, `final_phase=combat`). Generic fix:
  unupgraded Bloodletting now grants the source-backed 2 energy while
  Bloodletting+ grants 3, fixing the floor-10 Gremlin Nob turn-3 energy
  mismatch. Checks: `cargo fmt`, `cargo check -p sts_core --lib`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`, and
  strict replay of the active trace. UI restarted at `http://127.0.0.1:8799/`.
- Latest active trace replay follow-up: the extended
  `trace-2026-07-02T20-50-14-856Z.jsonl` now strict-replays to exhaustion
  (`verified=True`, `steps=97`, `final_phase=combat`). Generic fix: Mind
  Blast now uses the source `AbstractDungeon.player.drawPile.size()` damage
  formula instead of counting all combat piles, fixing the floor-8 generated
  Mind Blast damage mismatch. Checks: `cargo fmt`,
  `cargo check -p sts_core --lib`, `cargo check -p sts_verify --lib`,
  `uv run maturin develop --release`, and strict replay of the active trace.
  UI restarted at `http://127.0.0.1:8799/`.
- Latest active trace replay follow-up: the extended
  `trace-2026-07-02T20-50-14-856Z.jsonl` now strict-replays to exhaustion
  (`verified=True`, `steps=75`, `final_phase=combat`). Generic fix: large
  Acid Slime split no longer spends a post-split AI roll for the dead parent;
  decompiled `AcidSlime_L.takeTurn` spawns the two medium slimes and sets the
  parent's split move without enqueuing `RollMoveAction`, unlike large Spike
  Slime. Checks: `cargo fmt`, `cargo check -p sts_core --lib`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`, and
  strict replay of the active trace. UI restarted at `http://127.0.0.1:8799/`.
- Latest active trace replay follow-up: the extended
  `trace-2026-07-02T20-50-14-856Z.jsonl` now strict-replays through the next
  turn (`verified=True`, `steps=30`, `final_phase=combat`). Generic fix:
  after modeling Shame's separate end-turn autoplay/discard trigger, the normal
  end-turn hand discard no longer needs the prior shuffle exception and now
  always follows source top-of-hand discard order. Checks: `cargo fmt`,
  `cargo check -p sts_core --lib`, `cargo check -p sts_verify --lib`,
  `uv run maturin develop --release`, and strict replay of the active trace.
- Latest active trace replay slice: the extended
  `trace-2026-07-02T20-50-14-856Z.jsonl` now strict-replays to exhaustion
  (`verified=True`, `steps=29`, `final_phase=combat`). Generic fix: Shame now
  follows the source curse end-turn path, applying Frail and moving itself to
  discard before the normal end-turn hand discard batch, which fixes the live
  floor-2 discard-pile order diff. Checks: `cargo fmt`,
  `cargo check -p sts_core --lib`, `cargo check -p sts_verify --lib`,
  `uv run maturin develop --release`, and strict replay of the active trace.
- Latest representation-boundary slice: CommunicationMod combat pile order now
  has an explicit design note and named verifier/replay helpers for observed
  pile import, simulator-to-visible projection, and bridge `PLAY n` hand-slot
  mapping. This is behavior-preserving groundwork for removing pile-order
  leakage from `sts_core`. Checks: `cargo fmt`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`, and
  strict replay of `trace-2026-07-02T20-50-14-856Z.jsonl`
  (`verified=True`, `steps=12`).
- Latest live trace replay follow-up: the extended
  `trace-2026-07-02T20-50-14-856Z.jsonl` now strict-replays to exhaustion
  (`verified=True`, `steps=12`, `final_phase=combat`). Generic fix: end-turn
  hand discard now treats CommunicationMod hand order as a bridge-facing order;
  stable visible discard piles use source top-of-hand order, while hands that
  immediately enter a discard reshuffle preserve the bridge order needed for
  source-shaped shuffle parity. Checks: `cargo fmt`,
  `cargo check -p sts_core --lib`, `cargo check -p sts_verify --lib`,
  `uv run maturin develop --release`, and strict replay of the trace.
- Latest live trace replay slice: `trace-2026-07-02T20-50-14-856Z.jsonl`
  now strict-replays to exhaustion (`verified=True`, `steps=11`,
  `final_phase=combat`). Generic fix: end-turn hand discard now preserves the
  simulator's bridge-facing hand order before discard-pile reshuffles, matching
  the source `DiscardAction`/`EmptyDeckShuffleAction` behavior for observed hand
  ordering. Checks: `cargo fmt`, `cargo check -p sts_core --lib`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`, and
  strict replay of the trace.
- Latest live trace replay slice: newest trace
  `trace-2026-07-02T19-54-35-294Z.jsonl` now advances past the floor-5 Blue
  Slaver AI mismatch, floor-7 Colorless Potion reward mismatch, and floor-12
  Living Wall grid flow. Generic fixes: Blue Slaver encounter spawns no longer
  pre-lock opening Stab and instead consume source `aiRng`; combat entry now
  preserves post-opening monster AI RNG; Colorless Potion/Discovery combat
  colorless choices use the source-shaped non-healing `srcColorlessCardPool`
  order reconstructed from decompiled `CardLibrary`/`AbstractDungeon`; observed
  combat card reward comparison maps implemented colorless names to content
  ids; and Living Wall supports Forget/Change/Grow grids plus the target leave
  screen. Checks: `cargo fmt`, `cargo check -p sts_core --lib`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`, and
  active strict replay. Current next blocker is floor 13 at trace step 237:
  simulator spawned/advanced an Acid Slime M intent where observed has Spike
  Slime M `DEBUFF` alongside Looter.
- Latest live trace fidelity slice: potion kills now enter the same
  room-kind-aware combat victory rewards as card/end-turn kills and apply
  Burning Blood/Black Blood victory healing before opening rewards, so boss
  kills from Explosive Potion produce boss gold/reward timing. Spike Slime
  medium/large `DEBUFF` import and entry intent now preserve the source-backed
  Frail duration hidden behind CommunicationMod's generic `DEBUFF` label, and
  the verifier intent display mapper covers Constricted. Checks:
  `cargo fmt`, `cargo check -p sts_core --lib`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`.
  Active trace `trace-2026-07-02T09-19-40-178Z.jsonl` now advances to a later
  floor-13 Jaw Worm/Spike Slime AI RNG mismatch; newest live trace
  `trace-2026-07-02T19-54-35-294Z.jsonl` currently exposes a separate Blue
  Slaver intent blocker.
- Latest active trace replay slice: `trace-2026-07-02T09-19-40-178Z.jsonl`
  now strict-replays to exhaustion (`verified=True`, `steps=211`,
  `final_phase=combat`) after the floor-13 and Slime Boss blockers. Generic
  fixes: implemented source-backed executable `Exordium Wildlife` spawn
  generation, including constructor-order HP/misc RNG consumption; Slime Boss
  split now gives the Acid Slime L child its source normal-tackle starting
  intent; large Acid Slime split insertion mirrors target smart-positioning for
  Slime Boss child splits; and observed Spike Slime M/L `DEBUFF` import uses
  CommunicationMod monster ids instead of max HP so low-HP split children still
  import as Frail. Checks: `cargo fmt`, `cargo check -p sts_core --lib`,
  `cargo check -p sts_verify --lib`, `uv run maturin develop --release`, and
  active strict replay.
- Latest active trace replay slice: `trace-2026-07-02T09-19-40-178Z.jsonl`
  now advances from the floor-10 Bottled Lightning opening-hand mismatch and
  the floor-11 large Spike Slime/split blockers to a later floor-13 encounter
  mismatch. Generic fixes: bottle relic grids now mirror STS
  `getCardsOfType()` ordering; large Spike Slime receives entry AI rolls like
  small/medium Spike Slimes; large slime split spawn RNG follows queued action
  order (left child, right child, dead parent roll); and large Spike Slime Lick
  applies its source-backed larger Frail amount while preserving the visible
  generic debuff intent. Checks: `cargo fmt`, `cargo check -p sts_core --lib`,
  `uv run maturin develop --release`, and active strict replay. Current next
  blocker is floor 13 at trace step 165, where simulator encounter state has
  one monster but observed has two.
- Latest broad unit-test cleanup slice: broad inline `sts_core` mechanics unit
  tests have been removed in favor of trace-driven fidelity. Remaining
  `sts_core` unit tests are limited to small infrastructure/determinism surfaces
  (`rng`, `snapshot`, `ids`, and `error`). CommunicationMod corpus/live replay
  tests and Python UI/bridge infrastructure tests remain the intended
  regression gates.
- Latest simulator test-policy cleanup slice: agent rules now make
  CommunicationMod trace replay the preferred regression surface for real-game
  simulator fidelity bugs. Narrow unit tests are reserved for infrastructure,
  serialization/determinism, parsers/mappers, or tiny source-backed rules that
  traces cannot isolate cleanly. The Jaw Worm secondary-`aiRng` fix is covered
  by active strict replay advancement rather than an added mechanic unit test:
  `trace-2026-07-02T09-19-40-178Z.jsonl` now clears the floor-5 Jaw Worm
  intent blocker and reaches the later step-107 hand/draw-pile mismatch.
- Latest live trace display/replay slice: the UI replay summary no longer says
  `Verified` while a pending simulator/live prediction mismatch is active; it
  now surfaces `Prediction mismatch` until acknowledged/re-attached. The newest
  live trace `trace-2026-07-02T09-19-40-178Z.jsonl` strict-replays cleanly
  through its current end after generic fixes for Fairy potion name
  canonicalization, id-less visible-choice event import for Shining Light/Wing
  Statue, and Shining Light's real post-enter leave screen. Checks:
  `cargo fmt`, `cargo test -p sts_core shining_light --lib`,
  `cargo test -p sts_verify observed_event_screen_imports --lib`,
  focused Python Fairy normalization unittest,
  `uv run maturin develop --release`, and strict replay of the newest live
  trace.
- Latest live-regression corpus slice: the clean current live trace has been
  snapshotted into the persistent verification corpus as
  `communication_mod/live-regression-2026-07-01T20-30-26-163Z.jsonl`, and a
  manifest-driven Python test now strict-replays every
  `verification/corpus/live_regressions.json` entry. This keeps live fidelity
  regressions under automatic testing without seed-specific simulator behavior.
- Latest simulator developer-loop tooling slice: `simulator/scripts/dev-verify.ps1`
  now wraps the common local fidelity loop (`cargo fmt`, focused Rust test
  filters, `uv run cargo clippy`, `uv run maturin develop --release`, and
  optional strict replay checks), while `simulator/scripts/restart-ui.ps1`
  restarts only local `sts.ui_service` Python/uv processes and waits for
  `http://127.0.0.1:8799/`. These scripts are generic command wrappers; they do
  not bake in trace names, seeds, or simulator behavior.
- Latest live-bridge mapper slice: single-target bridge potion actions may omit
  an explicit `target_slot` when only one legal monster target is visible. The
  UI service maps simulator `UsePotion` recommendations to that command only
  when the simulator target resolves to that sole visible target slot, appends
  the inferred target to the bridge command, and focused Python tests cover both
  the accepted single-target case and the multi-target rejection case.

### Combat
- Latest Reptomancer RNG slice: Reptomancer now uses a source-shaped
  post-opening move helper instead of the representative modulo cycle. Empty
  history fixes Spawn Dagger; later low rolls recurse with
  `aiRng.random(33, 99)` when Snake Strike is blocked, mid rolls respect
  `canSpawn` and last-two Spawn history, and high rolls recurse with
  `aiRng.random(65)` when Big Bite is blocked. Combat entry and turn prep route
  through that helper, and focused coverage pins first move, can-spawn fallback,
  and both replacement counters. Remaining Reptomancer work is summon
  slot/action ordering, death cleanup, and Snake Strike's attack+Weak execution
  surface.
- Latest Darkling RNG slice: half-dead Darklings now keep the local
  non-targetable marker while following the source Count/Reincarnate turn
  shape. Count records source move byte `4`, consumes one normal ignored AI
  roll, and prepares Reincarnate byte `5`; Reincarnate revives to half max HP,
  clears the half-dead marker, and consumes the next normal roll for the
  following move. Focused coverage pins both transitions and AI counters.
  Remaining Darkling work is exact all-Darklings-dead room resolution, Regrow
  power visibility, and broader multi-Darkling trace validation.
- Latest Gremlin Leader RNG slice: recursive move replacements now consume the
  source AI ranges instead of representative fixed rolls. With one living
  gremlin, a blocked low-branch Rally consumes `aiRng.random(50, 99)`, and a
  blocked high-branch Stab consumes `aiRng.random(0, 80)`. Combat entry and
  turn prep pass the live AI stream into those replacements, and focused
  coverage pins both counters. Remaining Gremlin Leader work is summon
  identity/slot/action ordering, Encourage quote draw placement, and minion
  death/escape trace validation.
- Latest Byrd RNG slice: grounded Byrd turn prep now consumes the normal
  `RollMoveAction` AI integer, ignores it, and fixes Headbutt. Headbutt now
  directly sets Go Airborne without another AI roll; Go Airborne records source
  move byte `2` and reapplies Flight instead of adding Strength. Focused
  coverage pins the grounded prep and direct Headbutt transition. Remaining
  Byrd work is trace-backed multi-Byrd and damage-triggered Stun/Flight
  validation.
- Latest Spike Slime M/L RNG slice: collapsed medium/large Spike Slime
  handling now uses source Frail amounts and max-HP identity for large
  attack/count/Frail adaptation. Large Spike Slime split children now use
  current split HP as max HP, matching the target child constructor. Focused
  coverage pins sub-A17 versus A17 history guards and split-child max HP.
- Latest large Acid Slime RNG slice: the large Acid Slime helper now uses
  source move history instead of only previous intent, preserving the
  decompiled last-two Wound/Normal Tackle guards and their replacement
  `aiRng` boolean draws. Entry, turn-prep, and split follow-up call sites pass
  history. Remaining large-slime work is exact split child action ordering and
  trace-backed validation.
- Latest louse RNG slice: mixed `Exordium Thugs` and `Exordium Wildlife`
  helpers now attach rolled Curl Up when the weak slot selects a louse, with the
  `monsterHpRng` draw delayed until after the source candidate
  constructor/private HP draws. Focused coverage pins the hidden Curl Up amount
  for both mixed helpers. Remaining louse work is trace-backed action-order and
  state-import validation.
- Latest Jaw Worm RNG slice: focused coverage now proves the source helper's
  replacement draw counts. Unguarded threshold branches consume no extra AI
  draw, while the last-Chomp, last-two-Thrash, and last-Bellow guarded branches
  each consume exactly one replacement `aiRng.randomBoolean()` with the
  decompiled probabilities. Remaining Jaw Worm work is horde setup and broader
  trace-backed action ordering.
- Latest small Acid Slime RNG slice: combat entry now matches the decompiled
  ascension split. Small Acid Slime still consumes the normal ignored
  `RollMoveAction` integer on entry; below A17 it also consumes the source
  `aiRng.randomBoolean()` for Tackle versus Weak, while A17+ opens Weak without
  that extra boolean. Focused coverage pins the A16 two-draw and A17 one-draw
  counter difference.
- Latest Fungi Beast RNG slice: corrected the audit's stale A17 Artifact note;
  decompiled Fungi Beast applies Spore Cloud 2 pre-battle, not Artifact. Focused
  coverage now pins the `num < 60` Bite/Grow table, A17 Grow +1, Spore Cloud
  setup without Artifact, and Spore Cloud release only when combat is not
  ending. Remaining work is trace-backed action-order and broader HP/routing
  validation.
- Latest Centurion/Mystic RNG slice: Centurion Protect now mirrors target
  `GainBlockRandomMonsterAction` stream shape by consuming combat AI RNG to
  choose a valid non-source ally before the normal post-turn `RollMoveAction`
  roll. Focused coverage also pins Centurion Protect/Fury/Slash history and
  Mystic's missing-HP heal threshold, A17 history changes, attack+Frail, and
  Strength-all fallback. Remaining work is trace-backed pair action-order,
  heal-cap, death, and escape validation.
- Latest Snake Plant RNG slice: Snake Plant's decompiled one-roll table and
  effect surface now have focused coverage. `num < 65` selects Chompy Chomps
  unless the last two moves were attacks; high rolls select Spores unless
  history blocks Spores, with A17+ checking the last or previous move. Local
  setup starts Malleable at 3, and Spores applies Frail 2 plus Weak 2. Remaining
  work is trace-backed action-order validation.
- Latest Chosen RNG slice: Chosen's decompiled move table now has focused
  helper coverage. A17+ opens with Hex after the normal ignored
  `RollMoveAction` integer; below A17 opens with two-hit Poke, then Hex once;
  later turns use the source Debilitate/Drain threshold, Debilitate/Drain
  history guard, and Zap/Poke fallback. Remaining work is trace-backed
  action-order validation.
- Latest Snecko RNG slice: Snecko's decompiled move table now has focused
  helper coverage. The fixed opening Glare/Confusion consumes the normal
  ignored `RollMoveAction` integer, later turns use `num < 40` for Tail
  attack+debuff, high rolls Bite unless the last two moves were Bite, and A17
  adds Weak to Tail; remaining work is trace-backed action-order/card-cost
  validation.
- Latest small Spike Slime RNG slice: small Spike Slime now has source-backed
  fixed-attack coverage. Combat entry consumes the normal ignored
  `RollMoveAction` AI integer, the roll value is ignored, and the HP-collapsed
  local small branch always opens with Attack; remaining work is trace-backed
  action-order, poison, and split-routed small slime validation.
- Latest Red Slaver RNG slice: Red Slaver's opening `firstTurn` guard now
  matches the decompiled class. Combat entry consumes the normal ignored
  `RollMoveAction` AI integer, but empty move history always opens with Stab
  even on high rolls that can select Entangle later; later turns retain the
  source Entangle/Scrape/Stab roll table.
- Latest Gremlin Nob RNG slice: Gremlin Nob's A18+ move helper now follows
  the decompiled history-only branch after the normal ignored
  `RollMoveAction` AI roll. A18+ ignores the roll value, prefers Skull Bash
  unless Skull Bash appears in the prior two move slots, uses Rush unless the
  last two moves were Rush, and then forces Skull Bash; sub-A18 keeps the
  source `num < 33` Skull Bash branch.
- Latest Lagavulin RNG slice: sleeping Lagavulin now follows the decompiled
  direct-transition timing. Initial sleep and the first two idle sleep turns
  consume ignored `RollMoveAction` AI integers, the third natural wake
  direct-sets the attack without an extra AI roll, damage-wake Stun still
  consumes one ignored roll before attack, and source move bytes `5/4/3/1`
  are recorded.
- Latest Sentry RNG slice: Sentry now mirrors the decompiled fixed move
  surface while preserving roll timing. Combat entry and turn prep consume one
  ignored `RollMoveAction` AI integer, the first move uses group index
  parity (even Bolt/Dazed, odd Beam/attack), and later moves alternate from
  source move bytes `3` and `4`.
- Latest Gremlin Tsundere RNG slice: Shield Gremlin Protect now follows the
  decompiled `GainBlockRandomMonsterAction` stream shape. The block target is
  chosen from non-source, non-escaping, non-dying monsters with combat
  `aiRng`, including the one-candidate case; when no valid target exists it
  blocks itself. After Protect, Tsundere direct-sets the next Protect/Bash
  move without an extra post-turn `RollMoveAction` integer.
- Latest Shelled Parasite RNG slice: Shelled Parasite's first move now follows
  the source stream shape: below A17 it consumes the normal ignored
  `RollMoveAction` integer plus a source `aiRng.randomBoolean()` for Double
  Strike versus Life Suck, while A17+ fixes Fell after only the normal roll.
- Latest Book of Stabbing RNG/state slice: Book of Stabbing now stores the
  source hidden `stabCount`, initializes it to 1, mutates it during combat
  entry and turn-prep intent selection, and preserves the A18 rule where Big
  Stab also increments the hidden count. Book move history now records source
  bytes `1` for multi-stab and `2` for Big Stab.
- Latest Bronze Automaton RNG/cycle slice: Bronze Automaton now has a
  source-shaped history-aware boss cycle with source move bytes `4/1/5/2/3`,
  A9 Boost block 12, A4 Boost strength 4, and the A19 post-Hyper-Beam Boost
  branch instead of Stun. Turn prep consumes the normal ignored
  `RollMoveAction` AI integer while routing through that helper.
- Latest Spiker RNG/execution slice: Spiker now initializes source Thorns,
  including the A17 +3 bonus, tracks the hidden thorns-buff count separately
  from total Thorns, buffs by exactly 2 per buff move, and forces attacks after
  more than five thorns buffs.
- Latest Exploder RNG/execution slice: Exploder now carries its source
  Explosive(3) monster power in combat state, and its third-turn Unknown move
  deals 3 damage, clears the power, kills the monster, clears block, and skips
  follow-up intent preparation/post-death AI rolls.
- Latest Dagger RNG/execution slice: Snake Dagger/Dagger Explode now attacks
  for 25, loses all current HP, clears block, dies, and skips follow-up intent
  preparation so it does not consume an extra post-death AI roll.
- Latest Nemesis RNG slice: Nemesis now uses a decompiled-source helper for
  first move, Scythe/Burn/Tri-Attack thresholds, replacement
  `aiRng.randomBoolean()` draws, source move bytes `2/3/4`, fixed A8 HP, A18
  Burn count, and post-turn monster Intangible damage capping. Exact
  Intangible decrement/action timing still needs trace validation.
- Latest Giant Head RNG slice: Giant Head now uses the decompiled one-roll
  countdown table with A18 shortened setup, Glare/Count history guards, It Is
  Time damage ramp/cap, source move bytes `1/2/3`, fixed A8 HP, and a Slow
  setup marker. Slow's per-card damage amplification remains a later combat
  damage hook.
- Latest Spire Growth RNG slice: Spire Growth now uses the decompiled one-roll
  move table with A17 Constrict opener, Quick Tackle/Smash history guards,
  source move bytes `1/2/3`, fixed A7 HP, and a local Constricted player power
  that applies end-of-player-turn HP loss.
- Latest Maw RNG slice: The Maw now uses the decompiled one-roll move table:
  fixed opening Roar with A17 Weak/Frail scaling, Nom no-repeat plus
  turn-count hit scaling, Drool Strength scaling, Slam damage scaling, and
  source move bytes `2/3/4/5`.
- Latest Spheric Guardian RNG slice: Spheric Guardian setup and turn prep now
  mirror the decompiled class shape: Barricade, Artifact 3, 40 starting block,
  fixed activate/frail openers, normal ignored `RollMoveAction` AI draw after
  turns, and source move bytes `2/4/1/3` for the big/harden alternation.
- Latest Gremlin Wizard RNG slice: Gremlin Wizard post-turn prep now follows
  the decompiled direct `setMove` cycle without consuming `aiRng`: charge,
  charge, blast, then repeat below A17; at A17+ the Wizard keeps blasting after
  the first blast. Focused coverage pins the A0/A17 cycle, source move bytes,
  and zero post-turn monster RNG counter movement. Remaining work is exact
  escape/death-react behavior and trace validation.
- Latest Exploder RNG slice: Exploder now has a source-shaped ignored-roll
  countdown helper. Combat entry and turn prep consume the normal
  `RollMoveAction` AI integer but ignore its value, producing two attacks then
  the source Unknown/no-op move byte `2` instead of the old representative
  parity alternation. Focused coverage pins A2 damage, the two-attack countdown,
  move byte, and combat-entry helper routing. Remaining work is executable
  Explosive power/death timing and trace validation.
- Latest Repulsor RNG slice: Repulsor now has a source-style one-roll
  `getMove(int num)` helper: Attack only on `num < 20` when the previous move
  was not Attack, otherwise add two Dazed to the draw pile. Combat entry and
  turn prep route Repulsor through that helper instead of the representative
  alternating fallback, and focused coverage pins the threshold, no-repeat
  guard, move byte, and entry helper routing. Remaining work is trace-backed
  validation of random draw-pile insertion ordering in multi-shape fights.
- Latest Looter/Mugger RNG slice: Looter and Mugger post-attack move prep now
  follows the decompiled direct `SetMoveAction` path instead of a normal
  `RollMoveAction` roll table. Looter consumes the source 0.6 speech boolean
  after first Mug and 0.5 Smoke/Lunge boolean after second Mug. Mugger consumes
  source attack voice `aiRng.random(2)` draws plus the second-Mug 0.6 speech and
  0.5 Smoke/Big Swipe booleans. Focused tests cover the no-`random(99)` stream
  shape for first and second Mug transitions; remaining work is stolen-gold,
  escape/reward, Mugger death-voice RNG, and trace validation.
- Latest Donu/Deca RNG slice: Beyond Act 3 boss selection now follows the
  source boss-list shuffle and can construct the Donu/Deca pair instead of a
  generic fixture. The pair helper builds Deca then Donu with source fixed
  250/265 HP, A19 Artifact 3, Deca opening Beam after one ignored `aiRng` roll,
  Donu opening Circle/strength after one ignored `aiRng` roll, and source move
  bytes `0`/`2`. Focused coverage pins A19 pair state, opening intents, move
  bytes, Donu's second Beam intent, and Deca Square execution as all-living
  block plus A19 Plated Armor. Remaining work is exact Dazed insertion,
  strength action ordering, and trace coverage.
- Latest Transient RNG slice: Transient combat entry is source-locked with no
  opening AI roll, A4+ opening damage now starts at 40, and post-turn intent
  prep directly sets the escalating attack without consuming `aiRng`, matching
  the decompiled class's lack of `RollMoveAction`. Focused tests cover A4 spawn
  damage and zero post-turn AI counter movement.
- Latest Torch Head RNG slice: Collector-spawned Torch Heads now consume the
  source HP stream shape, with constructor HP always rolled from `38..40` and
  source `setHp` rolling `38..40` or A9+ `40..45`. Focused coverage pins two
  spawned Torch Heads at A9 with four HP draws, two ignored spawn-init `aiRng`
  rolls, fixed attack intents, and move-history byte `1`.
- Latest Taskmaster RNG slice: Taskmaster City member spawn now has focused
  coverage for the source constructor HP roll plus `setHp` roll, and combat
  entry routes Taskmaster through a fixed Scouring Whip intent that consumes
  one normal opening `aiRng` roll while ignoring its value. Taskmaster move
  history now records source byte `2`; wound count and A18 Strength remain
  handled in the execution path.
- Latest Reptomancer/Dagger RNG slice: target Reptomancer encounter import now
  builds the source `Dagger`, `Reptomancer`, `Dagger` group and consumes HP in
  target constructor order: Dagger HP, Reptomancer constructor HP,
  Reptomancer `setHp`, Dagger HP. Reptomancer's fixed first Spawn Dagger move
  now consumes the normal opening `aiRng` roll, Reptomancer Spawn Dagger uses a
  Reptomancer-specific Dagger spawn path instead of Gremlin Leader summons, and
  spawned Daggers consume source HP/opening AI rolls with fixed Wound first
  moves recorded. Focused tests cover group HP order and A18 two-Dagger spawn
  roll counters/slots. Remaining work includes Reptomancer's recursive later
  move table, exact `canSpawn` edge cases, and Dagger explode/death behavior.
- Latest Bronze Automaton orb RNG slice: Bronze Automaton-spawned Bronze Orbs
  now consume the live `monsterHpRng` constructor roll plus source `setHp` roll
  per orb, consume one opening combat `aiRng` roll per orb, and route that roll
  through the Bronze Orb source-style helper. Focused coverage pins A9 HP
  double-roll consumption, opening AI roll counters, insertion order, minion
  state, and move-history recording. Remaining monster RNG audit work still
  includes Bronze Orb Stasis selection proof, recursive reroll monsters, and
  broader constructor/private HP draws.
- Latest monster RNG helper-routing slice: combat-entry and turn-prep AI now
  use source-style helpers for Red Slaver, Snecko, Book of Stabbing, Bronze
  Orb, and Orb Walker. Book of Stabbing and Orb Walker gained compact
  decompiled-`getMove` helpers, and focused tests pin that this batch consumes
  exactly one combat `aiRng` roll at entry and routes through those helpers.
  Remaining monster RNG audit work is still broad: exact Book private
  `stabCount` persistence, Bronze Automaton/orb spawn details, Bronze Orb
  Stasis selection proof, recursive reroll monsters, constructor/private
  `monsterHpRng` draws, and `MonsterHelper` composition parity.
- Latest monster RNG audit slice: combat entry now documents local
  `CombatState.monster_rng` as target combat `aiRng`, stops pre-advancing
  `monsterHpRng` by monster count after source-backed spawn helpers have
  already consumed HP/private constructor rolls, and avoids an extra initial
  AI draw for source-locked spawn intents. Focused regression coverage pins
  locked-intent zero-draw behavior and ordinary unlocked one-roll behavior.
  Current milestone remains A0 strict/live trace parity; next monster-RNG work
  is translating the remaining source-backed first-move/recursive-reroll and
  `MonsterHelper` composition gaps from
  `simulator/docs/audit/monster_rng_decompiled_audit.md`.
- Latest live-trace Life Suck slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=240`,
  `final_phase=combat` after the lethal Shelled Parasite Life Suck mismatch.
  Generic fix: monster pending post-hit effects stop when monster attack damage
  reduces the player to 0 HP, so Life Suck self-heal and thorns reflection do
  not resolve after lethal player damage. Focused tests cover normal Life Suck
  healing, blocked hits, caps, thorns ordering, and the lethal no-heal/no-thorns
  case. Checks: `cargo fmt`,
  `cargo test -p sts_core shelled_parasite_life_suck --lib`,
  `cargo clippy -p sts_core --lib` (existing warnings only),
  `uv run maturin develop --release`, active strict replay, and the protected
  live trace gate confirming older known blockers stayed at the same
  boundaries.
- Latest live-trace Shelled Parasite Fell slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=234`,
  `final_phase=combat` after the floor-19 Shelled Parasite intent mismatch.
  Generic fix: Shelled Parasite Fell is plain attack before A17 and only applies
  Frail at A17+, while target move-id mapping still treats both forms as move 1.
  Focused tests cover the A0/A2 plain attack variants, A17 Frail variant, and a
  blocked Life Suck expectation that no longer assumed unrelated combat-victory
  healing. Checks: `cargo fmt`, `cargo test -p sts_core shelled_parasite --lib`,
  `cargo clippy -p sts_core --lib` (existing warnings only),
  `uv run maturin develop --release`, active strict replay, and the protected
  live trace gate confirming older known blockers stayed at the same
  boundaries. Current milestone remains A0 strict/live trace parity; next task
  is continuing live UI play to the next fresh mismatch or SlayTheData
  run-level decision replay.
- Latest live-trace multi-hit thorns slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=222`,
  `final_phase=reward` after the Act 2 Byrds combat-end HP mismatch. Generic
  fix: monster multi-hit attacks now apply player thorns hit-by-hit, stop
  adding hits once thorns kills the attacker, and carry the effective hit count
  into pending player damage so per-hit damage is not truncated across the
  original hit count. A Byrds-shaped regression pins the low-HP Peck/Bronze
  Scales timing without seed- or trace-specific production code. Checks:
  `cargo fmt`, `cargo test -p sts_core monster_multi_hit --lib`,
  `cargo test -p sts_core byrd_peck_thorns --lib`,
  `cargo clippy -p sts_core --lib` (existing warnings only),
  `uv run maturin develop --release`, active strict replay, and the protected
  live trace gate confirming older known blockers stayed at the same
  boundaries. Current milestone remains A0 strict/live trace parity; next task
  is continuing live UI play to the next fresh mismatch or SlayTheData
  run-level decision replay.
- Latest live-trace Headbutt single-discard slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=214`,
  `final_phase=combat` after the Act 2 Byrds mismatch where CommunicationMod
  returned directly to combat after `PLAY 4 0`. Generic fix: Headbutt now
  resolves immediately when the discard pile has exactly one card after damage,
  putting that card on top of draw and discarding Headbutt without opening a
  discard-select state; multi-card discard piles still open the normal selection
  and lethal Headbutt still skips the draw placement. Checks: `cargo fmt`,
  `cargo test -p sts_core headbutt --lib`, `cargo clippy -p sts_core --lib`
  (existing warnings only), `uv run maturin develop --release`, active strict
  replay, and the protected live trace gate confirming older known blockers
  stayed at the same boundaries. Current milestone remains A0 strict/live trace
  parity; next task is continuing live UI play to the next fresh mismatch or
  SlayTheData run-level decision replay.
- Latest live-trace Hexaghost Inferno/BurnIncrease slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=198`,
  `final_phase=combat` after the later boss-turn HP mismatch. Source-backed
  fix: Hexaghost Inferno now models the target six-hit attack followed by
  `BurnIncreaseAction`, which upgrades Burns in draw/discard and adds three
  upgraded Burns instead of treating Inferno as a single hit that adds normal
  Burns. Observed-state import now distinguishes Hexaghost Sear (`move_id` 4)
  from Inferno (`move_id` 6), and combat card instances preserve observed
  upgrade counts so `Burn+` damage can replay. Checks: `cargo fmt`,
  `cargo test -p sts_core hexaghost --lib`,
  `cargo test -p sts_verify hexaghost --lib`,
  `uv run maturin develop --release`, strict replay of the current live trace,
  and the protected live trace gate confirming older known blockers stayed at
  the same boundaries. Current milestone remains A0 strict/live trace parity;
  next task is the next fresh live mismatch or SlayTheData run-level decision
  replay.
- Latest live-trace Hexaghost orb-cycle slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=185`,
  `final_phase=combat` after the boss-turn mismatch where Hexaghost should have
  Strengthened instead of rolling straight to Inferno. Source-backed fix:
  Hexaghost intent preparation now follows the target `orbActiveCount` cycle
  decoded from the game jar (`Sear, Tackle, Sear, Strengthen, Tackle, Sear,
  Inferno`) after the opening Activate/Divider sequence. The Strengthen move
  applies 12 block and 2 Strength, then rolls the next Tackle intent. Checks:
  `cargo fmt`, `cargo test -p sts_core hexaghost --lib`,
  `uv run maturin develop --release`, strict replay of the current live trace,
  and the protected live trace gate confirming older known blockers stayed at
  the same boundaries. Current milestone remains A0 strict/live trace parity;
  next task is the next fresh live mismatch or SlayTheData run-level decision
  replay.
- Latest live-trace Large Slime visible split slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=138`,
  `final_phase=combat` after the floor-12 split that previously swapped the two
  medium Acid Slime child intents. Source-backed pieces: pending
  `SummonGremlins` split intents are no longer overwritten by small-slime
  follow-up preparation before the monster turn executes, and large Acid Slime
  wound-tackle follow-up no longer ignores the next AI roll. The split helper
  now preserves the visible child slot order while assigning the child AI rolls
  in the order observed after target `SpawnMonsterAction`/`init()` processing.
  Checks: `cargo fmt`, `cargo test -p sts_core large_acid_slime --lib`,
  `cargo test -p sts_core large_slime --lib`, `uv run maturin develop
  --release`, strict replay of the current live trace, and the protected live
  trace gate confirming older known blockers stayed at the same boundaries.
  Current milestone remains A0 strict/live trace parity; next task is the next
  fresh live mismatch or SlayTheData run-level decision replay.
- Latest live bridge mapper slice: combat recommendations that resolve to
  generic select-confirm actions (`confirm_hand_select`, `confirm_draw_select`,
  `confirm_discard_select`, or `confirm_exhaust_select`) now map to the bridge
  `ConfirmChoice` / `CONFIRM` command in both the Python live send path and the
  browser UI mapper. This unblocks Gambler's Brew and other select-confirm
  flows without adding seed- or trace-specific behavior. The mapper also guards
  normal dict-payload actions before confirm matching. Checks:
  `uv run python -m unittest python.tests.test_ui_service.UiServiceTests.test_bridge_action_for_exact_action_maps_select_confirm_to_confirm_choice python.tests.test_ui_service.UiServiceTests.test_bridge_action_for_exact_action_ignores_non_confirm_dict_payloads python.tests.test_ui_service.UiServiceTests.test_bridge_action_for_exact_action_maps_run_play_card_to_visible_slots`,
  strict replay gate over the current protected live traces, and live
  `trace-2026-07-01T20-30-26-163Z.jsonl` advanced cleanly to
  `verified=True`, `stop_reason=trace_exhausted`, `steps=133`,
  `final_phase=combat` after sending `CONFIRM`.
- Latest seed-start verifier regression slice: `sts_verify` is back to a clean
  full library suite after the M33 Neow regression set. Source-backed fixes:
  Neow `ThreeRareCards` now burns the same per-card rarity rolls as
  `NeowReward.getRewardCards(true)` before forcing Rare; the normal curse pool
  follows target `CardLibrary.getCurse()` Java `HashMap` iteration order rather
  than declaration order; seed-start replay now compares CommunicationMod's
  observable Neow queued-effect boundaries for curse/colorless/transform
  branches instead of forcing the final deck state into earlier trace states.
  Verifier normalization also keeps canonical numeric card reward ids, starter
  deck display ids, Calling Bell curse labels, and observed generic Louse names
  aligned with source-backed simulator identity. Checks: `cargo fmt`,
  `cargo test -p sts_core three_rare_cards_burn_rarity_rolls_before_forcing_rare --lib`,
  `cargo test -p sts_core random_curse_pool_uses_target_hash_map_iteration_order --lib`,
  and `cargo test -p sts_verify --lib` (121 passed). Current milestone remains
  A0 strict/live trace parity; next task is the next fresh live mismatch or
  SlayTheData run-level decision replay.
- Latest live-trace Mugger verifier import slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=317`,
  `final_phase=combat`. CommunicationMod reports Mugger's Thievery-backed swipe
  as generic `ATTACK`, matching the decompiled `Mugger.takeTurn` source where
  the attack steals gold through Thievery timing; observed-state import now
  normalizes Mugger swipe the same way it already normalized Looter swipe,
  without any trace- or seed-specific logic. Checks: `cargo fmt`,
  `cargo test -p sts_verify mugger_attack_observed_intent_imports_gold_steal_attack --lib`,
  `uv run maturin develop --release`, and strict replay of the current live
  trace. Current milestone remains A0 strict/live trace parity; next task is
  the next fresh mismatch or SlayTheData run-level decision replay.
- Latest live-trace Act 1 event fidelity slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=227`,
  `final_phase=combat`. Source-backed fixes: Scrap Ooze HP loss now follows
  target scaling (3/4/5... HP, or 5/6/7... at A15+) while preserving its misc
  RNG relic roll, Wing Statue/Golden Wing now implements the decompiled
  `INTRO -> PURGE -> MAP` flow (Pray loses 7 HP, Continue opens the removal
  grid, Confirm returns to Leave; Destroy rolls 50-80 misc RNG gold when a
  10+ base-damage attack exists), and strict replay can import observed Scrap
  Ooze/Wing Statue event screens without seed-specific branches. Checks:
  `cargo fmt`, `cargo test -p sts_core scrap_ooze --lib`,
  `cargo test -p sts_core wing_statue --lib`,
  `cargo test -p sts_verify observed_event_screen_imports_wing_statue --lib`,
  `uv run maturin develop --release`, and strict replay of the current live
  trace.
- Latest live-trace Large Slime split RNG slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays with
  `verified=True`, `stop_reason=trace_exhausted`, `steps=207`,
  `final_phase=combat`. Source check: `sts_lightspeed` models large slime split
  by `initSpawnedMonster` for each medium slime child; the game jar confirms
  `SpawnMonsterAction` calls `AbstractMonster.init()`, whose first operation is
  `rollMove()`. The remaining RNG drift was that simulator monster-turn code
  still prepared a next intent for the dead original large slime after split,
  consuming an extra AI roll and giving the dead parent a fake future move.
  Checks: `cargo fmt`,
  `cargo test -p sts_core large_slime_split_does_not_roll_dead_parent_next_intent --lib`,
  `cargo test -p sts_core large_acid_slime_split_children_roll_moves_with_ai_rng --lib`,
  `uv run maturin develop --release`, and strict replay of the current live
  trace. Current milestone remains A0 strict/live trace parity; next task is the
  next fresh mismatch or SlayTheData run-level decision replay.
- Latest live-trace verifier normalization slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` tail now strict-replays through step
  199 with `verified=True`, `stop_reason=trace_exhausted`,
  `final_phase=combat`. The exposed issues were verifier-side rather than core
  transition mismatches: Python strict replay did not normalize compact
  `powerthrough` card-choice ids to `POWER_THROUGH_ID`, and Rust observed-state
  import treated large Acid Slime `DEBUFF` as generic Weak 1 instead of the
  source-backed Weak 2 from `AcidSlime_L`. Checks: `cargo fmt`,
  `uv run python -m unittest
  python.tests.test_self_play.SelfPlayTests.test_observed_combat_card_reward_choices_normalize_compact_ironclad_ids`,
  `cargo test -p sts_verify large_acid_slime_debuff_observed_intent_imports_two_weak --lib`,
  `uv run maturin develop --release`, strict replay of the current live trace,
  and `cargo clippy -p sts_verify` (existing warnings). Known risk: observed
  card-choice normalization still has a duplicated Python table and should be
  centralized against Rust card definitions instead of being extended one
  compact id at a time.
- Latest live-trace Large Slime initial AI slice: the current
  `trace-2026-07-01T20-30-26-163Z.jsonl` tail through floor 11 now
  strict-replays with `verified=True`, `stop_reason=trace_exhausted`,
  `steps=191`, `final_phase=combat`. Source check: target
  `AbstractMonster.rollMove()` calls `AbstractDungeon.aiRng.random(99)`, and
  `AcidSlime_L.getMove` maps A0 rolls `30..69` to the normal 16-damage tackle.
  The bug was that Large Slime spawn data pre-stamped an
  `AttackAddSlimedToDiscard` intent, causing combat entry to consume but ignore
  the real first AI roll. The fix leaves Large Slime entry intent unlocked so
  the generic entry AI roll chooses the initial move. Checks: `cargo fmt`,
  `cargo test -p sts_core large_slime_initial_intent_uses_entry_ai_roll --lib`,
  `cargo clippy -p sts_core` (existing warnings), `uv run maturin develop
  --release`, and strict replay of the current live trace. Current milestone
  remains A0 strict/live trace parity; next task is the next fresh mismatch or
  SlayTheData run-level decision replay.
- Latest live-trace Discovery/Mummified Hand/Burning Blood slice: the fresh
  `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays to trace
  exhaustion. The fix keeps Mummified Hand inside the generic combat card-play
  transition using the combat `cardRandomRng`, shares the source-backed
  Discovery duplicate-aware hidden-generation burn across card and potion
  Discovery, and adds the observed live verifier screen-settle draw as a named
  generic DiscoveryAction timing draw rather than any seed/trace override.
  Start-of-player-turn victories now also apply Burning Blood, covering the
  Mayhem-style branch that previously won before the Ironclad heal. Checks:
  `cargo fmt`, `cargo test -p sts_core discovery --lib`,
  `cargo test -p sts_core mummified_hand --lib`,
  `cargo test -p sts_core burning_blood --lib`,
  `uv run maturin develop --release`, and strict replay of
  `trace-2026-07-01T20-30-26-163Z.jsonl` with `verified=True`,
  `stop_reason=trace_exhausted`, `steps=189`, `final_phase=idle`. Current
  milestone remains A0 strict/live trace parity; next task is the next fresh
  mismatch or SlayTheData run-level decision replay. Known risk: the extra
  DiscoveryAction settle draw is modeled generically from live/source timing
  evidence and should be replaced with a richer frame/action model if later
  instrumentation identifies a more exact source.
- Latest live-trace Gremlin Nob Bellow strength slice: Gremlin Nob's Bellow
  intent now applies Anger without immediate Strength, matching the target
  `AngerPower` source behavior; subsequent player Skill plays convert that
  Anger into Strength. Focused Nob regressions cover Bellow not increasing
  attack damage before Skills, and `trace-2026-07-01T20-30-26-163Z.jsonl`
  strict-replays to trace exhaustion after the floor-6 HP mismatch.
- Latest live-trace Gremlin Nob roll slice: Gremlin Nob now records target move
  bytes for Bellow, Bull Rush, and Skull Bash and uses a source-backed
  post-Bellow roll helper for next-intent selection. This fixes the floor-6 Nob
  mismatch where the real game rolled Bull Rush after opening Bellow while the
  simulator's representative sequence chose Skull Bash. Focused Nob regressions
  cover the target roll surface, and `trace-2026-07-01T20-30-26-163Z.jsonl`
  strict-replays to trace exhaustion again.
- Latest live-trace Looter branch slice: Looter now models the source-backed
  post-Mug branch with Lunge (`move_id` 4, 12/14 damage by ascension) and Smoke
  Bomb block/escape move bytes instead of going directly from two Mug attacks
  to block. Combat turn intent selection now uses the target Looter 50/50
  branch helper, focused regressions cover Looter lunge/smoke/escape surfaces,
  and `trace-2026-07-01T20-30-26-163Z.jsonl` again strict-replays to trace
  exhaustion after the floor-4 Looter mismatch.
- Latest live-trace Acid Slime M intent slice: Exordium Thugs spawn metadata now
  opens the weak-slot Acid Slime (M) with Wound Tackle
  (`AttackAddSlimedToDiscard`, 7/8 damage by ascension) instead of Normal
  Tackle. The existing floor-4 Exordium Thugs regression was updated, and the
  fresh live trace `trace-2026-07-01T20-30-26-163Z.jsonl` now strict-replays
  to trace exhaustion after the Acid Slime intent mismatch.
- Latest live-trace card reward rarity slice: Ironclad combat card rewards now
  classify Dark Embrace as an uncommon card in the target reward pool, matching
  the source `RarityCardPool` order used by `AbstractDungeon.getRewardCards`.
  This restores the uncommon reward RNG indexing that produced
  `Bloodletting/Cleave/Havoc` in the fresh UI trace. Focused regressions pin the
  known rarity metadata and the uncommon pool order around Combust, Dark
  Embrace, and Bloodletting; `trace-2026-07-01T20-30-26-163Z.jsonl` now
  strict-replays to trace exhaustion after the longer reward screen.
- Latest live-trace Discovery fidelity slice: Discovery now keeps the played
  source card pending while the generated-card reward screen is open, moving it
  to the normal played-card destination only after the combat reward choice
  closes. Untyped Discovery generation now uses the source-backed
  `returnTrulyRandomCardInCombat()` combat pool order instead of concatenating
  attack/skill/power pools, and Python strict-replay observed reward
  normalization recognizes Shockwave. Focused regressions cover Discovery
  source-card timing, target combat-pool order, observed reward normalization,
  and the fresh `trace-2026-07-01T20-30-26-163Z.jsonl`, which now verifies to
  trace exhaustion. Broader live-run parity remains evidence-driven by future
  strict replay traces.
- Latest live-trace Neow colorless reward slice: Neow's colorless choice-card
  rewards now open the normal run reward screen through `sts_core`, preserving
  the event/Neow RNG counter and the card reward RNG counter before returning
  to the Neow leave prompt after take/skip. The strict verifier's observed-card
  mapper now recognizes Dark Shackles and Discovery so visible colorless reward
  choices are not silently collapsed during CommunicationMod observed-state
  import. Focused regressions cover the core Neow colorless reward transition
  and observed import of Dramatic Entrance/Dark Shackles/Discovery; broader
  live-run parity still depends on subsequent strict replay blockers.
- Latest Searing Blow upgrade-model slice: `CardInstance` now carries a compact
  `searing_blow_upgrades` counter so Searing Blow can keep upgrading beyond the
  `Searing Blow+` content id. Combat damage, Havoc/top-draw play, Armaments,
  Apotheosis-style all-pile upgrades, Blessing of the Forge, rest smithing, map
  grid upgrades, event/random deck upgrades, and run-state upgrade helpers now
  preserve full card instances through upgrades. Focused tests cover the 12, 16,
  21, 27 damage sequence, JSON round-trip/zero-field omission, combat damage for
  a +2 instance, and repeated rest-site smithing. Remaining caveat: this is
  source-backed local mechanics coverage, not played-card CommunicationMod trace
  parity for repeated-upgrade Searing Blow.
- Latest M36 enemy audit backlog slice: the former public-audit "missing monster definition" backlog is now represented in core with partial executable definitions for Masked Bandits (`Bear`, `Pointy`, `Romeo`), City bosses/minions (`Champ`, `The Collector`, `Torch Head`), Beyond bosses/elites/normals/minions (`Awakened One`, `Dagger`, `Deca`, `Donu`, `Exploder`, `Giant Head`, `Nemesis`, `Reptomancer`, `Repulsor`, `Spiker`, `Spire Growth`, `Maw`, `Time Eater`, `Transient`, `Writhing Mass`), and Ending monsters (`Corrupt Heart`, `Spire Shield`, `Spire Spear`). Beyond normal/elite spawn metadata now covers the generated Beyond encounter keys instead of falling back to the Cultist fixture, Act 3 elite map entry selects Beyond elite metadata, and Masked Bandits/Colosseum fight branches now enter representative combat states. This deliberately remains partial: exact source-backed AI/history, boss phases, summon lifecycles, Act 4 special powers, event-combat reward timing, and trace parity are not claimed by this slice.
- Latest monster-audit fix slice: clear executable mismatches from the Ironclad monster audit are now corrected for implemented monsters. Orb Walker Laser/Claw damage and deterministic Claw intent now match the public baseline, Sentry A3 attack damage is wired, Gremlin Nob and Lagavulin A3 attack upgrades are wired, Guardian A4 Fierce Bash/Roll Attack damage is wired on ascension-aware intent preparation, Bronze Automaton A4 Flail/Hyper Beam damage is wired, Bronze Orb Support Beam now grants 12 block in both deterministic and target-helper paths, and Mugger Smoke Bomb setup now grants 28 block. Focused regressions cover these surfaces, and the stale Sentries milestone fixture expectation was updated from the old 6-damage assumption to the audited 9-damage attack.
- Latest strict CommunicationMod replay fidelity slice: the long `trace-2026-07-01T02-00-16-306Z.jsonl` now strict-replays to trace exhaustion alongside `trace-2026-06-30T23-52-04-940Z.jsonl` and the fresh UI smoke trace `trace-2026-07-01T02-48-15-464Z.jsonl`. Fixes in this slice include source-backed Act 3 Transient spawn/Shifting/Fading behavior, `4 Shapes` spawn metadata for the observed floor-40 group, full-belt reward potion visibility matching for strict replay, purpose-aware exact-action enumeration for hand/exhaust selection (`GamblingChip`/Elixir versus Exhume), and carried seed-start map transitions that use core map actions so `?` rooms resolve through the simulator. This is a selected-trace fidelity claim only; broader Act 3 encounter/move RNG and SlayTheData run-level decision replay remain follow-up work.
- Latest M36 Act 2 map prep slice: Milestone 36 has started while M35 remains `partial_data_blocked` on additional full Act 1 traces. The target map generator is now parameterized by `TargetMapAct` and preserves the existing Exordium API through compatibility wrappers. New City/Act 2 APIs (`generate_city_map_topology`, `generate_city_fixed_map`, `generate_city_map_choices_after_path`, and `city_room_kinds_on_path`) use the target-style `seed + actNum * 100` map RNG offset (`seed + 200` for Act 2), target fixed rows (row 0 combat, row 8 treasure, row 14 rest), room-list generation/shuffle/assignment rules, and act-2 `MapRunState` metadata. This is source-backed/synthetic-testable scaffolding only: no Act 2 trace parity or Act 2 boss-reward claim is made yet.
- Latest M36 City encounter-list prep slice: City normal and elite encounter key generation is now decoded from target `TheCity`: two weak normal encounters, first strong encounter with target exclusions, twelve additional strong entries with no-repeat-last-two behavior, and ten elite entries with adjacent-repeat avoidance. New constants/functions cover `CITY_WEAK_ENCOUNTERS`, `CITY_STRONG_ENCOUNTERS`, `CITY_ELITE_ENCOUNTERS`, `generate_city_normal_encounters`, and `generate_city_elite_encounters`, with focused source-backed unit tests. This is encounter-list scaffolding only: City monster group spawning/AI and Act 2 trace parity remain unclaimed.
- Latest M36 City encounter-group prep slice: Act-aware normal encounter-key lookup now supports `TargetMapAct::City`, and target City encounter group composition metadata is decoded from `MonsterHelper.getEncounter` for weak, strong, and elite City keys. This captures source-backed member names/constructor positions, including random-position placeholders, without claiming runnable City monster HP, AI, spawn RNG, or selected Act 2 trace parity.
- Latest M36 City monster HP inventory slice: City-native monster HP ranges are now source-backed from decompiled constructors for Byrd, Chosen, Shelled Parasite, Spheric Guardian, Mugger, Snake Plant, Snecko, Centurion, Healer, Book of Stabbing, Gremlin Leader, and Taskmaster, including the target A7/A8 threshold split. The new `target_city_monster_hp_range` helper is inventory only and is not yet wired into executable City combat spawning.
- Latest M36 City monster profile inventory slice: City-native damage/status/block constants are now decoded into `target_city_monster_profile` for the same City monster set. This covers source-backed A2 damage upgrades, A3/A18 elite constants, A17 support thresholds, starting block/armor/artifact/flight constants, and named multi-hit counts, but it remains data inventory only: no City AI move-selection or executable Act 2 combat groups are claimed.
- Latest M36 Spheric Guardian executable slice: `SphericGuardian` is now a registered core monster with source-backed 20 HP, pre-battle Artifact 3, starting block 40, Barricade-like monster block persistence across player end-turn block clearing, deterministic opening Harden block with source-backed A17 block, Frail attack, double attack, and attack+block loop with source-backed A2 damage variants. This is still only an executable City monster foothold: City encounter spawning and Act 2 trace parity are not claimed.
- Latest M36 Mugger executable slice: `Mugger` is now a registered core monster with source-backed A0 midpoint HP, two opening theft attacks, source-shaped post-second-swipe Big Swipe versus Smoke Bomb branching, deterministic smoke-bomb setup block, escape intent execution, source-backed A2 damage and A17 theft/block variants, and escaped-monster stolen-gold filtering so escaped stolen gold is not offered as recovered reward gold. This reuses the existing stolen-gold reward surface for killed thieves. This remains partial: exact room `mugged` flag semantics, Mugger death-voice `aiRng`, City encounter execution, and Act 2 trace parity are not claimed.
- Latest M36 Chosen executable slice: `Chosen` is now a registered core monster with source-backed A0 midpoint HP, opening double Poke, second-turn Hex intent, stored player Hex debuff state, Hex's non-Attack card-play trigger adding generated Dazed cards to the draw pile, representative post-Hex Debilitate, Drain, Zap, and Poke surfaces, source-backed A2 damage variants, and representative A17 opening Hex behavior. Drain applies player Weak and self Strength. This remains partial: exact random insertion order for Hex-created Dazed cards, exact Chosen RNG move-history selection, City encounter execution, and Act 2 trace parity are not claimed.
- Latest M36 Snake Plant/Malleable executable slice: `SnakePlant` is now a registered core monster with source-backed A0 midpoint HP, pre-battle Malleable 3, local Malleable block/increment on non-lethal attack HP damage, end-of-monster-turn Malleable reset, Chompy Chomps attack-multiple surface with A2 damage, and Spores Frail+Weak surface. This remains partial: Snake Plant's RNG move-selection table, exact A17 history-sensitive move-selection differences, exact non-attack damage interactions for Malleable, City encounter spawning, and Act 2 trace parity are not claimed.
- Latest M36 Snecko/Confusion executable slice: `Snecko` is now a registered core monster with source-backed A0 midpoint HP, deterministic opening Glare applying artifact-blockable Confusion, representative Tail Whip attack+Vulnerable and Bite attack surfaces, A2 Tail/Bite damage variants, A17 Tail Whip Weak+Vulnerable behavior, and drawn-card/opening-hand cost randomization through the shared Confusion/Snecko Eye surface including zero-cost playable cards. This remains partial: exact Snecko RNG move-history selection, exact UI/free-to-play flag timing, City encounter execution, and Act 2 trace parity are not claimed.
- Latest M36 Centurion/Mystic executable slice: `Centurion` and target `Healer`/Mystic are now registered core monsters with source-backed A0 midpoint HP, Centurion Slash/Protect/Fury representative intents, Mystic group Strength/attack+Frail/group Heal surfaces, source-backed A2 damage variants, Centurion A17 block, and Mystic A17 heal/strength variants. This remains partial: exact monster move RNG/history, random Protect target selection, group max-HP capping from rolled HP rather than definition midpoint, City encounter spawning, and Act 2 trace parity are not claimed.
- Latest M36 Byrd/Shelled Parasite executable slice: `Byrd` and `Shelled Parasite` are now registered core monsters with source-backed A0 midpoint HP, Byrd starting Flight metadata plus A17 Flight amount, local attack-damage halving, non-lethal hit stack reduction, grounding-to-Stun when Flight reaches zero, representative Peck/Caw/Swoop/Headbutt intents with source-backed Byrd A2 Peck-count and Swoop variants, and Shelled Parasite starting Plated Armor/block plus Plated Armor break-to-Stun, Double Strike, Life Suck healing from actual player HP damage capped at definition HP, Fell Frail attack surfaces, source-backed Shelled Parasite A2 damage variants, and source-backed A17 opening Fell. This remains partial: exact Byrd go-airborne state transition/Flight reset timing, monster move RNG/history, exact Life Suck action-manager/VFX timing, City encounter spawning, and Act 2 trace parity are not claimed.
- Latest M36 Book of Stabbing executable slice: `BookOfStabbing` is now a registered core monster with source-backed A0 midpoint HP, Painful Stabs metadata, representative growing Stab multi-hit intents, Big Stab attack surface, A3 Stab/Big Stab damage variants, representative A18 post-Big-Stab count growth, and Painful Stabs Wound generation after positive player HP damage from Book attacks. This remains partial: exact AI roll/history, exact action-manager timing for queued Wound generation, elite encounter spawning, and Act 2 trace parity are not claimed.
- Latest M36 Taskmaster executable slice: target `SlaverBoss`/Taskmaster is now a registered core monster with source-backed A0 midpoint HP, Scouring Whip attack+Wound discard generation, A3/A18 Wound-count variants, and A18 self-Strength after Scouring Whip. This remains partial: elite spawn RNG, exact elite action timing, and Act 2 trace parity are not claimed.
- Latest M36 Gremlin Leader executable slice: `GremlinLeader` is now a registered core monster with source-backed A0 midpoint HP, Stab multi-attack surface, Encourage as a whole-monster-list effect that gives Strength to all living monsters and block to living non-leaders with source-backed A3/A18 Encourage variants, a representative Rally path that summons up to two Warrior minions without letting newly summoned minions act during the same monster turn, and leader-death cleanup that makes living Gremlin Leader minions leave combat. This remains partial: exact Rally `aiRng` minion identity/slot parity, exact escape animation/reward timing, exact AI roll/history based on living gremlin counts, elite encounter spawning, and Act 2 trace parity are not claimed.
- Latest M36 City executable encounter-spawn slice: `executable_city_encounter_monsters_for_key` can now materialize local monster states for City groups whose members are explicitly supported (`2 Thieves`, `3 Byrds`, `Chosen`, `Shell Parasite`, `Spheric Guardian`, `Cultist and Chosen`, `3 Cultists`, `Chosen and Byrds`, `Sentry and Sphere`, `Snake Plant`, `Snecko`, `Centurion and Healer`, `Shelled Parasite and Fungi`, `Book of Stabbing`, `Slavers`, and a representative `Gremlin Leader` group). This is still a synthetic-testable spawn helper only: it is not wired as a full run-flow combat-entry claim, exact random minion identity/spawn RNG/rolled HP parity remains partial, and no Act 2 trace parity is claimed.
- Latest M36 City target spawn-metadata slice: `target_city_encounter_spawn_for_key` and `target_city_normal_encounter_spawn_at_combat_index` now expose source-backed City encounter spawn metadata for the same explicitly supported groups, including sequential monster-HP RNG rolls, Neow's Lament current-HP clamping, and decoded starting powers/block for Spheric Guardian, Byrds, Shelled Parasite, Book of Stabbing, Looter, Fungi Beast, Slavers, and representative Gremlin Leader minions. This remains metadata only and does not promote full run-flow combat entry, exact all-group spawn parity, or Act 2 trace parity.
- Latest M36 Fungi/Slavers executable slice: `FungiBeast`, `SlaverBlue`, and `SlaverRed` are now registered partial monsters with source-backed HP ranges/profile constants, Fungi Spore Cloud metadata/death release, Blue Slaver attack+Weak, Red Slaver attack+Vulnerable/Entangle surfaces, verifier intent bucketing for attack-debuff/strong-debuff, source-backed Fungi A2/A17 Grow strength, source-backed Blue/Red Slaver A2 damage and A17 debuff amounts, and source-backed Entangle attack-card play blocking through end-of-player-turn expiry. Fungi death now applies artifact-aware player Vulnerable when another monster remains alive and skips the last-monster battle-ending case. This unlocks `Shelled Parasite and Fungi` and `Slavers` in the City executable encounter and target spawn-metadata helpers. Remaining caveats: exact Fungi/Slaver move RNG/history, exact action-manager timing for simultaneous/multi-death Spore Cloud ordering, elite/run-flow combat entry, and Act 2 trace parity are not claimed.
- Latest M36 Gremlin Leader minion slice: Gremlin Leader's five summonable minion classes (`GremlinWarrior`, `GremlinThief`, `GremlinFat`, `GremlinTsundere`, and `GremlinWizard`) are now registered as partial monsters with source-backed HP ranges/profile constants, Minion/Angry metadata where applicable, representative intent surfaces, source-backed A2 damage/block variants, Warrior A17 Angry, Fat A17 Frail+Weak, Tsundere A17 block, and Wizard A2 magic damage. The `Gremlin Leader` City group now materializes in executable/spawn metadata helpers using the source summon pool's first representative minion type for both initial `random gremlin` slots plus the leader, Rally uses the same representative Warrior minion insertion with a three-living-minion cap, and leader death removes living minions from combat. Remaining caveats: exact `aiRng` minion identity selection, exact slot/animation parity, minion death-react behavior, Gremlin Leader AI roll/history, run-flow elite entry, and Act 2 trace parity are not claimed.
- Latest M36 City event-pool prep slice: City event and shrine inventories are now decoded from target `TheCity` and wired into the deterministic event-list picker when `current_act == 2`. Simple source-backed Act 2 event surfaces now include initial choices for Back to Basics, The Library, The Mausoleum, Vampires, Cursed Tome, Nest, Beggar, Addict, Forgotten Altar, Ghosts, Masked Bandits, Colosseum, and Drug Dealer; Back to Basics/Simplicity upgrades starter Strikes and Defends, Back to Basics/Elegance opens a one-card non-bottled removal grid, The Library/Read opens a 20-card unique obtain grid from the card reward RNG stream, The Library/Sleep heals 33% max HP rounded or 20% at A15+, The Mausoleum/open-coffin grants an event relic and source-backed Writhe curse chance/forced A15 curse, Vampires/Accept loses source-backed max HP and replaces starter Strikes with five Bite cards, Vampires/Blood Vial consumes Blood Vial and replaces starter Strikes without max-HP loss, Bite is a playable damage-and-heal attack, Cursed Tome supports the staged read path with 1/2/3 page HP loss, 10/15 A15 final HP loss, stop-reading damage, random unowned book reward keys, and Circlet fallback, Nest supports the staged continue, 99/50 A15 gold branch, and 6-HP Ritual Dagger branch, Beggar supports 75-gold payment into a non-bottled one-card removal grid, Addict supports 85-gold buy-relic plus steal-relic-with-Shame branches, Forgotten Altar supports Golden Idol to Bloody Idol/Circlet swap plus Shed Blood max-HP/damage plus Smash Altar Decay, Ghosts supports max-HP loss plus 5/3 A15 Apparitions, base/upgraded Apparition plays for one Intangible and exhausts with upgraded Apparition modeled as non-ethereal, Masked Bandits supports the pay-all-gold staged dialogue/exit path plus representative bandit combat entry, Colosseum supports the intro-to-forced-fight prompt, representative Slavers combat entry, post-combat flee surface, and representative Nob combat entry for synthetic staging, and Drug Dealer supports J.A.X. obtain, two-card event transform, and Mutagenic Strength/Circlet reward branches. This remains partial: book relic combat effects, Ritual Dagger combat scaling, exact event-combat reward timing, and Act 2 trace parity are not claimed.
- Latest M35 corpus slice: Milestone 35 is now wired as a manifest-backed Act 1 corpus in `verification/corpus/act1_a0_ironclad.json`, with a regression that runs each listed entry in seed-start mode and requires `unexpected_diffs=0`, `seed_start.expected_failure=false`, boundary category `none`, and no `observed_state_restorations` when the entry forbids restoration. `SimRealReport` and the `sts_verify parity` CLI expose `observed_state_restorations`, and the manifest has per-entry `allow_observed_state_restoration` so M35-complete entries can mechanically forbid verifier restoration. The only clean full Act 1 candidate currently listed is `trace-2026-06-21T09-57-10-380Z.jsonl` (`TEST`), which reaches Act 1 boss relic pickup and pre-Act-2 map return with `unexpected_diffs=0`, `seed_start.expected_failure=false`, and `observed_state_restorations=0`; the manifest now marks it `allow_observed_state_restoration=false`. M35 is not complete because the corpus still needs 4-9 more clean full Act 1 traces before it can be promoted from `partial_data_blocked` to `complete`.
- Latest M35 elite/boss ratchet slice: TEST verifier options can now disable observed-state restoration for the first N TEST elites and for the TEST boss independently, and the selected TEST trace now runs all three elite combats plus the boss without observed-state restoration by default. Gremlin Nob now opens with target-style Bellow, then A0 Skull Bash for 6, then Rush, and Enrage grants visible Strength while its Anger amount no longer double-counts as direct attack damage. `Metallicize+`, `Defend+`, `Bash+`, and `Immolate+` are now first-class enough for the selected TEST trace and Blessing of the Forge path. Guardian Charge Up now grants 9 block in core and observed-intent replay, and Guardian Mode Shift schedules Close Up before Roll Attack with Sharp Hide applied by Close Up rather than at mode-entry. Regret end-turn handling now follows target source more closely by queuing/removing Regret to discard before normal hand discard, clearing the former step 128 pile-order boundary, and player Vulnerable just-applied handling clears the former step 131 HP boundary. The first-two-elites ratchet has no unexpected diffs after rewinding normal reward RNG side effects before replacing the simulated victory screen with an elite reward. The third-elite ratchet clears the Blessing of the Forge upgraded-hand path, and the TEST boss-only ratchet has no unexpected diffs after clearing the former Guardian step 210, Pen Nib step 212, Mummified Hand step 214, Shrug It Off+ step 216, potion path, Bash HP, Guardian end-turn HP, and Pommel Strike current-HP boundaries; lethal attacks now apply pre-existing Sharp Hide/spikes before Burning Blood victory healing.
- Latest M35 hand-select/potion slice: Combat hand-select `PLAY`/`CHOOSE`/`CONFIRM` steps and `POTION USE` commands now replay deterministically through the verifier instead of broad observed-combat restoration, with narrow non-pile refresh only when the surrounding combat already allows observed sync. The TEST trace no longer reports any `combat hand-select` or `combat potion path` observed-state restorations, and total reported restorations are down to 0.
- Latest M35 Looter slice: Looter is now a first-class Act 1 monster for the selected A0 corpus path. Core models the A0 HP range, first Mug-style `AttackStealGold` intent, Thievery amount, run-gold subtraction capped by available gold, hidden stolen-gold carry through observed monster refreshes, and the `STOLEN_GOLD` reward pickup through `RunAction::TakeStolenGoldReward`. The previous TEST-only verifier bridge that subtracted observed Looter theft on END was removed. The TEST trace reaches Act 1 boss relic return-to-map with `unexpected_diffs=0`, `seed_start.expected_failure=false`, and observed-state restorations now reduced further by later M35 ratchets.
- Latest M35 post-END audit slice: post-`END` non-pile restoration records are gone from the TEST full Act 1 trace. Louse curl behavior now models the target-style curl move as +3 Strength with no move block, while the existing Curl Up power still grants its rolled one-time block amount only when HP damage is dealt. The verifier's post-END sync gate now records restoration only when the normalized supported subset differs, while still performing hidden-state stabilization when raw ignored fields differ. TEST still passes with `unexpected_diffs=0`. Disabling post-END sync entirely now cascades from Small Slimes at step 27, not the earlier Louse step 11. Remaining M35 work includes the remaining elite/boss action and potion restoration scopes plus more clean full Act 1 traces.
- Latest M35 late-normal-sync removal: the TEST-specific post-reward handoff that broadened later normal combats into observed-state sync is now disabled by default. TEST still reaches Act 1 boss relic return-to-map with `unexpected_diffs=0` and `seed_start.expected_failure=false`, while reported observed-state restorations drop from 112 to 96. Fixes needed to remove the handoff were narrow: CommunicationMod `Burn` cards now map to `BURN_ID`, `Offering+` now draws five cards instead of three, large slime IDs map to slime content, large Spike Slime `ATTACK_DEBUFF` adds two `Slimed`, and observed-intent reconstruction uses observed max HP to distinguish large slime status counts. Remaining M35 restoration work is now outside the TEST late-normal handoff path.
- Latest M34 shuffle/deck slice: Milestone 34 is complete for the selected Ironclad A0 scope. Selected modified-deck first-combat opening piles are now seed-derived instead of trace-restored. Core combat setup uses the current master-deck order, consumes `shuffleRng.randomLong()`, shuffles with Java `Collections.shuffle`, treats the vector end as draw-pile top, and places innate/bottled cards on top before opening draw. End-turn hand refill now draws a fixed hand-size count even when retained/start-turn cards are already in hand, matching target draw actions; selected post-`END` pile comparisons no longer strip or restore piles. The active `sts_verify` regression covers CODEX04's Neow `Dramatic Entrance`, TEST's Neow `Swift Strike`, M290001's transformed `Sever Soul`, and M290008's transformed `Sentinel`; all selected seed-start parity/corpus tests pass with `unexpected_diffs=0` within their existing completion/expected-boundary scopes.
- Latest simulator hygiene slice: `simulator/AGENTS.md` now explicitly forbids seed-specific branches, allowlists, trace identity tables, hardcoded RNG counters, and observed-state restoration keyed to named seeds or trace labels in simulator, verifier, bridge, or orchestration implementation code. The previous LIVE01 Neow hardcodes were reverted, and seed-start verifier shortcuts/restorations gated on CODEX04/TEST/M290001/M290008/VERIFY01 were removed. Existing selected traces may fail again until the underlying generic Neow, reward, map, and combat mechanics are modeled from source-backed behavior; that is preferred over trace-specific pass-through.
- Latest Neow selected-trace integration slice: Milestone 33 now has table-driven seed-start regression coverage for the clean selected CommunicationMod traces collected on 2026-06-26: upgrade grid, remove-two grid, rare-card reward, rare-colorless reward, rare-relic reward, three-potion reward-screen flow, curse + 250 gold, curse + rare relic, curse + rare colorless, curse + transform-two, and curse + max-HP. The verifier now matches the real grid semantics observed in those traces: Neow upgrade displays the full deck, Neow remove-two/transform-two keep the full visible grid after the first pick, and multi-select completes on the final `CHOOSE` without `CONFIRM`. Neow three potions now supports the real COMBAT_REWARD shape with three individual potion picks followed by `PROCEED`, while preserving older synthetic immediate-shape coverage. Selected-trace display/identity coverage maps the generated rare cards/colorless cards, rare relic `Ice Cream`, and observed curse identities. The selected-trace behavior must be maintained only through generic mechanics and source-backed RNG/state modeling, not named-seed verifier overrides.
- Latest Neow identity/display mapping slice: Milestone 33 verifier canonical display helpers now map the generated Neow identities that were falling back to `unknown` in selected traces: rare Ironclad card rewards `Limit Break`, `Impervious`, and `Feed`; rare colorless rewards `Mayhem`, `Secret Weapon`, `Transmutation`, `Magnetism`, `Chrysalis`, and `Hand Of Greed`; normal curse drawback identities `Parasite`, `Decay`, `Writhe`, and `Doubt`; transform outputs `Limit Break` and `Armaments`; and rare relic `Ice Cream`. Focused `sts_verify` tests pin the card/content display helper and relic trace-name round trip so these are handled through canonical mapping rather than trace tables.
- Latest Neow ThreeRareCards verifier slice: Milestone 33 seed-start verification now has a focused synthetic CommunicationMod-style prefix for a generated Neow `ThreeRareCards` option with a simple drawback. The test reaches the rare-card reward screen, picks a generated rare card, verifies Neow leave, and reaches the first map screen. This improves synthetic/helper coverage only; no selected real CommunicationMod trace currently exercises this exact `ThreeRareCards` reward-screen path.
- Latest Neow curse-transform verifier slice: Milestone 33 seed-start verification now supports the source-possible slot-2 `Curse + TransformTwoCards` branch. The verifier applies the generated curse drawback first through `cardRng`, opens a Neow transform-two multi-select grid, consumes generated Neow transform identities, and reaches Neow leave in a synthetic CommunicationMod-style prefix. This remains synthetic/helper-level coverage only; no selected real CommunicationMod trace currently exercises this exact combo branch.
- Latest Neow verifier grid/Lament carry slice: Milestone 33 seed-start verification now carries generated Neow's Lament through core `RunState::neow_lament_combats_remaining` instead of a local verifier-only flag, decrementing the counter on observed combat entry and relaxing the Neow RNG boundary caveat accordingly. Synthetic CommunicationMod-style verifier prefixes now exercise generated `RemoveTwo` and `UpgradeCard` Neow grid branches through select/confirm, Neow leave, and map entry with zero unexpected diffs. This is still synthetic/helper-level coverage for those two selected branches; no selected real CommunicationMod trace currently exercises `RemoveTwo` or `UpgradeCard`.
- Latest Neow's Lament core slice: Milestone 33 now stores `neow_lament_combats_remaining` on `RunState`, applies the next-three-combats current-HP-to-1 effect during combat entry, decrements once per combat, and round-trips the counter through JSON while omitting the zero value. Focused core tests cover the first three combats, the fourth normal combat, all-monster application, and serialization. This is core simulator support; selected-trace verifier coverage still remains limited to the existing CODEX03 path and CommunicationMod-visible monster max HP remains outside the current `MonsterState` schema.
- Latest Neow boss-swap Tiny House pick slice: Milestone 33 seed-start verification now exercises Tiny House reward-screen card picks after the generated Neow `BossRelic` path. The verifier opens the pending Tiny House card reward, picks a generated card, and compares the updated deck before Neow leave. This remains synthetic/helper-level coverage only: no selected real CommunicationMod trace currently exercises Tiny House boss-swap follow-up.
- Latest Neow boss-swap follow-up slice: Milestone 33 seed-start verification now follows generated Neow `BossRelic` results for Calling Bell queued relic rewards, Empty Cage removal grids, and Tiny House reward-screen opening. Calling Bell now confirms the curse grid and takes the queued common/uncommon/rare relic rewards before Neow leave; Empty Cage now selects and confirms two removals before Neow leave; Tiny House now opens the reward screen before pickup side effects so the pending card reward is queued, then verifies reward opening plus skip. These remain synthetic/helper-level coverage only: no selected real CommunicationMod trace currently exercises these boss-swap follow-ups.
- Latest Neow boss-swap Pandora's Box follow-up slice: Milestone 33 seed-start verification now follows the generated Neow `BossRelic` path when the boss swap produces Pandora's Box. The verifier compares the Pandora's Box replacement grid, accepts `CONFIRM`/`PROCEED` through the existing core grid confirmation helper, and compares the immediate Neow leave state with the transformed deck. Focused synthetic verifier coverage pins the grid-to-leave flow. This remains synthetic/helper-level coverage only: no selected real CommunicationMod trace currently exercises Pandora's Box boss-swap follow-up.
- Latest Neow boss-swap Astrolabe follow-up slice: Milestone 33 seed-start verification now follows the generated Neow `BossRelic` path when the boss swap produces Astrolabe. The verifier compares the initial Astrolabe grid, accepts three `CHOOSE` selections through the existing core grid multi-select/auto-confirm helper, and compares the resulting Neow leave state with the transformed upgraded deck. Focused synthetic verifier coverage pins the grid-to-leave flow. This remains synthetic/helper-level coverage only: no selected real CommunicationMod trace currently exercises Astrolabe boss-swap follow-up.
- Latest Neow normal-curse pool slice: Milestone 33 now uses a source-backed normal curse identity pool/order for random curse generation (`Clumsy`, `Decay`, `Doubt`, `Injury`, `Normality`, `Pain`, `Parasite`, `Regret`, `Shame`, `Writhe`) from target jar `CardLibrary`/`AbstractDungeon.returnRandomCurse` evidence. Neow curse drawback and Cursed Key chest curse now share the same helper while preserving their existing RNG streams (`cardRng`/card reward stream for Neow curse identity; `cardRandomRng` for Cursed Key chest curse). Newly added normal curse definitions are explicit unplayable placeholders where combat/removal effects are not modeled, so this is identity/RNG-pool parity only, not full curse combat behavior parity.
- Latest Neow curse-combo verifier slice: Milestone 33 seed-start verification now allows generated Curse drawback branches to flow into the already-generalized Neow card reward and fixed-tier relic reward surfaces, in addition to the prior immediate gold/max-HP slice. The verifier applies the core curse drawback first, carries the deck/cardRng counter update into the visible comparison, and for rare colorless rewards uses a new core helper that starts colorless identity generation from the post-curse `cardRng` counter instead of reusing counter-zero choices. Newer curse-transform coverage also supports the source-possible `Curse + TransformTwoCards` branch. Caveats remain narrow and explicit: curse identity now uses the source-backed normal curse pool/order, but several curse combat/removal effects are still inert/unmodeled; selected full-trace parity is synthetic/helper-level until a captured trace exercises these exact combo branches; impossible curse plus potion/remove/upgrade/boss-swap branches are not modeled.
- Latest Neow rare-relic verifier slice: Milestone 33 seed-start verification now extends the generated fixed-tier relic branch from `RandomCommonRelic` to `OneRareRelic` options with simple drawbacks and the narrow generated Curse drawback path. The verifier applies the supported drawback first, calls the core `apply_neow_relic_reward` helper with `NeowRewardType::OneRareRelic`, compares the immediate post-Neow event state, and updates the visible deck/relic list. Focused synthetic/helper tests pin simple max-HP-loss rare relic routing, curse plus mapped rare-relic deck updates, rejection of rare-colorless identity branches from the relic path, and a synthetic TEST rare-relic option through Neow leave; this is synthetic/helper-level coverage only unless a selected trace later exercises the branch.
- Latest Neow rare-colorless verifier slice: Milestone 33 seed-start verification now routes generated `RandomColorlessTwo` options through `generate_neow_colorless_reward(numeric_seed, RandomColorlessTwo)`, remembers the generated CARD_REWARD choices, compares the visible rare-colorless card reward screen, and reuses the generic Neow card pickup path for the selected card. Focused synthetic/helper tests pin the rare-colorless dispatch to the core colorless helper and guard that existing `RandomColorless` helper behavior stays intact. This does not claim selected full-trace parity until a trace exercises the rare-colorless branch; card reward UUIDs remain unobservable in CommunicationMod state.
- Latest Neow curse-immediate verifier slice: Milestone 33 seed-start verification wires the existing core `apply_neow_curse_drawback` helper for generated Curse drawback branches whose reward is an immediate `TwoFiftyGold` or `TwentyPercentHpBonus`; newer curse-combo coverage also routes supported card reward, fixed-tier relic reward, and transform-two follow-ups. The verifier seeds the card-reward RNG from the run seed, applies the modeled curse before the reward, compares the immediate post-Neow event state where applicable, and records an explicit caveat that curse identity uses the source-backed normal curse pool/order while several curse combat/removal effects remain inert/unmodeled. Focused verifier helper tests pin the gold and max-HP branches, deck growth, and one cardRng draw; selected real-trace coverage for these exact curse combos remains missing.
- Latest Neow boss-swap Calling Bell follow-up slice: Milestone 33 seed-start verification now follows the generated Neow `BossRelic` path when the boss swap produces Calling Bell. The verifier compares the immediate Calling Bell curse grid, accepts `PROCEED`/`CONFIRM` through the existing core grid confirmation helper, and compares the resulting first relic reward screen; Astrolabe, Pandora's Box, Empty Cage, and Tiny House are now handled by newer synthetic follow-up slices. Focused synthetic verifier tests pin generated slot-3 routing, starter-relic removal, mapped boss-relic display, boss-swap classification, and a synthetic Calling Bell grid-to-reward trace. This is synthetic/helper-level coverage only and does not claim selected real-trace parity; the Calling Bell grid card label still uses the current verifier `unknown` fallback until Curse of the Bell display mapping is generalized.
- Latest Neow grid verifier slice: Milestone 33 seed-start verification now has a narrow generated-option grid path for Neow `RemoveCard`/`RemoveTwo`/`UpgradeCard` rewards that opens the existing core Neow grid helper, compares the visible grid subset, and processes choose/confirm back to Neow leave. Focused synthetic verifier unit tests pin the `UpgradeCard` grid dispatch and choose/confirm flow, plus the multi-select continuation shape for `RemoveTwo`; selected real-trace coverage now pins `M290005` choosing generated `RemoveCard`, removing a Strike, confirming, and leaving to the map with `unexpected_diffs=0` and boundary `none`. `RemoveTwo` and `UpgradeCard` selected real-trace parity remain helper-level until selected traces exercise them.
- Latest Neow potion reward verifier slice: Milestone 33 seed-start verification now has a narrow `THREE_SMALL_POTIONS`/`obtain 3 random potions` branch that compares the post-Neow potion belt against `generate_neow_three_potions` and canonical potion display names. A focused synthetic CODEX04 prefix test pins the branch routing and verifier comparison without claiming a selected full-trace branch; potion reward UUIDs remain unobservable, and this still relies only on the existing direct full-pool `potionRng` helper evidence rather than broadening normal reward potion rarity/order claims.
- Latest Neow card reward verifier slice: Milestone 33 seed-start verification now wires generated `THREE_CARDS`, immediate `ONE_RANDOM_RARE_CARD`, and `THREE_RARE_CARDS` Neow card rewards through the existing generated helper in `sts_core::run::neow`, remembers generated reward-screen choices where the target opens a screen, and applies the generated card to the visible deck. The same generic Neow card reward pickup path also handles the already-generated colorless reward screens instead of a captured seed-only card lookup. Focused verifier tests pin that VERIFY01's generated `choose a card to obtain` branch uses the helper and that reward pickup indexes resolve from generated choices; the selected real MANUAL01 prefix now verifies `ONE_RANDOM_RARE_CARD` through the immediate `Immolate` deck add and map leave with zero unexpected diffs. `THREE_RARE_CARDS` remains synthetic/helper-level until a selected corpus trace exercises that reward screen; card reward UUIDs/RNG internals remain unobservable in CommunicationMod state.
- Latest Neow curse drawback slice: Milestone 33 now has a narrow `apply_neow_curse_drawback` helper that consumes exactly one `cardRng` (`RunRngStream::CardReward`) identity draw, stores the updated `card_rng_counter`, and then uses the existing deck-add path so Omamori, Darkstone Periapt, and ordinary curse classification hooks stay centralized. Focused tests pin that the helper leaves `cardRandomRng` untouched, consumes the `cardRng` draw even when Omamori prevents the curse from entering the deck, and grants Darkstone Periapt HP when the curse is actually added. Curse identity now uses the source-backed normal curse pool/order; full combat/removal behavior for all normal curses remains partial because newly added curse definitions are explicit inert/unplayable placeholders.
- Latest Neow option/reward-generation slice: Milestone 33 now has a source-backed `sts_core::run::neow` option generator for Ironclad A0, and seed-start verification compares Neow option labels from `numeric_seed` instead of a hardcoded label table. Target jar bytecode evidence pins `NeowEvent.blessing()` to `NeowEvent.rng = Random(Settings.seed)`, visible slots 0-3 in order, and five option-screen RNG draws: slot-0 reward, slot-1 reward, slot-2 drawback, slot-2 reward, and slot-3 boss-swap reward. Focused unit tests verify generated labels for VERIFY01 and CODEX04 and the five-draw counter; verifier Neow tests preserve current trace behavior. Seed-start Neow branch dispatch now derives the selected reward from generated options for transform, Neow's Lament, colorless-card, common-relic, and simple immediate gold/max-HP branches instead of routing those branches by seed name. Reward helpers now cover normal `THREE_CARDS` rewards as three common/uncommon rolls on `NeowEvent.rng` with the target `0.33f` uncommon threshold, forced-rare Neow card rewards (`ONE_RANDOM_RARE_CARD`, `THREE_RARE_CARDS`) on `NeowEvent.rng`, split-stream colorless rewards (`RANDOM_COLORLESS`, `RANDOM_COLORLESS_2`) that burn `NeowEvent.rng` rarity rolls and draw identities from `cardRng`, Neow's three-potion reward as three direct full-pool `potionRng` draws, fixed-tier Neow common/rare relic application that initializes relic pools and pops the requested tier without a relic-tier roll, no-RNG immediate gold/max-HP reward and gold/max-HP/HP-loss drawback application, Neow-specific remove-one/remove-two/upgrade grid opening and confirmation without shop/rest side effects, Neow boss-swap application that removes Burning Blood before popping the initialized boss relic pool without extra relic RNG draws, and a Neow transform helper that consumes `NeowEvent.rng` over the target-style Ironclad transform pool (`srcCommonCardPool + srcUncommonCardPool + srcRareCardPool`, excluding the source card) without Astrolabe's upgrade step. VERIFY01 seed-start common relic identity now derives Toy Ornithopter from the fixed-tier relic helper instead of a hardcoded relic name, while retaining the trace-local Toy Ornithopter potion-use caveat. CODEX04/TEST seed-start colorless reward screens derive card choices from the generated helper instead of captured card tables, and seed-start helper coverage now routes generated `THREE_CARDS`, `ONE_RANDOM_RARE_CARD`, and `THREE_RARE_CARDS` Neow card reward choices through the core card-reward generator and generic pick path. M290001/M290008 transform-card identity now derives from generated transform rewards and focused tests pin Strike_R -> Sever Soul / Sentinel plus source exclusion and one/two-transform counters, so the old captured transformed-card lookup is retired. M290008 generated `obtain 100 gold` and `lose all gold max hp +16` verifier helper tests now exercise the simple immediate helper path without claiming a selected full-trace branch. Follow-up target evidence says curse drawback identity uses `cardRng`; selected-trace verifier application of newly generalized card, potion, grid-selection, boss-swap, and curse branches is still caveated until exercised by trace data or explicit branch tests.
- Latest direct potion action evidence slice: Milestone 32C target jar constant-pool/string audit links `FirePotion` to `DamageAction`/`DamageInfo`, `BlockPotion` to `GainBlockAction`, `FearPotion` to `ApplyPowerAction`/`VulnerablePower`, `WeakenPotion` to `ApplyPowerAction`/`WeakPower`, `EnergyPotion` to `GainEnergyAction`, `DuplicationPotion` to `ApplyPowerAction`/`DuplicationPower`, `LiquidMemories` to `BetterDiscardPileToHandAction`, and `SmokeBomb` to `canUse` plus smoke/escape class fields. The relic/potion matrix promotes only those narrow direct-use/action surfaces plus existing focused unit coverage to `source_and_unit`; exact action-manager ordering, UI/popup timing, Smoke Bomb escape-effect flags/timing, Duplication/Double Tap combination timing, Liquid Memories selection ordering, and played-potion CommunicationMod trace parity remain unclaimed.
- Latest run-world evidence backfill slice: Milestone 32C target jar constant-pool inspection promotes only narrow RUN-WORLD rows in `m32a_run_world_matrix.md`: Slime Boss and Guardian move to `source_and_unit` for target boss class/action/power surfaces plus local unit coverage; Golden Shrine and The Cleric move to `source_and_unit` for target event branch/action symbols plus local unit coverage; ascension clamp, A2/A17 damage flags, and A10 Ascender's Bane starter-deck insertion move to `source_and_unit` where target class/field inventory supports the narrow claim. A1 elite-room plumbing, A7 HP scaling, A20 double-boss flag, placeholder map generation, and milestone8 fixture remain `unit_only`/fixture-scoped because this slice found no safe source/trace parity promotion. No full Act 1, full boss-fight, event UI timing, non-A0 trace parity, or double-boss execution parity is claimed.
- Latest relic hook/counter evidence slice: Milestone 32C promoted only the grouped relic/potion matrix rows for `RelicSpawnContext`, `Relic counters`, and `Potion-relic interactions` to narrow `source_and_unit` evidence. Existing target source/class evidence plus focused local regressions cover the named spawn filters, relic counter/reset/persistence hooks, potion-relic capacity/reward/use/heal surfaces, and Fairy/Lizard Tail/Sacred Bark lethal-damage ordering; exact action-manager timing, broad seed-start trace parity, full relic-pool exhaustion permutations, and instruction-level revive queue timing remain unclaimed.
- Latest simple upgraded/basic Ironclad evidence slice: Milestone 32C target jar constant-pool audit links `BattleTrance` to `DrawCardAction`/`ApplyPowerAction`/`NoDrawPower`, `SeeingRed` to `GainEnergyAction`, `Inflame` to `ApplyPowerAction`/`StrengthPower`, `Flex` to `ApplyPowerAction`/`StrengthPower`/`LoseStrengthPower`, `SpotWeakness` to `SpotWeaknessAction` and `ApplyPowerAction`/`StrengthPower`, `Whirlwind`/`WhirlwindAction` to `DamageAllEnemiesAction` plus energy-use/multi-damage surfaces, `SearingBlow` to `DamageAction`/`DamageInfo`, and `Sentinel` to `GainBlockAction`/`GainEnergyAction`/`triggerOnExhaust`. The cards matrix promotes only those narrow source/action/power surfaces plus existing focused unit coverage to `source_and_unit`; exact action-manager ordering, target UI/timing details, Whirlwind energy-panel semantics, Flex/LoseStrength lifecycle, Sentinel exhaust ordering, Searing Blow multi-upgrade scaling beyond the modeled + row, and played-card CommunicationMod trace parity remain unclaimed.
- Latest status/curse evidence cleanup: Milestone 32C target jar constant-pool audit links `Wound` to inert status metadata, `Dazed` to status metadata plus `isEthereal`, `Burn` to status metadata plus `dontTriggerOnUseCard`/`DamageInfo`, `Slimed` to status metadata plus `SELF`/`exhaust`, `Regret` to curse metadata plus `dontTriggerOnUseCard`/`LoseHPAction` with `BlueCandle` curse-use evidence, and `Doubt` to curse metadata plus `dontTriggerOnUseCard`/`ApplyPowerAction`/`WeakPower`. The cards matrix promotes only these narrow definition/action surfaces plus existing focused unit coverage for Wound, Dazed, Burn, Slimed, Regret, and Doubt to `source_and_unit`; exact end-turn/action-manager ordering, duplicate-trigger ordering, generated-status timing, and CommunicationMod trace parity remain unclaimed.
- Latest HP-loss/damage-growth attack evidence slice: Milestone 32C target jar constant-pool audit links `Hemokinesis` to `LoseHPAction`/`DamageAction`, `Bloodletting` to `LoseHPAction`/`GainEnergyAction`, `BloodForBlood` to `DamageAction` plus `tookDamage`/`damagedThisCombat`/`updateCost`, `Rampage` to `DamageAction`/`ModifyDamageAction` with UUID-targeted base-damage modification, `Carnage` to `DamageAction` plus `isEthereal`, `Dropkick`/`DropkickAction` to `DamageAction`/`GainEnergyAction`/`DrawCardAction` gated by `VulnerablePower`, and `FlameBarrier`/`FlameBarrierPower` to `GainBlockAction`/`ApplyPowerAction` plus `onAttacked`/`DamageAction`/`RemoveSpecificPowerAction`. The cards matrix promotes only these narrow source/action/power surfaces plus existing focused unit coverage to `source_and_unit`; exact HP-loss trigger timing, Blood for Blood update ordering, Rampage copy/stat-equivalent-copy semantics, Carnage end-turn ethereal timing, Flame Barrier power-removal timing, and played-card CommunicationMod trace parity remain unclaimed.
- **68 Ironclad cards plus deterministic colorless slices** (Milestone 5 complete + Ascender's Bane + Dramatic Entrance + base Flame Barrier + base Limit Break + base Offering + base Armaments + base Headbutt + base Reaper + base Second Wind + base Fiend Fire + base Corruption + base Juggernaut + base Barricade + base Berserk + base Rampage + base Brutality + base Combust + base Double Tap + base Rupture + base Blood for Blood + base Evolve + base Fire Breathing + base Feed + base Exhume + base Infernal Blade + deterministic colorless uncommon/rare sweep through Sadistic Nature)
- Latest Ironclad power/effect evidence slice: Milestone 32C target jar constant-pool audit links Feel No Pain, Dark Embrace, Metallicize, Demon Form, Combust, Rupture, and Rage card plays to `ApplyPowerAction` plus their expected power classes. The audited power classes link Feel No Pain/Dark Embrace exhaust hooks to `GainBlockAction`/`DrawCardAction`, Metallicize end-turn pre-end-turn-card hook to `GainBlockAction`, Demon Form post-draw start-turn hook to `ApplyPowerAction`/`StrengthPower`, Combust end-turn hook to `LoseHPAction`/`DamageAllEnemiesAction`/`stackPower`, Rupture to `isPostActionPower`/Strength application, and Rage attack-use/end-turn hooks to `GainBlockAction`/`RemoveSpecificPowerAction`. The cards matrix promotes only these narrow source/action surfaces plus existing focused unit coverage to `source_and_unit`; exact action-manager ordering, broad hook coverage, upgraded edge behavior where caveated, Demon Form target power identity versus the local Ritual representation, and played-card CommunicationMod trace parity remain unclaimed.
- Latest Ironclad select/exhaust skills evidence slice: Milestone 32C target jar constant-pool audit links Havoc to `PlayTopCardAction`/`cardRandomRng`/`MonsterGroup.getRandomMonster`, Warcry to `DrawCardAction`/`PutOnDeckAction`, Dual Wield to `DualWieldAction`/`MakeTempCardInHandAction`, Burning Pact to `ExhaustAction`/`DrawCardAction`, Armaments to `GainBlockAction`/`ArmamentsAction`, Headbutt to `DamageAction`/`DiscardPileToTopOfDeckAction`, Second Wind to `BlockPerNonAttackAction`/`ExhaustSpecificCardAction`/`GainBlockAction`, Entrench to `DoubleYourBlockAction`, and Disarm to `ApplyPowerAction`/`StrengthPower`. The cards matrix promotes only those narrow action/select/exhaust surfaces to `source_and_unit`; exact action-manager ordering, selection UI/order, target RNG parity, upgraded Dual Wield cost parity, and played-card CommunicationMod trace parity remain unclaimed.
- Latest starter/common simple attack/block evidence slice: Milestone 32C target jar constant-pool audit links `Strike_Red` to `DamageAction`, `Defend_Red` to `GainBlockAction`, `Bash`/`ThunderClap`/`Uppercut` to `ApplyPowerAction` with Vulnerable/Weak power surfaces as applicable, `Anger` to `MakeTempCardInDiscardAction`, `Cleave`/`Immolate` to `DamageAllEnemiesAction`, `TwinStrike`/`PommelStrike`/`SeverSoul` to `DamageAction`, `PommelStrike`/`ShrugItOff` to `DrawCardAction`, `ShrugItOff`/`TrueGrit` to `GainBlockAction`, and `TrueGrit` to `ExhaustAction`. The cards matrix promotes only the narrow source/action-surface plus existing local unit evidence for Strike/Strike+/Defend/Bash/Anger/Anger+/Cleave/Cleave+/Twin Strike/Twin Strike+/Pommel Strike/Pommel Strike+/Shrug It Off/True Grit/Thunderclap/Uppercut/Immolate/Sever Soul. Exact action-manager instruction ordering, True Grit selection/random semantics, Sever Soul's non-Attack exhaust behavior beyond local unit evidence, generated-card ordering, and played-card CommunicationMod trace parity remain unclaimed.
- Latest random/generated/complex evidence slice: Milestone 32C target jar constant-pool audit links `SwordBoomerang`/`SwordBoomerangAction` to random-enemy damage, `cardRandomRng`, and `getRandomMonster`; `InfernalBlade` to `MakeTempCardInHandAction`, `makeCopy`, and `setCostForTurn`; `Shockwave` to `ApplyPowerAction` plus Weak/Vulnerable/Strength power strings; `Forethought` to `ForethoughtAction`/`CardGroup`; `MindBlast`/`MindBlastAction` to damage plus `drawPile`/`CardGroup`; and `Panache`/`PanachePower` to `ApplyPowerAction`, `onUseCard`, `UseCardAction`, and `DamageAllEnemiesAction`. The cards matrix promotes only the narrow source-backed action surfaces for Sword Boomerang, Infernal Blade, Shockwave, and Forethought; Mind Blast stays `unit_only` because the simulator uses current combat piles rather than target master-deck count, and Panache stays `unit_only` because constant-pool evidence does not establish fifth-card timing versus the played card's own queued effects. Multi-enemy Sword Boomerang RNG parity, generated-card exact pools/RNG, exact action-manager ordering, and played-card CommunicationMod trace parity remain unclaimed.
- Final risky colorless-row audit: Mind Blast and Panache were rechecked against the target jar class inventory. The available evidence still does not resolve Mind Blast's target master-deck damage source versus the simulator's current combat-pile count, nor Panache's fifth-card trigger timing versus the played card's own queued effects. Both rows remain `unit_only`; no new parity promotion is claimed.
- Latest Limit Break / Fire Breathing / Evolve source-evidence slice: Milestone 32C target jar constant-pool audit found `LimitBreak` references to `LimitBreakAction`, with `LimitBreakAction` referencing player `Strength`, `ApplyPowerAction`, and `StrengthPower`; `FireBreathing` references `ApplyPowerAction`/`FireBreathingPower`, and `FireBreathingPower.onCardDraw` references Status/Curse checks plus `DamageAllEnemiesAction`/`DamageInfo`; `Evolve` references `ApplyPowerAction`/`EvolvePower`, and `EvolvePower.onCardDraw` references Status checks plus `DrawCardAction`. The cards matrix promotes only these narrow card-play/power/draw-trigger action surfaces to `source_and_unit`; exact instruction ordering, action-manager timing, upgraded edge details where caveated, and played-card CommunicationMod trace parity remain unclaimed.
- Latest Offering/Reaper source-evidence slice: Milestone 32C target jar constant-pool audit found `Offering` references to `LoseHPAction`, `GainEnergyAction`, `DrawCardAction`, `OfferingEffect`, `SELF`, `SKILL`, magic-number fields, and exhaust, and `Reaper` references to `VampireDamageAllEnemiesAction`, `DamageInfo`, `multiDamage`, `ALL_ENEMY`, `HEALING`, and exhaust; `VampireDamageAllEnemiesAction` references monster damage, `lastDamageTaken`, `HealAction`, and monster-death checks. The cards matrix promotes only the narrow local unit plus target class/action-surface evidence for Offering HP-loss/energy/draw/exhaust and Reaper all-enemy damage/heal/exhaust. Exact action-manager instruction ordering, Reaper overkill/minion/half-dead edge behavior, and played-card CommunicationMod trace parity remain unclaimed.
- Latest Exhume/Discovery/Madness evidence slice: Milestone 32C target jar constant-pool audit found `Exhume` -> `ExhumeAction` with `exhaustPile`/`gridSelectScreen`/`selectedCards`/`removeCard`/`addToHand`, `Discovery` -> `DiscoveryAction` with `generateCardChoices`/random combat card generation/`makeStatEquivalentCopy`/`setCostForTurn`/hand add effect, and `Madness` -> `MadnessAction` with player `hand`/`CardGroup.getRandomCard`/`cardRandomRng`/`setCostForTurn`. The cards matrix promotes only those narrow select/random/action surfaces to `source_and_unit`; exact UI ordering, full action-manager timing, fallback behavior without attached RNG, and played-card CommunicationMod trace parity remain unclaimed.
- Latest Fiend Fire source-evidence slice: Milestone 32C target jar constant-pool audit found `FiendFire.use` references to `FiendFireAction`/`DamageInfo`, and `FiendFireAction` references to player `hand`, `ExhaustAction`, and `DamageAction`, so the cards matrix promotes only the narrow damage-per-other-hand-card plus hand-exhaust action surface to `source_and_unit`. Exact action-manager interleaving/exhaust-vs-damage ordering and played-card CommunicationMod trace parity remain unclaimed.
- Latest passive potion evidence slice: Milestone 32C added a focused Fairy in a Bottle regression for spent Lizard Tail fallback with Sacred Bark-doubled Fairy healing, and narrowed the relic/potion matrix caveat to local/unit evidence only. This does not claim target trace parity or instruction-level revive queue timing.
- Latest Smoke Bomb evidence slice: Milestone 32C strengthened local regressions for non-boss combat escape without reward, boss-room rejection before potion consumption, and Toy Ornithopter's run-level potion-use heal after escape. The matrix remains `unit_only` because no target bytecode/source or CommunicationMod trace parity is claimed.
- Latest Entropic Brew evidence slice: Milestone 32C tightened local/source regressions for consume-before-fill refill order, exact local `target_random_potion` sequence/counter, full ordinary belt one-slot refill, Sozu no-roll/no-fill behavior, and Potion Belt expanded capacity. The matrix promotes only this narrow local/source plus unit evidence; no played-potion CommunicationMod trace parity or target bytecode action-order claim is made.
- Latest event slice: Milestone 32C event evidence backfill added focused Golden Shrine coverage for generated shrine-pool entry plus the legacy fixed fixture routing to the same Pray gold branch, and pinned The Cleric heal branch while making the remove-curse branch explicitly unsupported. `m32a_run_world_matrix.md` now marks Golden Shrine as unit-only evidence and The Cleric as partial/unit-only; no CommunicationMod trace parity or event action-timing parity is claimed.
- Previous rest slice: Milestone 32C rest action evidence backfill added a focused transition regression covering heal/Peace Pipe remove/Girya lift idle exits, smith grid opening, and Shovel relic reward entry, then promoted the rest rows in `m32a_run_world_matrix.md` only to narrow source-backed campfire-option/relic-gating evidence. Evidence came from target jar class/constant-pool inspection of `RestOption`, `SmithOption`, `TokeOption`, `LiftOption`, `DigOption`, `PeacePipe`, `Girya`, and `Shovel`; `javap` was not available, so exact instruction ordering, full campfire UI timing, and played CommunicationMod trace parity remain unclaimed.
- Latest Blind/Trip/Dark Shackles/Sadistic Nature source-evidence slice: Milestone 32C target jar constant-pool audit links Blind to `ApplyPowerAction`/`WeakPower` plus base enemy and upgraded all-enemy target constants, Trip to `ApplyPowerAction`/`VulnerablePower` plus base enemy and upgraded all-enemy target constants, Dark Shackles to `ApplyPowerAction`/`StrengthPower`/`GainStrengthPower`/`ArtifactPower`, and Sadistic Nature/SadisticPower to `ApplyPowerAction`, `onApplyPower`, `AbstractPower.PowerType.DEBUFF`, `DamageAction`, `DamageInfo`, target/source parameters, and `ArtifactPower` checks. The cards matrix promotes only these narrow source-class/action-surface claims plus existing focused units for Sadistic hooks, Champion Belt, Strange Spoon source-exhaust, and monster Artifact blocking; exact action-manager instruction ordering, Dark Shackles paired Strength/GainStrength queue timing, a global `ApplyPowerAction` model, and played-card CommunicationMod trace parity remain unclaimed.
- Previous slice: Milestone 32C evidence backfill strengthened Snecko Oil's local evidence for draw-five before playable-hand cost randomization, unplayable skips, and exact `cardRandomRng.random(3)` sequence/counter, promoting only the narrow source-class/constant-pool plus unit-backed potion row. Earlier 32C work added focused Strange Spoon non-exhaust/no-roll regressions for Swift Strike and Flash of Steel, and promoted only their narrow target-jar action-surface evidence from constant-pool inspection (`SwiftStrike` -> `DamageAction`, `FlashOfSteel` -> `DamageAction`/`DrawCardAction`). Earlier 32C work added Finesse/Good Instincts no-roll regressions and broadened Sadistic Nature's supported debuff hooks so Champion Belt Weak counts as a second player-applied debuff and Hand Drill Vulnerable triggers Sadistic when block breaks. Parallel 32C slices also promoted Panacea, Apotheosis, Master of Strategy, Hand Of Greed, Secret Weapon, and Secret Technique with focused source/Strange Spoon evidence while keeping caveats scoped: Snecko Oil has no played-potion CommunicationMod trace parity or full action-manager timing claim, Apotheosis action ordering beyond the shared played-card exhaust hook remains local/source-audit incomplete, Secret Weapon/Technique search-screen order remains local, Hand Of Greed's gold-on-kill payout is not modeled because combat state does not carry run gold, Panache remains `unit_only` pending instruction-level timing evidence, generated-card/random-selection semantics remain local deterministic approximations unless separately source-backed, and there is no broad played-card CommunicationMod trace parity claim for these cards yet.
- Guardian/Slime Boss run-world evidence audit: clarified the boss rows without promoting parity. Slime Boss remains `unit_only` with local split behavior limited to the half-HP acid-slime spawn regression and no real-trace boss-fight/selection claim. Guardian now records the historical TEST observed-sync slice and `test_seed_start_m29_test_elite_boss_without_observed_sync` guard as relevant coverage, but the row intentionally remains `unit_only` because this is verifier scaffolding/local mechanics evidence, not full boss-fight, boss-selection, or all-branch Guardian parity.
- Distilled Chaos 32C evidence slice: added a mixed-pile regression for actual top-draw ordering, unplayable top-card no-target/no-RNG behavior, and targeted-card `cardRandomRng` consumption; aligned the shared top-draw definition helper with the existing play-top-draw pop order. A follow-up target jar constant-pool audit found `DistilledChaosPotion` references to `PlayTopCardAction`, `cardRandomRng`, and `MonsterGroup.getRandomMonster`, plus `PlayTopCardAction` references to `drawPile`, `getTopCard`, `NewQueueCardAction`, and `EmptyDeckShuffleAction`, so the matrix row is promoted only to narrow `source_and_unit` evidence. Exact action-manager instruction order and played-potion CommunicationMod trace parity remain unclaimed.
- Double Tap 32C evidence audit: promoted the matrix row only to narrow `source_and_unit` for the power/replay surface after target jar constant-pool inspection found `DoubleTap` references to `ApplyPowerAction` and `DoubleTapPower`, plus `DoubleTapPower` references to `onUseCard`, `ATTACK`, `amount`, `CardQueueItem`, `addCardQueueItem`, `UseCardAction`, and `RemoveSpecificPowerAction`. Existing focused unit tests already cover base and upgraded replay counts, per-Attack consumption, stacking, non-Attack non-consumption, and event-log ordering. Exact instruction ordering/action-manager timing, Duplication Potion combination behavior, and played-card trace parity remain unclaimed.
- Power rare card 32C evidence audit: target jar constant-pool inspection links Corruption/Barricade/Berserk card plays to `ApplyPowerAction` plus their expected power classes, `CorruptionPower` to `onUseCard`/`UseCardAction`/`setCostForTurn`, and `BerserkPower` to `atStartOfTurn`/`GainEnergyAction`. The cards matrix promotes only those narrow card-play/power surfaces to `source_and_unit`; exact action-manager timing, broader power lifecycle ordering, Strange Spoon/cost-modifier edge cases where noted, and played-card CommunicationMod trace parity remain unclaimed.
- Full Act 1 monster + boss roster
- Ascension modifiers A0-A20 (config, elites, damage, HP, Bane, deadly enemies, double boss)

### Run / Meta
- Reward screen with source-backed card/gold/potion/relic RNG; elite/chest/boss relic reward screens from persisted pools
- Shop: full target-style inventory (7 cards, 3 relics, 3 potions, remove service) via `merchantRng`/`cardRng`/`potionRng` and relic pools; legacy fixed Anger/Vajra/Fire fixture when `merchant_rng_seed == 0`
- Potions: full 33-potion Ironclad reward pool for drops, direct use/legality coverage for implemented active potions, discovery choices, Entropic Brew refill, Duplication replay, Distilled Chaos, Liquid Memories, Snecko Oil, Smoke Bomb, Elixir, and Fairy in a Bottle passive revive
- Events: Act 1 event/shrine pools with `generateEvent` shrine chance; map event rooms call `enter_event_screen`; Shining Light costs 20% max HP and upgrades up to two random upgradeable deck cards
- Rest: heal, smith, card removal (deterministic heal amount; no RNG)

### Relics / Potions
- Common simple relic: Strawberry pickup HP bonus
- Pickup/capacity relics: Blood Vial, Pear, Mango, Old Coin, Lee's Waffle, and Potion Belt
- Start-combat relics: Lantern, Bag of Preparation, Bag of Marbles, Bronze Scales, Thread and Needle, Red Skull
- Energy relic: Coffee Dripper energy per turn and rest restriction
- Start-combat relic: Anchor block
- On-card-play relic: Ink Bottle draw after 10 cards
- Damage/block relic: Ornamental Fan block every 3 attacks per turn
- Card-play counter relics: Nunchaku, Shuriken, Kunai, and Letter Opener
- Turn-timed combat relics: Happy Flower, Orichalcum, Horn Cleat, Captain's Wheel, Mercury Hourglass, and Stone Calendar
- Combat-victory healing relics: Black Blood and Meat on the Bone
- Room/rest healing relics: Meal Ticket, Regal Pillow, Dream Catcher, and Eternal Feather
- Damage mitigation relics: Torii and Tungsten Rod
- Shop/economy relics: Ceramic Fish, Membership Card, and Smiling Mask
- Boss-entry relic: Pantograph
- Debuff-immunity relics: Ginger and Turnip
- Boss energy relic: Mark of Pain
- Combat healing relic: Magic Flower
- Vulnerable synergy relics: Paper Phrog and Champion Belt
- Elite HP relic: Preserved Insect
- Curse synergy relics: Darkstone Periapt and Du-Vu Doll
- Boss energy/rest-restriction relic: Fusion Hammer
- Boss energy/potion-lockout relic: Sozu
- Boss energy/card-reward relic: Busted Crown
- Boss energy/card-limit relic: Velvet Choker
- Potion-use healing relic: Toy Ornithopter
- Card-add upgrade relics: Molten Egg, Toxic Egg, and Frozen Egg
- Small unblocked attack damage relic: The Boot
- Power-play healing relic: Bird-Faced Urn
- No-attack-turn energy relic: Art of War
- Card reward choice relic: Question Card
- Curse-prevention relic: Omamori
- Elite-combat strength relic: Sling of Courage
- Floor-entry gold relic: Maw Bank
- Rest-site energy relic: Ancient Tea Set
- Block-retention relic: Calipers
- Stateful relic: Ice Cream preserves energy between turns
- Potion identity fix: Gambler's Brew uses discard/draw selection instead of local gold RNG

### Verification (Milestone 12 + 19)
- CommunicationMod trace importer (`sts_verify`)
- Canonical observed-state normalizer for combat/run JSON
- `sts_verify` CLI: `trace`, `diff`, `parity`, `corpus`
- Observed-state sim-vs-real verifier for captured CommunicationMod traces
- Seed-start verifier mode parses `START IRONCLAD 0 VERIFY01` and verifies the captured trace through return to map with `seed_start.expected_failure=false`
- Manual corpus: milestone1, cultist bash step, known divergence list
- Regression corpus includes `trace-2026-06-18T16-50-50-232Z.jsonl` (CODEX04 controller trace)
- Nightly parity script: `scripts/nightly_parity.ps1`
- Observed-state verifier hygiene (Milestone 19):
  - unmapped combat/reward cards classify as unsupported instead of shifting indices
  - `PLAY n` no-target commands work for mapped no-target cards such as Dramatic Entrance
  - combat comparison uses the first living monster, not slot 0
  - unsupported monster-turn AI names monster groups (for example `AcidSlime_M`, `FuzzyLouseDefensive`)
  - reward `CHOOSE n` preserves observed choice indices when some reward options are unmapped
  - deck comparisons are partial when the observed deck contains unmapped cards
- Seed-start Neow coverage (Milestone 21):
  - `VERIFY01` verifies the captured Toy Ornithopter Neow branch through return to map; this trace carries the relic but does not observe a potion-use transition, so Toy Ornithopter healing remains covered by focused `sts_core` unit tests
  - `CODEX04` verifies talk, colorless reward choices, Dramatic Entrance pickup, leaving Neow, and the first captured map choice into a 54/54 HP Cultist
  - unchosen Neow branches remain explicitly classified

Run the VERIFY01 captured-trace verifier with:

```powershell
cd simulator
cargo run -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

Run the CODEX04 observed-state verifier with:

```powershell
cd simulator
cargo run -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
```

Expected result: `unexpected_diffs=0` with unsupported items named for seed-start gaps, unmapped cards, draw/shuffle scope, and unsupported monster groups.

Run the seed-start RNG harness with:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

Expected result: `seed_start.expected_failure=false`, `seed_start.first_boundary.path=$.actions[complete]`, and `unexpected_diffs=0`.

Run the CODEX04 seed-start Neow harness with:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
```

Expected result: `unexpected_diffs=0`, `seed_start.expected_failure=false`, verified labels through floor-3 combat completion and return-to-map steps, and `seed_start.first_boundary.path=$.actions[complete]`.

Current fidelity limit: VERIFY01, CODEX04, and CODEX03 seed-start traces pass with `unexpected_diffs=0` through their declared completion boundaries (CODEX03 ends after floor-3 return-to-map; CODEX04 after floor-3 combat completion). The TEST trace passes through Act 1 boss relic pickup and pre-Act-2 map return, and is now the first entry in the M35 Act 1 corpus manifest. Post-reward map returns are simulation-driven from captured map topology. M34 removed selected opening-pile and selected post-END pile restoration for the covered traces, but broad non-pile post-END observed sync remains interim scaffolding outside the selected M34 checks. M35 still needs additional clean full Act 1 traces and removal of remaining observed-state restoration inside the selected corpus scope before completion.

Milestone 28 is complete on the TEST trace (`trace-2026-06-21T09-57-10-380Z.jsonl`). Shop inventory at entry (step 168) and shop purchase/purge through step 176 are source-backed: class-card prices use library rarity with target-style `(int)(base * factor)` truncation, colorless prices use `AbstractCard.getPrice` bases (50/75/150) with the 1.2 multiplier, `affordable_shop_picks` drives CommunicationMod `choice_list` and `CHOOSE` index mapping, and membership/sale pricing matches captured gold. Full seed-start parity reports `unexpected_diffs=0` through Act 1 boss relic return-to-map (`test_seed_start_full_act1_boss_relic_prefix`); nightly includes this trace.

Milestone 27 is complete for the same TEST trace through Act 1 boss relic pickup and pre–Act-2 map return. Coverage includes events, normal/elite combats, rest/treasure/shop rooms, potion/hand-select/reward flows, Guardian boss combat (observed-state sync), boss chest, and Cursed Key boss relic reward.

Milestone 29 is in progress. The TEST trace elite/boss slice has a passing guard test, `test_seed_start_m29_test_elite_boss_without_observed_sync`, with elite/boss observed-state restoration disabled. This slice covers Lagavulin sleep/Metallicize block, wake-on-HP-damage, player vulnerable, Regret end-turn damage, Demon Form/Thunderclap trace playability, Gremlin Nob coverage in the TEST route, Guardian mode-shift scaffolding, and Act 1 boss relic return through the M27/M28 verifier path. Important carve-out: the TEST Lagavulin fight uses Power Potion; the in-combat potion reward, temporary zero-cost card, and downstream potion-tainted combat state still sync from observed state and are not yet a full card/potion parity claim. M29 is not complete until a structurally complete Sentries seed-start trace is captured and verified. The overnight collector produced a structurally valid Sentries run, `trace-2026-06-23T02-56-19-245Z.run2.valid-prefix.jsonl`, which reaches floor 7 Sentries. `trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl` removes 49 no-progress card-reward skip/reopen pairs from that run. Seed-start verification now supports its captured transform-card Neow branch, Sever Soul, Uppercut, floor-2 lethal Bash sequencing where Vulnerable follows lethal damage, the captured The Ssssserpent event branch, the Sentries elite reward sequence, and the following Blue Slaver combat/reward prefix. On the cleaned trace it verifies all 225 actions with `unexpected_diffs=0`; the only remaining boundary is `missing_post_reward_boundary` because the trace ends on a reward screen before a final `PROCEED`. Corpus and nightly parity now run this cleaned trace as an expected-failing boundary regression, not as M29 completion evidence.

### Tests
- `test_seed_start_m28_shop_entry_parity`, `test_seed_start_full_act1_boss_relic_prefix`, and `test_seed_start_m29_test_elite_boss_without_observed_sync` pass on `trace-2026-06-21T09-57-10-380Z.jsonl`
- Focused monster acceptance: `cargo test -p sts_core --test milestone6` passes
- Full-suite checks pass: `cargo test -p sts_core -- --test-threads=1` and `cargo test -p sts_verify --test corpus -- --test-threads=1`
- Latest direct potion action evidence slice checks: target jar class/constant-pool/string inspection with PowerShell zip reads; `jar` and `javap` were not on PATH. Focused checks: `cargo fmt`, `cargo test -p sts_core potion -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest simple upgraded/basic Ironclad evidence cleanup checks: target jar class/constant-pool inspection with PowerShell zip reads; `jar` and `javap` were not on PATH. Focused checks: `cargo test -p sts_core battle_trance`, `cargo test -p sts_core seeing_red`, `cargo test -p sts_core inflame`, `cargo test -p sts_core flex`, `cargo test -p sts_core spot_weakness`, `cargo test -p sts_core whirlwind`, `cargo test -p sts_core searing_blow`, `cargo test -p sts_core sentinel`, and `cargo test -p sts_core --test m32a_matrix`.
- Latest run-world evidence backfill checks: target jar class/constant-pool inspection with PowerShell zip reads; `jar`/`java` were not on PATH. Focused checks: `cargo fmt`; `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`; `cargo test -p sts_core slime_boss -- --test-threads=1`; `cargo test -p sts_core guardian -- --test-threads=1`; `cargo test -p sts_core golden_shrine -- --test-threads=1`; `cargo test -p sts_core cleric -- --test-threads=1`; and `cargo test -p sts_core ascension -- --test-threads=1`.
- Latest status/curse evidence cleanup checks: target jar class/constant-pool inspection with PowerShell zip reads; `java`, `jar`, and `javap` were not on PATH. Focused checks: `cargo test -p sts_core wound -- --test-threads=1`, `cargo test -p sts_core dazed -- --test-threads=1`, `cargo test -p sts_core burn -- --test-threads=1`, `cargo test -p sts_core regret -- --test-threads=1`, `cargo test -p sts_core doubt -- --test-threads=1`, `cargo test -p sts_core slimed -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Finesse/Good Instincts source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core good_instincts -- --test-threads=1`, `cargo test -p sts_core finesse -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest Swift Strike / Flash of Steel no-roll source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core swift_strike -- --test-threads=1`, `cargo test -p sts_core flash_of_steel -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Target jar evidence was limited to PowerShell zip/constant-pool inspection because `jar`, `java`, and `javap` were not on PATH in this environment.
- Latest passive potion evidence slice checks: `cargo fmt`, `cargo test -p sts_core fairy -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest Blind/Trip/Dark Shackles/Sadistic Nature source-evidence checks: target jar class/constant-pool inspection with PowerShell zip reads; `cargo fmt`; `cargo test -p sts_core blind_triggers_sadistic -- --test-threads=1`; `cargo test -p sts_core trip_champion_belt_weak_triggers_sadistic -- --test-threads=1`; `cargo test -p sts_core dark_shackles_strange_spoon -- --test-threads=1`; and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest live trace fidelity slice checks: `cargo fmt`, `cargo test -p sts_core liquid_memories --lib`, `cargo test -p sts_core guardian_mode_shift --lib`, `cargo test -p sts_core guardian_sharp_hide_damage_flushes_after_attack_card_resolves --lib`, `cargo test -p sts_verify guardian_twin_slam_attack_buff_imports_two_hit_attack --lib`, `uv run python -m unittest python.tests.test_self_play.SelfPlayTests.test_observed_combat_card_reward_choices_normalize_compact_ironclad_ids`, `cargo clippy -p sts_core` (existing warnings), `cargo clippy -p sts_verify` (existing warnings), `uv run maturin develop --release`, and strict replay of `trace-2026-07-01T20-30-26-163Z.jsonl` (`verified=True`, `trace_exhausted`). Source-backed fixes cover Liquid Memories one-choice auto-confirm plus turn-only zero cost, Guardian Sharp Hide timing after card/selection resolution, Guardian Thorns damage feeding Mode Shift, Guardian Mode Shift threshold growth, Guardian Twin Slam observed intent import, and compact Ironclad reward-card id normalization. This slice is generic simulator/verifier behavior only; no seed- or trace-specific production branches were added.
- Latest monster Artifact debuff-helper slice checks: `cargo fmt`, `cargo test -p sts_core artifact -- --test-threads=1`, `cargo test -p sts_core blind_artifact -- --test-threads=1`, `cargo test -p sts_core trip_artifact -- --test-threads=1`, `cargo test -p sts_core dark_shackles_artifact -- --test-threads=1`, `cargo test -p sts_core potion_consumes_monster_artifact -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, `cargo test -p sts_verify -- --test-threads=1`, `cargo clippy -p sts_core` (existing warnings), and `cargo clippy -p sts_verify` (existing warnings). Adds serialized monster Artifact state, routes shared monster Weak/Vulnerable/Strength-down actions through Artifact consumption, covers Blind/Trip/Dark Shackles/Sadistic Nature blocked-debuff behavior, parses observed monster Artifact, and routes Fear/Weak potion Weak application through the same Artifact helper while keeping the no-global-`ApplyPowerAction`/no-trace-parity caveat.
- Latest Dark Shackles slice checks: `cargo fmt`, `cargo test -p sts_core dark_shackles -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, and `cargo clippy -p sts_core` (existing 8 warnings).
- Latest Deep Breath/Impatience source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core deep_breath -- --test-threads=1`, `cargo test -p sts_core impatience -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Coverage adds focused Strange Spoon no-roll regressions for both non-exhausting source cards and promotes matrix evidence only to source-class/action/count evidence while keeping exact Deep Breath shuffle/order and Havoc/top-draw caveats local.
- Latest Enlightenment source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core enlightenment -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds focused Strange Spoon non-exhaust/no-roll coverage, confirms base turn-only versus upgraded combat-long hand cost reduction with unit tests plus target jar constant-pool evidence for `EnlightenmentAction` cost fields, and promotes only the narrow source/action/cost surface without a played-card trace-parity claim.
- Latest Forethought slice checks: `cargo fmt`, `cargo test -p sts_core forethought -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Jack Of All Trades slice checks: `cargo fmt`, `cargo test -p sts_core jack_of_all_trades -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Madness slice checks: `cargo fmt`, `cargo test -p sts_core madness -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Mind Blast slice checks: `cargo fmt`, `cargo test -p sts_core mind_blast -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`. The row remains `unit_only`; the latest regression explicitly pins that local Mind Blast counts combat-only generated cards in current combat piles, which can diverge from target master-deck-size damage.
- Latest Master of Strategy slice checks: `cargo fmt`, `cargo test -p sts_core master_of_strategy -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Previous Trip slice checks: `cargo fmt`, `cargo test -p sts_core trip -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, and `cargo clippy -p sts_core` (existing 8 warnings).
- Previous Panacea slice checks: `cargo fmt`, `cargo test -p sts_core panacea -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), and `cargo test -p sts_verify -- --test-threads=1`.
- Previous Blind slice checks: `cargo fmt`, `cargo test -p sts_core blind -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, and `cargo clippy -p sts_core` (existing 8 warnings).
- Previous Flash of Steel slice checks: `cargo fmt`, `cargo test -p sts_core flash_of_steel -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, and `cargo test -p sts_core -- --test-threads=1`.
- Latest Golden Shrine / The Cleric event evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core generated_golden_shrine -- --test-threads=1`, `cargo test -p sts_core cleric -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds only unit evidence for Golden Shrine generated/legacy branch routing and Cleric heal/remove-curse branch boundaries; no trace parity is claimed.
- Previous Bandage Up source-evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core bandage_up -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds focused Strange Spoon source-exhaust regression and promotes the matrix row only for source-backed heal/action/source-exhaust evidence while retaining the no trace-parity caveat.
- Latest Dramatic Entrance source-evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core dramatic_entrance -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds focused event-log coverage for the `DealDamageAll` source/action surface and keeps the existing Strange Spoon source-exhaust destination/counter regression; row promotion is limited to constant-pool/source-class action evidence plus local unit coverage, with no broad played-card CommunicationMod trace parity claim.
- Latest random/generated/complex evidence slice checks: target jar class/constant-pool inspection with PowerShell zip reads; `javap` was not on PATH. Focused checks: `cargo test -p sts_core sword_boomerang -- --test-threads=1`, `cargo test -p sts_core infernal_blade -- --test-threads=1`, `cargo test -p sts_core shockwave -- --test-threads=1`, `cargo test -p sts_core forethought -- --test-threads=1`, `cargo test -p sts_core mind_blast -- --test-threads=1`, `cargo test -p sts_core panache -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Final Mind Blast/Panache audit reran the targeted class inventory check and the focused `mind_blast`, `panache`, and `m32a_matrix` tests. The safe slice promotes only narrow source/action surfaces for Sword Boomerang, Infernal Blade, Shockwave, and Forethought while keeping Mind Blast and Panache `unit_only` for the documented master-deck and timing gaps.
- Latest Distilled Chaos evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core distilled_chaos -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds a mixed-pile regression for actual top-draw ordering, unplayable top-card no-target/no-RNG behavior, and targeted-card `cardRandomRng` consumption; follow-up target jar constant-pool evidence promotes the row only to `source_and_unit` for the potion/action/top-draw/random-target surface, with no exact action-manager instruction-order or played-potion CommunicationMod trace parity claim.
- Latest power rare card evidence audit checks: `cargo test -p sts_core corruption -- --test-threads=1`, `cargo test -p sts_core barricade -- --test-threads=1`, `cargo test -p sts_core berserk -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Target jar evidence was limited to PowerShell zip/constant-pool inspection; Corruption, Barricade, and Berserk are promoted only for narrow card-play/power surfaces, with no exact action-manager timing or played-card CommunicationMod trace parity claim.
- Latest rest action source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core rest_actions_transition_to_expected_phase_reward_or_grid_destinations -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Source evidence was limited to target jar class/constant-pool inspection because `javap` was not on PATH; no broad rest UI parity or CommunicationMod trace parity is claimed.
- Latest Entropic Brew evidence slice checks: `cargo fmt`, `cargo test -p sts_core entropic_brew -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Coverage pins consume-before-fill local refill order, exact generated potion sequence/counter, full ordinary belt refill, Sozu no-roll/no-fill, and Potion Belt capacity while retaining the no played-potion trace parity caveat.
- Latest Offering/Reaper source-evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core offering -- --test-threads=1`, `cargo test -p sts_core reaper -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Target jar evidence was limited to PowerShell zip/constant-pool inspection because `jar` and `javap` were not on PATH; no played-card CommunicationMod trace parity is claimed.
- Latest Limit Break / Fire Breathing / Evolve source-evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core limit_break -- --test-threads=1`, `cargo test -p sts_core fire_breathing -- --test-threads=1`, `cargo test -p sts_core evolve -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Target jar evidence was limited to PowerShell zip/constant-pool inspection because `javap` was not on PATH; no played-card CommunicationMod trace parity is claimed.
- Latest Exhume/Discovery/Madness evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core exhume -- --test-threads=1`, `cargo test -p sts_core discovery -- --test-threads=1`, `cargo test -p sts_core madness -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Target jar evidence was limited to PowerShell zip/constant-pool inspection because `javap` was not on PATH; rows promote only narrow select/random/action surfaces, with no played trace parity claim.
- Latest Juggernaut/Brutality evidence slice checks: `cargo test -p sts_core juggernaut`, `cargo test -p sts_core brutality`, and `cargo test -p sts_core --test m32a_matrix`. Target jar evidence was limited to PowerShell zip/constant-pool inspection because `jar` and `javap` were not on PATH; rows promote only narrow power/action surfaces, with no played-card CommunicationMod trace parity claim.
- Latest HP-loss/damage-growth attack evidence slice checks: target jar class/constant-pool inspection with PowerShell zip reads; `cargo test -p sts_core hemokinesis -- --test-threads=1`; `cargo test -p sts_core bloodletting -- --test-threads=1`; `cargo test -p sts_core blood_for_blood -- --test-threads=1`; `cargo test -p sts_core rampage -- --test-threads=1`; `cargo test -p sts_core carnage -- --test-threads=1`; `cargo test -p sts_core dropkick -- --test-threads=1`; `cargo test -p sts_core flame_barrier -- --test-threads=1`; and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Rows promote only narrow target card/action/power surfaces plus local unit coverage, with no played-card CommunicationMod trace parity claim.
- Latest Ironclad select/exhaust skills evidence slice checks: target jar constant-pool inspection with PowerShell zip reads; `jar`/`javap` were not on PATH. Focused checks: `cargo test -p sts_core havoc -- --test-threads=1`, `cargo test -p sts_core warcry -- --test-threads=1`, `cargo test -p sts_core dual_wield -- --test-threads=1`, `cargo test -p sts_core burning_pact -- --test-threads=1`, `cargo test -p sts_core armaments -- --test-threads=1`, `cargo test -p sts_core headbutt -- --test-threads=1`, `cargo test -p sts_core second_wind -- --test-threads=1`, `cargo test -p sts_core entrench -- --test-threads=1`, `cargo test -p sts_core disarm -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest starter/common simple attack/block evidence slice checks: target jar constant-pool inspection with PowerShell zip reads; `cargo fmt` from `simulator/`; `cargo test -p sts_core strike -- --test-threads=1`; `cargo test -p sts_core defend -- --test-threads=1`; `cargo test -p sts_core bash -- --test-threads=1`; `cargo test -p sts_core anger -- --test-threads=1`; `cargo test -p sts_core cleave -- --test-threads=1`; `cargo test -p sts_core twin_strike -- --test-threads=1`; `cargo test -p sts_core shrug_it_off -- --test-threads=1`; `cargo test -p sts_core true_grit -- --test-threads=1`; `cargo test -p sts_core pommel_strike -- --test-threads=1`; `cargo test -p sts_core sever_soul -- --test-threads=1`; `cargo test -p sts_core thunderclap -- --test-threads=1`; `cargo test -p sts_core uppercut -- --test-threads=1`; `cargo test -p sts_core immolate -- --test-threads=1`; and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest Ironclad power/effect evidence slice checks: target jar class/constant-pool inspection with PowerShell zip reads; `javap` was not on PATH. Passing checks: `cargo test -p sts_core --test m32a_matrix`, `cargo test -p sts_core --lib feel_no_pain`, `cargo test -p sts_core --lib dark_embrace`, and `cargo test -p sts_core --lib metallicize`. Additional focused filters for `demon_form`, `combust`, `rupture`, and `rage` were blocked by a concurrent compile error in `simulator/crates/sts_core/src/combat/transition.rs` test code (`ContentId` subtraction in the Immolate regression), so this slice makes no source edits beyond docs/status.
- Nightly parity (`scripts/nightly_parity.ps1`) passes including TEST seed-start

## Current Captured Controller Trace

`verification/corpus/communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl` imports successfully with 42 states and 41 actions. Observed-state parity verifies floor 1–3 combat (Cultist, Small Slimes, 2 Louse), Dramatic Entrance, Battle Trance path cards, multiple `END` turns, and reward screens with `unexpected_diffs=0`. Unsupported commands are classified for Neow/map/seed-start gaps only.

`verification/corpus/communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl` (CODEX03) seed-start replay covers Neow's Lament, three combats (Jaw Worm, Cultist, 2 Louse), simulation-driven rewards/map returns, and ends after floor-3 return-to-map with `unexpected_diffs=0`.

## Next Task

Milestone 32A is complete. The inventory is split across `simulator/docs/content_support_matrix.md`, `simulator/docs/m32a_cards_matrix.md`, `simulator/docs/m32a_relic_potion_matrix.md`, and `simulator/docs/m32a_run_world_matrix.md`; `simulator/crates/sts_core/tests/m32a_matrix.rs` now fails when known Ironclad A0 content or named run-world surfaces are missing from the matrices.

Current milestone: Milestone 33, Neow generalization. Latest monster RNG audit work has source-shaped the Donu/Deca boss-pair surface: Beyond boss selection can produce the source boss-list shuffle result, Donu/Deca construction uses Deca then Donu with fixed 250/265 HP and A19 Artifact 3, Deca opens Beam after one ignored AI roll, Donu opens Circle/strength after one ignored AI roll, and source move bytes are recorded. Focused and full `sts_core` library tests pass; clippy passes with the existing warning backlog. Milestone 32B's deterministic card completion sweep is complete for the known Ironclad A0/card-pool rows in `simulator/docs/m32a_cards_matrix.md`; remaining `placeholder` card rows are mechanic-test fixtures or non-A0/special curse surfaces, not unimplemented Ironclad A0 card sweep work. Milestone 32C evidence backfill has promoted all safe currently evidenced high-risk surfaces while leaving explicit caveats where source/trace evidence is insufficient: Mind Blast/Panache timing/count gaps, unproved potion/relic identity rows, A1/A7/A20 ascension caveats, and contaminated/unverified CommunicationMod traces. Latest M33 slices implement source-backed Neow option generation, forced-rare card rewards, split-stream colorless rewards, three-potion rewards, fixed-tier common/rare relic rewards, transform identity generation, boss-swap helper application, Neow grid opening/confirmation, simple immediate rewards/drawbacks, and narrow curse verifier paths behind the `sts_core::run::neow` facade, including the source-possible `Curse + TransformTwoCards` grid branch. Seed-start verification now uses generated Neow option labels and generated identities for the exercised CODEX04/TEST colorless rewards, VERIFY01 common relic, M290001/M290008 transform replacements, MANUAL01 immediate rare-card reward, and helper/synthetic rare-relic, rare-colorless, grid, potion, curse-transform, and boss-swap branches. Current boss-swap follow-up slices include Calling Bell queued relic rewards, Astrolabe, Pandora's Box, Empty Cage, and Tiny House reward-screen opening/skip/pick; the current Ironclad A0 boss relic pool has no known unsupported initial boss-swap outcomes, but branch combinations and selected-trace coverage remain caveated where only helper/synthetic tests exercise them.

Next task: exercise the next caveated Neow branch with explicit RNG counters and source-backed trace comparison where possible, likely selected real-trace coverage for remove-two/upgrade grids, `THREE_RARE_CARDS`, rare-colorless, rare-relic, potion, supported curse combos, or boss-swap branches. Remaining implementation gaps are narrower: selected-trace evidence for implemented boss-swap follow-ups and branch-combination coverage still need follow-up beyond synthetic/helper tests. Do not preserve named-seed behavior with implementation branches; keep trace labels as fixtures only and fix the generic simulator/verifier mechanics that explain them.

Milestone 32 is complete. Completed relic slices now cover simple pickup/capacity relics, start-of-combat relics, first-attack damage relics, first-HP-loss draw relics, card-play counter relics, turn-timed combat relics, combat-victory healing relics, room/rest healing relics, damage mitigation relics, shop/economy relics, boss-entry relics, debuff-immunity relics, boss energy relics, boss conditional-energy relics, boss energy/enemy-strength relics, boss energy/gold-lockout relics, combat-healing multiplier relics, Vulnerable synergy relics, elite HP relics, elite-combat strength relics, floor-entry gold relics, rest-site energy relics, block-retention relics, reward-screen max-HP relics, X-cost relics, curse synergy relics, boss energy/rest-restriction relics, boss energy/potion-lockout relics, potion potency relics, boss energy/card-reward relics, boss energy/card-limit relics, boss energy/randomized-draw relics, hand-retention relics, information-only relics, rest removal relics, rest strength relics, rest dig relics, debuff-cleanse relics, Strike-card damage relics, potion reward guarantee relics, pickup upgrade relics, potion-use healing relics, card-add upgrade relics, small unblocked attack damage relics, power-play healing relics, no-attack-turn energy relics, shop start-turn strength relics, card reward choice relics, card reward count relics, curse-prevention relics, shuffle-trigger relics, monster-death relics, shuffle-counter relics, exhaust-damage relics, unplayable-card relics, one-shot revive relics, previous-turn card-count relics, block-break attack relics, Buffer relics, elite bonus-reward relics, chest bonus-reward relics, pickup removal/card-reward relics, bottled-card relics, card-copy relics, random-potion pickup relics, boss pickup bundle relics, random-card-on-exhaust relics, power-play cost relics, hand-empty draw relics, persistent turn-counter relics, chest-curse boss-energy relics, event-room replacement relics, exhaust-retention relics, map-jump relics, boss pickup multi-relic queue relics, starter-transform boss relics, off-character starter/fallback no-op relics, and starter/fallback no-op relics: Blood Vial, Pear, Mango, Old Coin, Lee's Waffle, Potion Belt, Lantern, Bag of Preparation, Bag of Marbles, Bronze Scales, Thread and Needle, Red Skull, Nunchaku, Art of War, Shuriken, Kunai, Letter Opener, Happy Flower, Orichalcum, Horn Cleat, Captain's Wheel, Mercury Hourglass, Stone Calendar, Black Blood, Meat on the Bone, Meal Ticket, Regal Pillow, Dream Catcher, Eternal Feather, Torii, Tungsten Rod, Ceramic Fish, Membership Card, Smiling Mask, Maw Bank, Ancient Tea Set, Calipers, Singing Bowl, Chemical X, Philosopher's Stone, Slaver's Collar, Snecko Eye, Ectoplasm, Runic Dome, Strike Dummy, Brimstone, Akabeko, Centennial Puzzle, Pen Nib, Self-Forming Clay, Clockwork Souvenir, Runic Cube, The Abacus, Gremlin Horn, Sundial, Charon's Ashes, Blue Candle, Medical Kit, Lizard Tail, Pocketwatch, Hand Drill, Burning Blood, Circlet, Red Circlet, White Beast Statue, Whetstone, War Paint, Pantograph, Ginger, Turnip, Mark of Pain, Magic Flower, Paper Phrog, Champion Belt, Preserved Insect, Sling of Courage, Darkstone Periapt, Du-Vu Doll, Fusion Hammer, Sozu, Sacred Bark, Busted Crown, Velvet Choker, Runic Pyramid, Frozen Eye, Peace Pipe, Girya, Orange Pellets, Toy Ornithopter, Molten Egg, Toxic Egg, Frozen Egg, The Boot, Bird-Faced Urn, Question Card, Prayer Wheel, Cracked Core, Frozen Core, Pure Water, Holy Water, Ring of the Snake, Ring of the Serpent, Omamori, Unceasing Top, Shovel, Fossilized Helix, Black Star, Matryoshka, Empty Cage, Bottled Flame, Bottled Lightning, Bottled Tornado, Dolly's Mirror, Orrery, Cauldron, Tiny House, Dead Branch, Mummified Hand, Strange Spoon, Wing Boots, Calling Bell, Pandora's Box, Astrolabe, Juzu Bracelet, Prismatic Shard, The Courier's source-backed shop discount, purge-cost, and card/relic/potion restock hooks, Incense Burner's persistent sixth-turn Intangible hook, Cursed Key's pickup energy plus non-boss chest curse hook, Tiny Chest's persistent fourth-`?` treasure replacement hook, Snecko Eye's pickup energy plus `cardRandomRng` opening/turn draw cost-randomization hooks, Strange Spoon's source-backed `cardRandomRng.randomBoolean()` played-card exhaust-to-discard hook, Wing Boots' three-charge same-next-floor map jump hook, Calling Bell's source-backed Curse of the Bell confirmation grid plus common/uncommon/rare screenless relic reward queue, and Pandora's Box's source-backed starter Strike/Defend removal plus `cardRandomRng` replacement confirmation grid, Astrolabe's source-backed three-card `miscRng` transform/auto-upgrade grid, Gambling Chip's start-of-combat multi-discard/redraw selection hook, Toolbox's start-of-combat colorless-card choice hook, Juzu Bracelet's source-backed `?` room monster-outcome conversion with persistent event-room chance counters, and Prismatic Shard's source-backed combat reward any-color card pool hook with the extra `cardRng.randomLong()` per pick. Acceptance evidence: all modeled relic keys promote without key-only placeholders, focused relic tests pass, relic counters round-trip through run/combat state, and the relic-heavy corpus traces pass seed-start verification.

The previous M29 cleaned single-run prefix can still be structurally checked with:

```powershell
node tools\communication\trace_tools.js validate verification\corpus\communication_mod\trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl
```

Current seed-start verifier result for that prefix: all 225 actions verify with `unexpected_diffs=0`; `seed_start.expected_failure=true` only because the cleaned trace ends on the final reward screen before a post-reward `PROCEED`. The corpus test and nightly parity script include this exact expected-failing classification to prevent regressions while keeping M29 blocked on new trace data.

Milestone 32C/M29 verifier feasibility check: `trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl` structurally validates (`actions=548`, `max_floor=37`, elite and boss coverage, terminal map floor 37) but seed-start parity verifies only the bootstrap action, then reports `unexpected_diffs=1` at Neow (`$.actions[step=3].command`, `unexpected_seed_start_command`, MANUAL01 Neow choices differ from the modeled captured branches). `trace-2026-06-23T07-42-06-085Z.best-run.jsonl` structurally validates as a boss-reaching trace but seed-start currently stops at step 73 on unsupported multi-enemy Sword Boomerang random targeting. No existing untracked/cleaned trace found in this pass can advance M29 completion or a 32C targeted real-trace claim without new generic verifier support. Recommended next collection shape: capture a fresh single-run Ironclad A0 trace, drive every action through the trace UI, avoid manual in-game clicks, include the final `PROCEED` after the Sentries/target reward boundary, and for 32C card claims isolate one target surface per short trace with the branch supported by generic seed-start mechanics before the target card appears.

Overnight collector hardening after the `M290001` run:

- `overnight_collector.js` rejects commands whose verb is not currently listed in `available_commands`.
- repeated identical commands on unchanged state fall back conservatively (`SKIP`, `PROCEED`, `LEAVE`, or `state`) and then exit instead of spamming forever.
- stale bridge/session files make the collector exit with a clear idle reason, letting the supervisor validate the partial trace instead of waiting forever.
- `overnight_supervisor.js` runs the collector in restart loops, validates the current trace after collector exit or stale-session startup, writes a `.valid-prefix.jsonl` salvage file when a trace is missing an action response, writes a `.best-run.jsonl` extracted keeper from valid traces, updates `tools/communication/session/harvest_report.json`, logs compact harvest-quality and best-run lines, and stops with a clear stale-session/bridge-exited reason when STS or CommunicationMod needs manual recovery.
- `overnight_collector.test.js` covers the known policy regressions: full potion belt reward, repeated card reward fallback, unavailable commands, living target selection, and state-signature changes. `overnight_supervisor.test.js` covers stale-session and trace-quality formatting without requiring a live STS process.
- `trace_tools.js validate` now reports starts, seeds, room path, encounters, deaths, terminal state, elite/boss room coverage, and a simple harvest score for harvested traces. `trace_tools.js report` adds per-run summaries and best-run selection for multi-run overnight captures, and `extract-best-run` materializes the highest-scoring run as a verifier-ready single-run trace.
- `harvest_status.js` is the non-mutating status check for the latest `harvest_report.json`; it validates referenced raw, valid-prefix, and best-run artifacts without creating or rewriting trace files.
- `overnight_preflight.js` checks for stale session files, pending `next_command.txt`, bridge-exited status, and sent-command/newer-than-summary mismatches before starting an overnight supervised run.
- `run_overnight_preflight.cmd` and `run_communication_checks.cmd` provide one-command Windows entry points for preflight and communication-tool regression checks.
- `run_overnight_guarded.cmd` is the safer overnight entry point: it runs preflight and starts the supervisor only when the bridge/session is fresh.
- The overnight collector map policy scores currently visible room choices deterministically, preferring elites, fights, chests, events, shops, then rests. It intentionally does not claim route lookahead until the bridge exposes enough stable map-node context for that.
- The overnight collector combat policy now has a small survival bias: when low HP faces heavy incoming damage, defensive cards outrank basic attacks. Transient choose-capable screens with no parsed choices now poll state instead of sending `CHOOSE 0`.
- `bridge_probe.js` is the active bridge liveness check for overnight setup. It writes one temporary `state` command, verifies whether CommunicationMod consumes it, and removes the probe command on failure so stale sessions do not poison the next launch.
- `trace_client.js`, `summary.json`, and `status.json` now include `client_pid`, which exposed duplicate bridge clients during live collection. Before overnight collection there should be exactly one active bridge client consuming commands.
- `overnight_collector.js` persists a pending `START` guard so it cannot send a second seed while the previous start transition is still awaiting an in-game confirmation.
- Live collection on 2026-06-23 produced `trace-2026-06-23T07-42-06-085Z.jsonl`. The raw trace validates at completed boundaries and, as of the latest snapshot, contains 3 starts (`M290005`..`M290007`), 378 completed actions, max floor 10, 3 elite-room entries, 2 deaths, shop/rest/chest/event coverage, and an active floor-7 elite fight. `trace-2026-06-23T07-42-06-085Z.best-run.jsonl` is valid and extracts `M290006`: 105 actions, max floor 10, 1 elite, terminal death.
- Live fixes from that run: `SHOP_ROOM` now sends `PROCEED` instead of reopening the shop after `LEAVE`, and `HAND_SELECT` now chooses then confirms required card selections. These prevent the observed shop reopen loop and Warcry hand-select polling stall.
- The same 2026-06-23 raw trace later produced a stronger best-run extraction, `trace-2026-06-23T07-42-06-085Z.best-run.jsonl`, selecting `M290008` / numeric seed `40560393133`. It is structurally valid with 193 actions, max floor 16, boss room coverage, and terminal `in_progress` inside Hexaghost combat. Milestone 30 now verifies the seed-start early-Act-1 slice through step 99 with `verified=99`, `unexpected_diffs=0`, and first boundary `$.actions[step=100].command` because the verifier intentionally stops after the treasure-to-map boundary. Coverage includes the captured transform-card Neow branch (`Sentinel`), floors 1-2 combats/rewards, Scrap Ooze success, The Ssssserpent, Sword Boomerang in the floor-5 combat, captured Looter escape-to-reward, rest, and treasure. Remaining M290008 support is explicitly captured-slice scoped: broad Neow RNG, Scrap Ooze success RNG, transformed-card opening pile generalization, Sword Boomerang random targeting, Looter escape AI, and later Act 1 rooms remain future work.

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-21T09-57-10-380Z.jsonl
```

Expected result: `unexpected_diffs=0`, `seed_start.expected_failure=false`, verified labels through shop purchase/purge and Act 1 boss relic return-to-map.

## Milestone 28 Notes

Milestone 28 is complete on `trace-2026-06-21T09-57-10-380Z.jsonl`. Shop inventory, purchase, purge, and affordable choice-list refresh are source-backed through step 176. Key model pieces: `shop_card_price_rarity` (library rarity for class cards), colorless `getPrice` bases with 1.2 multiplier, Java-style int truncation on class-card merchant rolls, and `affordable_shop_picks` for `CHOOSE` mapping. Corpus: `test_seed_start_m28_shop_entry_parity` (prefix through step 168) and `test_seed_start_full_act1_boss_relic_prefix` (full trace).

## Milestone 27 Notes

Milestone 27 is complete for `trace-2026-06-21T09-57-10-380Z.jsonl` (seed `TEST` / numeric `1_218_623`). Seed-start verifies through Act 1 boss relic pickup and pre–Act-2 map return with `unexpected_diffs=0`. Coverage includes events (Scrap Ooze, Big Fish), normal/elite combats, rest/treasure/shop rooms, potion/hand-select/reward flows, Guardian boss combat (observed-state sync), boss chest, and Cursed Key boss relic reward. The trace is in nightly parity (`scripts/nightly_parity.ps1`) and `sts_verify/tests/corpus.rs`.

## Milestone 26 Notes

Milestone 26 is complete. The scratch `_tmp_test.rs` debugging artifact was removed, nightly parity passed, and the M25 seed-start regression gate is ready to use as the clean baseline for M27.

## Milestone 25 Notes

VERIFY01, CODEX04, and CODEX03 seed-start traces pass with `unexpected_diffs=0` through their declared completion boundaries. Nightly parity (`scripts/nightly_parity.ps1`) runs all three. Use `sts_verify minimize` to produce prefix traces under `verification/corpus/bugs/` when debugging new failures. Seed-start hidden-state assumptions are documented in `VERIFICATION.md` (shuffle fallback, pile resync, UUID fields, deferred card reward, combat-entry `cardRng` +3).

## Milestone 24 Notes

Milestone 24 is complete for captured reward RNG and source-backed shop/event generation. Normal-combat and elite/chest/boss relic rewards use target-style RNG over persisted pools without corrupting `relic_rng_counter` after pool initialization. Shop generation mirrors `sts_lightspeed` `Shop.cpp` (7 cards, 3 relics, 3 potions, sale slot, remove pricing) with `relic_key`-only shop relic ownership. Act 1 events use target pool lists with shrine roll; Golden Shrine, Cleric heal, and Shining Light (HP cost + random upgrades) have implemented outcomes. Seed-start VERIFY01/CODEX04 reward verification is simulation-driven; nightly parity includes both traces. Captured shop/event/rest CommunicationMod traces are not in the passing nightly set. Unmapped shop colorless cards are RNG placeholders until mapped. Post-reward map-return pins and CODEX03 remain Milestone 25.

## Milestone 20 Notes

External seed conversion is source-backed from the target `SeedHelper.getLong(String)` bytecode in `desktop-1.0.jar`: uppercase, map `O` to `0`, parse in base 35 with alphabet `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`. Captured checks now pass for `VERIFY01`, `CODEX03`, and `CODEX04`, and seed-start CLI output includes `seed_start.numeric_seed`.

## Milestone 21 Notes

CODEX04 seed-start verification originally covered the captured Neow colorless-card branch: `START IRONCLAD 0 CODEX04`, talk, choose the colorless-card reward option, verify `Deep Breath` / `Dramatic Entrance` / `Jack Of All Trades`, pick `Dramatic Entrance`, and leave to the first map-choice screen with the card in the deck. Later M33 work now uses source-backed/generated Neow options and selected reward helpers for multiple Ironclad A0 surfaces; branch combinations and selected-trace coverage remain explicitly caveated where only helper or synthetic tests exercise them.

## Milestone 22 Notes

Milestone 22 is complete for the available captured evidence. Act 1 map, normal encounter selection, and monster spawn parity are source-backed for `VERIFY01`, `CODEX04`, and `CODEX03`. Full captured map topology/edges/room symbols match for all three seeds. Map-choice prefixes and chosen combat paths are pinned, including CODEX04 `[2, 3, 2]`, CODEX03 `[1, 0, 1]`, and VERIFY01 `[1, 2]` with captured nodes entering combat rooms. Normal encounter list generation covers weak/strong pools, first-strong exclusions, and no-repeat-last-two retries; room execution maps combat index to list entries via `normal_encounter_key_at_combat_index`. Target spawn state at combat entry covers Cultist, Jaw Worm, Small Slimes, and 2 Louse with floor-offset `monsterHpRng`, `miscRng` louse kind selection, and post-HP/bite Curl Up rolls from the decoded 3–7 range. Seed-start reports include `m22_encounter_report`; CODEX04 and CODEX03 each have three captured verified combat-entry rosters, while VERIFY01 has one captured verified entry plus two clearly separated source-backed predictions because that trace ends after the first combat reward. CODEX04 seed-start now reaches floor-3 combat completion; CODEX03 seed-start replays Neow's Lament through floor-3 return-to-map with `unexpected_diffs=0`.

## Milestone 23 Notes

Milestone 23 is complete for captured CODEX04/VERIFY01 scope. Observed-state and seed-start CODEX04 floor 1–3 combat parity pass with `unexpected_diffs=0`; END transitions are no longer draw/shuffle scope failures. Game-compatible pieces now in place: decoded Ironclad starter master-deck instance order and `shuffleRng(seed + floor)` opening piles (VERIFY01 pure; CODEX04 falls back to trace when innate/extra cards are present), top-of-pile draw semantics matching CommunicationMod bottom-first export, `StsRng` in-combat draws via `shuffle_rng`, deterministic slime/louse move cycles, and captured card mechanics for `Dramatic Entrance`, `Battle Trance`, and `Shrug It Off`. Post-END pile resync remains interim scaffolding until innate/extra-card master-deck ordering is fully decoded without trace fallback (M24 follow-up).
