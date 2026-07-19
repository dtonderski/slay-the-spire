# SlayTheData Event Auto-Play Status

Updated: 2026-07-10

## Result

All 52 simulator event variants now have an automatic SlayTheData path. None of the event mappings below depends on the removed generic choice-label fallback. Event names are matched through an exhaustive alias table, and event choices are resolved through event-specific source-metric mappings or a dedicated structural binder (Neow, N'loth, Match and Keep, Knowing Skull, and card grids).

“Complete” here means:

- the simulator has a dedicated event implementation and focused mechanics coverage;
- the recorded SlayTheData event name is recognized;
- recorded choice outcomes bind deterministically to a live action;
- staged Continue/Leave screens, card grids, rewards, and combat transitions are handled by the auto-play loop;
- the event family is represented in the exhaustive name test and the choice/outcome fixture, or in a dedicated structural regression.

## Coverage

| # | Event | Auto-play status | Binding evidence |
|---:|---|---|---|
| 1 | Neow | Complete | Dedicated talk, bonus/cost, reward/grid follow-up, and leave binders |
| 2 | AccursedBlacksmith | Complete | Forge/Rummage/Ignore mappings; upgrade grid |
| 3 | BonfireElementals | Complete | Offered-rarity mapping; offer grid and rarity result stages |
| 4 | Designer | Complete | Adjustments/Clean Up/Full Service/Punch mappings; multi-card grids |
| 5 | Duplicator | Complete | Copy mapping and obtain-card grid |
| 6 | FountainOfCleansing | Complete | `The Divine Fountain` alias; curse-removal mapping |
| 7 | GoldenShrine | Complete | Pray/Desecrate/Ignore mappings and final Leave flush |
| 8 | BigFish | Complete | Banana/Donut/Box mappings and Box reward/curse follow-up |
| 9 | TheCleric | Complete | Heal/Purify/Leave mappings; remove grid |
| 10 | DeadAdventurer | Complete | Search-count mapping, reward stages, and event combat transition |
| 11 | GoldenIdol | Complete | Take/Outrun/Smash/Hide outcome mappings and queued curse stage |
| 12 | WingStatue | Complete | `Golden Wing` alias; Destroy/Pray mapping and remove grid |
| 13 | WorldOfGoop | Complete | Gather/Leave-It mappings |
| 14 | TheSsssserpent | Complete | observed spelling alias; Agree/Disagree mappings |
| 15 | LivingWall | Complete | Forget/Change/Grow mappings and grids |
| 16 | HypnotizingColoredMushrooms | Complete | Fight/Heal mappings and event combat transition |
| 17 | ScrapOoze | Complete | repeated Search/Reach and Leave mappings |
| 18 | ShiningLight | Complete | Enter-Light/Leave mappings |
| 19 | FaceTrader | Complete | Touch/Trade/Leave mappings |
| 20 | Nloth | Complete | `N'loth` alias; lost-relic evidence selects the exact offered trade |
| 21 | NoteForYourself | Complete | `A Note For Yourself` alias; Take-and-Give/Ignore and grid stages |
| 22 | SecretPortal | Complete | Take-Portal/Leave mappings and boss-combat transition |
| 23 | TheJoust | Complete | owner/murderer bet mappings and staged result |
| 24 | WeMeetAgain | Complete | potion/gold/card/attack mappings and dynamic offers |
| 25 | TheWomanInBlue | Complete | 1/2/3/0-potion mappings and potion reward |
| 26 | Transmorgrifier | Complete | `Transmogrifier` alias; transform grid |
| 27 | Purifier | Complete | Purge mapping and card grid |
| 28 | UpgradeShrine | Complete | Upgrade mapping and card grid |
| 29 | WheelOfChange | Complete | all six outcomes, reward stages, and remove grid |
| 30 | MatchAndKeep | Complete | dedicated card-position binder, pair tracking, safe miss, Play/Leave |
| 31 | Addict | Complete | Buy/Steal/Leave mappings |
| 32 | BackToBasics | Complete | Elegance/Simplicity mappings and grids |
| 33 | Beggar | Complete | Give/Leave mapping and remove grid |
| 34 | Colosseum | Complete | Fight/Flee/Nobs mappings and combat/reward transitions |
| 35 | CursedTome | Complete | Read/Stop/Take-Book mappings and staged relic reward |
| 36 | DrugDealer | Complete | JAX/Test-Subject/Leave mappings and grid |
| 37 | ForgottenAltar | Complete | Idol/Blood/Smash mappings |
| 38 | Ghosts | Complete | `Council of Ghosts` alias; Accept/Refuse mappings |
| 39 | KnowingSkull | Complete | one source record expands into ordered Potion/Gold/Card/Leave clicks |
| 40 | MaskedBandits | Complete | Fight/Pay mappings and combat transition |
| 41 | Nest | Complete | `The Nest` alias; Smash-and-Grab/Stay-in-Line mappings |
| 42 | TheLibrary | Complete | Read/Sleep mappings and obtain-card grid |
| 43 | TheMausoleum | Complete | Open/Leave mappings and queued reward/curse stage |
| 44 | Vampires | Complete | `Vampires(?)` alias; Accept/Blood-Vial/Refuse mappings |
| 45 | Lab | Complete | Search mapping and indexed multi-potion reward |
| 46 | Falling | Complete | Skill/Power/Attack mappings and removal grid |
| 47 | MindBloom | Complete | War/Awake/Healthy mappings and combat/reward stages |
| 48 | MoaiHead | Complete | `The Moai Head` alias; Heal/Idol/Leave mappings |
| 49 | MysteriousSphere | Complete | Fight/Leave mappings and combat/reward transition |
| 50 | SensoryStone | Complete | Live Interact/Recall labels, HP costs, and colorless reward; session-22/23 seed-start regressions |
| 51 | TombOfLordRedMask | Complete | Wear/Offer/Leave mappings |
| 52 | WindingHalls | Complete | Madness/Writhe/Max-HP mappings |

## Structural fixes made during auto-play verification

- Added exhaustive SlayTheData display-name aliases for all 52 events.
- Removed generic event-choice equality as a fallback; ordinary choices now require an event-specific mapping.
- Preserved `relics_lost` from SlayTheData records so N'loth trades bind to the exact relic.
- Split Knowing Skull’s combined `player_choice` sequence into ordered live steps with one-click advancement.
- Made event grids bind outside Reward phase, support multiple and duplicate target cards, and confirm after selection.
- Kept staged event records active through Continue/Leave, reward, grid, and event-combat transitions.
- Updated the Python adapter for indexed event potion rewards used by Lab and Woman in Blue.

## Verification

Passing gates on 2026-07-10:

- `cargo fmt --all -- --check`
- `cargo test --workspace` with the bundled Python runtime: 14 `py_sts`, 166 `sts_core`, 225 `sts_live`, 162 `sts_verify` unit tests, 28 corpus tests, 16 SlayTheData integration tests, all remaining integration/doc tests; zero failures.
- Event-specific regressions include all 52 recorded event-name forms, the full source-outcome mapping fixture, N'loth exact-relic selection, Knowing Skull sequence expansion/advancement, Match and Keep positional binding, event-phase grids, multi-card grids, grid confirmation, and reward/stage flushing.

Strict workspace clippy was run but is not a clean gate: it reports existing warnings in unrelated `sts_core` and `sts_verify::sim_real` code. No clippy failure reported in the event-binding changes themselves before compilation stopped on those existing warnings.

## Remaining blockers

None for event auto-play. Rare-event behavior is regression-fixture verified where no permanent live trace is available; it does not require manual intervention.
