use crate::model::{
    BlockedState, BrokenSlayTheDataRun, LegalAction, LegalActionKind, LiveError, LivePhase,
    LiveResult, LiveState, SlayTheDataAdvisorStep, SlayTheDataGuidedDivergence,
    SlayTheDataGuidedDivergenceKind, SlayTheDataRunOutcome, SlayTheDataRunSummary,
    SlayTheDataSearchFilters, SlayTheDataSessionSnapshot,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row, ToSql};
use serde_json::{json, Value};
use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::Duration,
};
use sts_core::{
    apply_run_decision_action, content::cards::get_card_definition, legal_run_decision_actions,
    match_and_keep_label_index_for_group, ContentId, MapAction, RoomKind, RunDecisionAction,
    RunPhase, RunState, SimError, SimResult,
};
#[cfg(test)]
use sts_verify::SlayTheDataCardName;
use sts_verify::{
    import_slaythedata_run_json, slaythedata_replay_plan, slaythedata_replay_preflight,
    SlayTheDataBridgeDescriptor, SlayTheDataPreflightReport, SlayTheDataPreflightStatus,
    SlayTheDataPreflightStep, SlayTheDataReplayStepKind, SLAYTHEDATA_NORMAL_MAX_FLOOR_REACHED,
};

pub const SLAYTHEDATA_DB_ENV: &str = "STS_LIVE_SLAYTHEDATA_DB";
pub const DEFAULT_SLAYTHEDATA_DB: &str = "slaythedata-chunks.sqlite3";
const SLAYTHEDATA_SEARCH_BUILD_VERSION: &str = "2020-07-30";
// A normal run starts with 99 gold and has at most 57 floors. Final gold above
// this is a useful conservative signal for debug/edited SlayTheData records;
// nullable legacy indexes remain eligible because they cannot apply this test.
const SLAYTHEDATA_MAX_REASONABLE_FINAL_GOLD: i64 = 3_000;
const SLAYTHEDATA_WIN_MIN_FLOOR_REACHED: i64 = 50;
const BROKEN_SLAYTHEDATA_RUNS_TABLE: &str = "broken_slaythedata_runs";
const CORPUS_SLAYTHEDATA_RUNS_TABLE: &str = "corpus_slaythedata_runs";
pub const ILLEGAL_SLAYTHEDATA_RUN_IDS: &[i64] = &[
    7_332_290, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    6_617_772, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_000_178, // SlayTheData route expects Big Fish on floor 5; live map produced a monster.
    4_310_808, // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_908_969, // SlayTheData Neow/card follow-up reward mismatches the live Neow card choice.
    2_709_730, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_195_333, // SlayTheData card reward choice is not present in the live floor 1 reward.
    5_433_747, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_262_855, // SlayTheData card reward choice is not present in the live floor 1 reward.
    2_884_169, // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_162_906, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    921_376,   // SlayTheData card reward choice is not present in the live floor 1 reward.
    2_581_628, // SlayTheData card reward choice is not present in the live floor 2 reward.
    489_770,   // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_556_136, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_088_349, // SlayTheData guided card reward does not match a unique live command.
    3_661_579, // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_769_073, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_481_296, // SlayTheData card reward choice is not present in the live floor 1 reward.
    4_418_494, // SlayTheData card reward choice is not present in the live floor 1 reward.
    262_069,   // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    2_082_841, // SlayTheData card reward choice is not present in the live floor 2 reward.
    2_420_440, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_211_979, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    722_673,   // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_331_790, // SlayTheData expects immediate Neow leave after Pandora's Box grid.
    364_610,   // SlayTheData card reward choice is not present in the live floor 2 reward.
    431_291,   // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    521_173,   // SlayTheData card reward choice is not present in the live floor 1 reward.
    3_386_800, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_245_073, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_796_000, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_303_561, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_159_869, // SlayTheData card reward choice is not present in the live floor 1 reward.
    4_469_851, // SlayTheData card reward choice is not present in the live floor 1 reward.
    4_946_677, // SlayTheData card reward choice is not present in the live floor 1 reward.
    3_158_116, // SlayTheData card reward choice is not present in the live floor 1 reward.
    3_850_886, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    5_849_072, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    61_818,    // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_217_638, // SlayTheData card reward choice is not present in the live floor 1 reward.
    2_328_567, // SlayTheData Neow THREE_CARDS/NONE mismatch.
    2_858_417, // SlayTheData card reward choice is not present in the live floor 1 reward.
    2_053_678, // SlayTheData expects a Neow card reward while live game is already on map.
    2_311_265, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_628_718, // SlayTheData card reward choice is not present in the live floor 5 reward.
    2_613_973, // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_212_758, // SlayTheData card reward choice is not present in the live floor 1 reward.
    2_098_518, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_533_082, // SlayTheData expects a Neow card reward while live game is already on map.
    434_832,   // SlayTheData card reward choice is not present in the live floor 4 reward.
    4_440_656, // SlayTheData card reward choice is not present in the live floor 2 reward.
    5_277_743, // SlayTheData expects a Neow card reward while live game is already on map.
    6_389_452, // SlayTheData expects a Neow card reward while live game is already on map.
    1_969_283, // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_704_060, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    60_134,    // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_277_402, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    774_237,   // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_301_567, // SlayTheData card reward choice is not present in the live floor 1 reward.
    4_218_199, // SlayTheData expects a Neow card reward while live game is already on map.
    5_122_469, // SlayTheData card reward choice is not present in the live floor 1 reward.
    5_534_871, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_005_401, // SlayTheData card reward choice is not present in the live floor 1 reward.
    3_014_203, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_600_485, // SlayTheData card reward choice is not present in the live floor 1 reward.
    112_139,   // SlayTheData expects a Neow card reward while live game is already on map.
    242_559,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    859_020,   // SlayTheData Neow TWENTY_PERCENT_HP_BONUS/NO_GOLD mismatch.
    1_423_433, // SlayTheData card reward choice is not present in the live floor 3 reward.
    4_644_700, // SlayTheData card reward choice is not present in the live floor 1 reward.
    1_496_190, // SlayTheData Neow TRANSFORM_CARD/NONE mismatch.
    2_171_687, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_745_198, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    7_242_786, // SlayTheData expects a Neow card reward while live game is already on map.
    5_946_500, // SlayTheData route expects a rest site on floor 6; live map only offered a monster.
    4_638_368, // SlayTheData card reward choice is not present in the live floor 5 reward.
    1_017_147, // SlayTheData card reward choice is not present in the live floor 1 reward.
    6_021_952, // SlayTheData card reward choice is not present in the live floor 1 reward.
    1_661_225, // SlayTheData expects a Neow card reward while live game is already on map.
    1_642_912, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_918_495, // SlayTheData card reward choice is not present in the live floor 1 reward.
    5_460_294, // SlayTheData expects a Neow card reward while live game is already on map.
    71_865,    // SlayTheData card reward choice is not present in the live floor 1 reward.
    2_658_938, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_367_751, // SlayTheData card reward choice is not present in the live floor 1 reward.
    3_739_222, // SlayTheData expects a Neow card reward while live game is already on map.
    2_005_709, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_577_940, // SlayTheData card reward choice is not present in the live floor 1 reward.
    373_114,   // SlayTheData expects a Neow card reward while live game is already on map.
    3_225_037, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    930_809,   // SlayTheData route expects a shop on floor 3; live map only offered a monster.
    6_980_079, // SlayTheData card reward choice is not present in the live floor 1 reward.
    3_161_655, // SlayTheData expects a Neow card reward while live game is already on map.
    5_329_036, // SlayTheData card reward choice is not present in the live floor 1 reward.
    5_852_478, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_675_466, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_155_276, // SlayTheData expects a Neow card reward while live game is already on map.
    2_233_197, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    993_185,   // SlayTheData card reward choice is not present in the live floor 1 reward.
    410_992,   // SlayTheData card reward choice is not present in the live floor 1 reward.
    5_043_542, // SlayTheData expects a Neow card reward while live game is already on map.
    5_311_495, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_419_032, // SlayTheData expects a Neow card reward while live game is already on map.
    1_397_240, // SlayTheData card reward guidance has no unique live command.
    1_936_714, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    83_498,    // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    955_737,   // SlayTheData card reward guidance has no unique live command.
    1_012_898, // SlayTheData expects a Neow card reward while live game is already on map.
    2_818_016, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_051_433, // SlayTheData expects a Neow card reward while live game is already on map.
    2_061_891, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_383_935, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    6_835_305, // SlayTheData card reward guidance has no unique live command.
    796_956,   // SlayTheData card reward guidance has no unique live command.
    7_236_193, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_787_674, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_264_633, // SlayTheData expects a Neow card reward while live game is already on map.
    1_679_466, // SlayTheData card reward guidance has no unique live command.
    4_074_639, // SlayTheData expects a Neow card reward while live game is already on map.
    3_129_845, // SlayTheData expects a Neow card reward while live game is already on map.
    3_299_551, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_941_857, // SlayTheData event Liars Game mismatches live Big Fish event.
    4_186_065, // SlayTheData expects a Neow card reward while live game is already on map.
    961_365,   // SlayTheData expects a Neow card reward while live game is already on map.
    1_774_962, // SlayTheData card reward guidance has no unique live command.
    3_657_941, // SlayTheData card reward guidance has no unique live command.
    5_894_672, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_196_878, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    1_857_580, // SlayTheData route expects an event on floor 2; live map only offered monster/shop.
    3_172_542, // SlayTheData expects a Neow card reward while live game is already on map.
    3_860_493, // SlayTheData card reward guidance has no unique live command.
    4_578_407, // SlayTheData expects a Neow card reward while live game is already on map.
    2_279_786, // SlayTheData route expects a shop on floor 5; live map had no matching shop node.
    3_553_961, // SlayTheData card reward guidance has no unique live command.
    6_851_513, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_823_631, // SlayTheData card reward guidance has no unique live command.
    3_827_849, // SlayTheData route expects a monster; live map had no matching monster node.
    1_571_202, // SlayTheData expects a Neow card reward while live game is already on map.
    4_599_633, // SlayTheData event guidance mismatches the live event/choice.
    2_396_155, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_643_809, // SlayTheData card reward guidance has no unique live command.
    899_522,   // SlayTheData card reward guidance has no unique live command.
    5_289_457, // SlayTheData card reward guidance has no unique live command.
    181_542,   // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    3_328_878, // SlayTheData card reward guidance has no unique live command.
    3_425_166, // SlayTheData card reward guidance has no unique live command.
    2_854_795, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_351_287, // SlayTheData card reward guidance has no unique live command.
    2_340_894, // SlayTheData expects a Neow card reward while live game is already on map.
    759_972,   // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_129_670, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    399_566,   // SlayTheData card reward guidance has no unique live command.
    7_186_300, // SlayTheData card reward guidance has no unique live command.
    6_759_627, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_446_497, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_367_204, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_665_547, // SlayTheData card reward guidance has no unique live command.
    1_721_373, // SlayTheData card reward guidance has no unique live command.
    3_071_448, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_817_591, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_241_264, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_068_239, // SlayTheData card reward guidance has no unique live command.
    3_267_726, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    4_635_607, // SlayTheData card reward guidance has no unique live command.
    6_234_667, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_847_254, // SlayTheData card reward guidance has no unique live command.
    794_092,   // SlayTheData card reward guidance has no unique live command.
    5_236_541, // SlayTheData card reward guidance has no unique live command.
    3_574_702, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_103_460, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_181_967, // SlayTheData card reward guidance has no unique live command.
    5_131_929, // SlayTheData card reward guidance has no unique live command.
    2_947_839, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    656_235,   // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_003_019, // SlayTheData event guidance mismatches the live event/choice.
    4_651_147, // SlayTheData card reward guidance has no unique live command.
    3_953_104, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_822_269, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    849_101,   // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_634_737, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_649_705, // SlayTheData card reward guidance has no unique live command.
    4_317_009, // SlayTheData card reward guidance has no unique live command.
    3_046_855, // SlayTheData card reward guidance has no unique live command.
    5_647_886, // SlayTheData card reward guidance has no unique live command.
    3_728_615, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    64_041,    // SlayTheData card reward guidance has no unique live command.
    1_635_159, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_052_900, // SlayTheData card reward guidance has no unique live command.
    3_454_866, // SlayTheData card reward guidance has no unique live command.
    5_467_756, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_155_605, // SlayTheData card reward guidance has no unique live command.
    3_503_455, // SlayTheData expects a Neow card reward while live game is already on map.
    2_623_244, // SlayTheData card reward guidance has no unique live command.
    831_463,   // SlayTheData card reward guidance has no unique live command.
    6_041_163, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_699_039, // SlayTheData route expects a monster; live map had no matching monster node.
    4_540_817, // SlayTheData card reward guidance has no unique live command.
    5_820_644, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    5_411_739, // SlayTheData card reward guidance has no unique live command.
    6_657_151, // SlayTheData card reward guidance has no unique live command.
    2_752_193, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    211_925,   // SlayTheData card reward guidance has no unique live command.
    1_782_776, // SlayTheData card reward guidance has no unique live command.
    4_215_531, // SlayTheData route expects a shop; live map had no matching shop node.
    4_572_073, // SlayTheData card reward guidance has no unique live command.
    7_132_207, // SlayTheData route expects an event on floor 5; live map had no matching event node.
    6_942_913, // SlayTheData card reward guidance has no unique live command.
    5_654_904, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    44_677,    // SlayTheData card reward guidance has no unique live command.
    2_983_394, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_470_836, // SlayTheData expects a Neow card reward while live game is already on map.
    2_104_765, // SlayTheData card reward guidance has no unique live command.
    2_537_541, // SlayTheData card reward guidance has no unique live command.
    1_869_488, // SlayTheData card reward guidance has no unique live command.
    3_320_103, // SlayTheData card reward guidance has no unique live command.
    832_489,   // SlayTheData expects a Neow card reward while live game is already on map.
    1_611_216, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_937_220, // SlayTheData expects a Neow card reward while live game is already on map.
    4_720_412, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    7_512_818, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_133_048, // SlayTheData card reward guidance has no unique live command.
    5_318_724, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    6_768_178, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_823_240, // SlayTheData card reward guidance has no unique live command.
    4_097_297, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    7_152_172, // SlayTheData expects a Neow card reward while live game is already on map.
    1_846_023, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_307_713, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_947_428, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    4_841_593, // SlayTheData route expects a monster on floor 2; live map had no matching monster node.
    5_368_985, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_633_823, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    3_980_889, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    273_238,   // SlayTheData card reward guidance has no unique live command.
    4_083_096, // SlayTheData expects a Neow card reward while live game is already on map.
    5_580_216, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_004_777, // SlayTheData card reward guidance has no unique live command after a clean floor 5 path.
    6_194_651, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_307_991, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_154_274, // SlayTheData card reward guidance has no unique live command.
    2_683_677, // SlayTheData expects a Neow card reward while live game is already on map.
    4_707_498, // SlayTheData card reward guidance has no unique live command.
    7_456_172, // SlayTheData expects a Neow card reward while live game is already on map.
    5_812_210, // SlayTheData card reward guidance has no unique live command.
    4_183_224, // SlayTheData card reward guidance has no unique live command.
    4_268_157, // SlayTheData card reward guidance has no unique live command.
    5_865_682, // SlayTheData card reward guidance has no unique live command.
    6_356,     // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    7_488_865, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    3_411_212, // SlayTheData card reward guidance has no unique live command on floor 6.
    4_660_032, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_222_737, // SlayTheData card reward guidance has no unique live command on floor 1.
    797_717, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    2_257_959, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    4_433_560, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_155_057, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_586_382, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_715_856, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_642_187, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_847_927, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_497_301, // SlayTheData expects a Neow card reward while live game is already on map.
    4_269_868, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    801_223, // SlayTheData card reward skip guidance mismatches the live reward screen.
    2_493_643, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_156_078, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_395_390, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_593_863, // SlayTheData expects a Neow card reward while live game is already on map.
    1_759_098, // SlayTheData expects a Neow card reward while live game is already on map.
    6_241_955, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    2_647_280, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_150_689, // SlayTheData card reward guidance has no unique live command on floor 6.
    5_634_963, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_674_194, // SlayTheData card reward guidance has no unique live command on floor 1.
    86_803,  // SlayTheData card reward guidance has no unique live command on floor 1.
    3_752_681, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_443_431, // SlayTheData Neow card reward guidance leaves the live game on map.
    6_456_861, // SlayTheData card reward guidance has no unique live command on floor 1.
    636_577, // SlayTheData card reward guidance has no unique live command on floor 2.
    665_409, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_188_281, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_521_415, // SlayTheData card reward guidance has no unique live command after Tiny House Neow.
    2_330_330, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_664_635, // SlayTheData route expects a map node not available in the live path.
    7_252_389, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    84_522,  // SlayTheData card reward guidance has no unique live command on floor 1.
    4_389_932, // SlayTheData event guidance has no unique live command on floor 4.
    4_762_028, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    60_069,  // SlayTheData card reward guidance has no unique live command on floor 1.
    2_463_026, // SlayTheData card reward guidance has no unique live command on floor 1.
    705_961, // SlayTheData expects a card reward while live game is already on map.
    2_004_993, // SlayTheData event guidance has no unique live command on floor 5.
    2_129_745, // SlayTheData expects a card reward while live game is already on map.
    5_286_722, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    217_730, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_442_924, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_573_722, // SlayTheData event guidance has no unique live command on floor 2.
    6_501_951, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_346_528, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_899_260, // SlayTheData expects a card reward while live game is already on map.
    4_397_492, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    4_401_689, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    321_647, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_425_531, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_985_843, // SlayTheData event guidance has no unique live command on floor 5.
    1_493_314, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_467_795, // SlayTheData card reward guidance has no unique live command on floor 1.
    770_583, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    700_769, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    2_497_355, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_220_539, // SlayTheData expects a card reward while live game is already on map.
    4_715_161, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_221_259, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_384_149, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_896_254, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_827_509, // SlayTheData expects a card reward while live game is already on map.
    2_246_366, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_319_760, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    2_422_330, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_874_158, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_703_658, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    3_569_846, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    3_969_633, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    6_872_062, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_309_673, // SlayTheData expects a card reward while live game is already on map.
    5_821_999, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    3_366_119, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_035_066, // SlayTheData card reward guidance has no unique live command on floor 1.
    818_544, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_399_036, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_043_274, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_212_560, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_272_042, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    5_503_886, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    6_119_851, // SlayTheData card reward guidance has no unique live command after Neow.
    6_470_328, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    191_359, // SlayTheData automatic potion reward transition does not verify.
    2_953_261, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_919_070, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_225_837, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    5_361_731, // SlayTheData expects a Neow card reward while live game is already on map.
    7_394_876, // SlayTheData expects a Neow card reward while live game is already on map.
    5_150_732, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_549_494, // SlayTheData card reward guidance has no unique live command on floor 1.
    897_907, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_054_751, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_520_346, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    5_687_611, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_652_765, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    7_514_825, // SlayTheData card reward guidance has no unique live command on floor 1.
    858_805, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    1_757_475, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    1_847_559, // SlayTheData card reward guidance has no unique live command on floor 1.
    10_969,  // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_612_377, // SlayTheData expects a Neow card reward while live game is already on map.
    3_369_184, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_190_249, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_336_399, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_416_083, // SlayTheData expects a Neow card reward while live game is already on map.
    813_562, // SlayTheData guided event choice has no unique live command on floor 2.
    7_489_153, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    6_723_682, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_592_560, // SlayTheData guided event choice has no unique live command on floor 2.
    4_819_611, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_365_520, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_321_201, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_717_316, // SlayTheData expects a Neow card reward while live game is already on map.
    3_988_984, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    4_137_624, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    6_203_517, // SlayTheData card reward guidance has no unique live command on floor 1.
    566_295, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_237_737, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_229_750, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_424_185, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_653_076, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_823_457, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_326_247, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_305_901, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_088_973, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_810_264, // SlayTheData card reward guidance has no unique live command on floor 5.
    5_386_202, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    240_624, // SlayTheData card reward guidance has no unique live command on floor 1.
    811_192, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_700_511, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_241_376, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_646_098, // SlayTheData expects a Neow card reward while live game is already on map.
    4_004_873, // SlayTheData expects a Neow card reward while live game is already on map.
    4_626_758, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    4_805_672, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_788_271, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_007_505, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_467_560, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    5_182_994, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_296_070, // SlayTheData expects a Neow card reward while live game is already on map.
    7_505_753, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_045_963, // SlayTheData expects a Neow card reward while live game is already on map.
    3_248_150, // SlayTheData card reward guidance has no unique live command on floor 1.
    463_178,   // SlayTheData card reward guidance has no unique live command on floor 1.
    588_641,   // SlayTheData card reward guidance has no unique live command on floor 1.
    1_505_064, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_120_313, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_289_321, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_468_943, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_093_141, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_563_489, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_447_787, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_303_075, // SlayTheData card reward guidance has no unique live command on floor 1.
    575_181,   // SlayTheData card reward guidance has no unique live command on floor 1.
    4_278_078, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_752_456, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_087_270, // SlayTheData card reward guidance has no unique live command on floor 1.
    497_094,   // SlayTheData expects a Neow card reward while live game is already on map.
    1_145_478, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_636_292, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_836_385, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_166_379, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_273_558, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    961_555,   // SlayTheData card reward guidance has no unique live command on floor 1.
    2_482_875, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_854_769, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_727_462, // SlayTheData guided event choice has no unique live command on floor 4.
    2_272_133, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_150_107, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_123_712, // SlayTheData expects a Neow card reward while live game is already on map.
    5_839_509, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_645_215, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_899_375, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    1_322_367, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_752_417, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_955_121, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    5_605_081, // SlayTheData expects a Neow card reward while live game is already on map.
    6_159_363, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_693_974, // SlayTheData card reward guidance has no unique live command on floor 1.
    333_815,   // SlayTheData card reward guidance has no unique live command on floor 1.
    1_191_020, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_330_141, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_782_604, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_559_569, // SlayTheData expects a Neow card reward while live game is already on map.
    6_758_614, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_488_297, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_382_624, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_198_359, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_972_020, // SlayTheData expects a Neow card reward while live game is already on map.
    4_378_924, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_951_941, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_449_926, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_889_203, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_332_943, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_725_803, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_587_489, // SlayTheData expects a Neow card reward while live game is already on map.
    7_056_425, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_350_002, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_274_170, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_168_303, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    7_449_459, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_507_974, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    3_186_912, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_157_165, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    5_521_802, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    7_003_374, // SlayTheData expects a Neow card reward while live game is already on map.
    2_660_352, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_017_161, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_897_379, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_018_416, // SlayTheData card reward guidance has no unique live command on floor 1.
    175_352,   // SlayTheData card reward guidance has no unique live command on floor 1.
    1_400_613, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_428_901, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_787_214, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    5_336_184, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_437_511, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_202_752, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_487_544, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_323_096, // SlayTheData guided event choice has no unique live command on floor 2.
    1_221_175, // SlayTheData expects a Neow card reward while live game is already on map.
    1_260_175, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_994_117, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_460_923, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_215_902, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_447_076, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_591_957, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_824_435, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_836_940, // SlayTheData card reward guidance has no unique live command on floor 1.
    390_935, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    525_338, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_234_186, // SlayTheData expects a Neow card reward while live game is already on map.
    1_291_798, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_057_884, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_284_734, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_123_671, // SlayTheData expects a Neow card reward while live game is already on map.
    6_555_130, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_418_599, // SlayTheData expects a Neow card reward while live game is already on map.
    3_042_299, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_453_824, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_542_214, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_165_718, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_328_023, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_474_857, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_006_632, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_174_089, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_634_840, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    6_493_068, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_354_881, // SlayTheData card reward guidance has no unique live command on floor 1.
    192_173, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_122_278, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_712_261, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_997_002, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_391_555, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    3_995_166, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_505_805, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_560_439, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_570_139, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_992_591, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_371_814, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_394_055, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    752_513, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_375_011, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_419_898, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_111_742, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_628_299, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_325_608, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_207_174, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    6_836_279, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    552_597, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_217_244, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_472_143, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_480_633, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_573_059, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_918_446, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_493_116, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_451_821, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_586_165, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_840_021, // SlayTheData card reward guidance has no unique live command on floor 1.
    556_752, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_211_976, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_076_744, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_201_254, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_630_828, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_663_826, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_789_499, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_588_775, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_080_282, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_873_101, // SlayTheData card reward guidance has no unique live command on floor 1.
    6_826_400, // SlayTheData card reward guidance has no unique live command on floor 1.
    225_582, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_296_647, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_412_977, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    4_838_949, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_964_958, // SlayTheData expects a Neow card reward while live game is already on map.
    5_020_296, // SlayTheData card reward guidance has no unique live command on floor 1.
    5_060_523, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_325_715, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    6_224_755, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    7_377_604, // SlayTheData card reward guidance has no unique live command on floor 1.
    62_689,  // SlayTheData card reward guidance has no unique live command on floor 1.
    315_274, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    402_517, // SlayTheData expects a Neow card reward while live game is already on map.
    600_574, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    1_225_763, // SlayTheData expects a Neow card reward while live game is already on map.
    4_281_057, // SlayTheData route expects an event on floor 3; live map had no matching event node.
    5_361_506, // SlayTheData card reward guidance has no unique live command on floor 0.
    7_531_368, // SlayTheData route expects an elite on floor 10; live map had no matching elite node.
    1_653_298, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_145_896, // SlayTheData card reward guidance has no unique live command after a clean floor 6 shop.
    2_754_062, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_764_176, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_767_669, // SlayTheData event choice guidance has no unique live command on floor 2.
    5_134_124, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_462_856, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    5_875_501, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    563_001,   // SlayTheData card reward guidance has no unique live command on floor 1.
    1_494_201, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    1_756_288, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    2_417_842, // SlayTheData card reward guidance leaves the live game on map at Neow.
    3_391_259, // SlayTheData route expects an event on floor 4; live map had no matching event node.
    4_842_090, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_484_861, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_497_498, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    181_378,   // SlayTheData expects a card reward while live game is already on map.
    1_378_504, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_228_900, // SlayTheData event choice guidance has no unique live command on floor 2.
    4_812_901, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_218_794, // SlayTheData event choice guidance has no unique live command on floor 4.
    6_402_807, // SlayTheData event choice guidance has no unique live command on floor 3.
    458_304, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    1_515_144, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_938_184, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    3_124_756, // SlayTheData event choice guidance has no unique live command on floor 2.
    3_331_183, // SlayTheData route expects a monster on floor 9; live map had no matching monster node.
    3_387_282, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_398_377, // SlayTheData event choice guidance has no unique live command on floor 2.
    5_390_767, // SlayTheData card reward guidance has no unique live command on floor 6.
    5_488_976, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    6_170_280, // SlayTheData card reward guidance has no unique live command on floor 1.
    227_923,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    398_054,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    557_466,   // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    786_224, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    1_390_744, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_968_612, // SlayTheData expects a card reward while live game is already on map.
    2_413_586, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    3_503_290, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    4_179_913, // SlayTheData card reward guidance has no unique live command on floor 1.
    4_666_880, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    4_875_384, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_764_269, // SlayTheData event choice guidance has no unique live command on floor 3.
    6_651_613, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    6_967_391, // SlayTheData event choice guidance has no unique live command on floor 3.
    7_411_680, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    2_215_168, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_990_362, // SlayTheData expects a card reward while live game is already on map.
    3_269_860, // SlayTheData event choice guidance has no unique live command on floor 2.
    5_884_607, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_577_568, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    1_728_044, // SlayTheData event choice guidance has no unique live command on floor 3.
    5_953_860, // SlayTheData card reward guidance has no unique live command on floor 1.
    7_109_388, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_366_165, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    504_022,   // SlayTheData card reward guidance has no unique live command on floor 1.
    841_033,   // SlayTheData card reward guidance has no unique live command on floor 1.
    1_002_862, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    1_035_959, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_128_900, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_451_359, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_580_550, // SlayTheData card reward guidance has no unique live command on floor 3.
    2_586_947, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_623_466, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_304_630, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_386_018, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_508_084, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    5_076_411, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    5_226_523, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_237_241, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    6_860_974, // SlayTheData expects a card reward while live game is already on map.
    293_742, // SlayTheData route expects an event on floor 4; live map had no matching event node.
    1_128_053, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_185_301, // SlayTheData event choice guidance has no unique live command on floor 2.
    1_233_689, // SlayTheData expects a card reward while live game is already on map.
    2_612_641, // SlayTheData event choice guidance has no unique live command on floor 2.
    4_011_908, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_897_562, // SlayTheData event choice guidance has no unique live command on floor 2.
    5_582_698, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_322_939, // SlayTheData expects a card reward while live game is already on map.
    6_358_037, // SlayTheData event choice guidance has no unique live command on floor 3.
    7_432_952, // SlayTheData card reward guidance has no unique live command on floor 1.
    22_588,  // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    300_155, // SlayTheData card reward guidance has no unique live command on floor 1.
    405_129, // SlayTheData event choice guidance has no unique live command on floor 2.
    836_664, // SlayTheData event choice guidance has no unique live command on floor 2.
    883_690, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    955_820, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    1_142_693, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_321_471, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_962_531, // SlayTheData card reward guidance has no unique live command on floor 1.
    3_099_143, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_341_425, // SlayTheData event choice guidance has no unique live command on floor 2.
    4_525_533, // SlayTheData event/card history leaves Note For Yourself guidance unusable on floor 2.
    4_663_457, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_891_791, // SlayTheData expects a card reward while live game is already on map.
    5_022_454, // SlayTheData card reward guidance has no unique live command on floor 2.
    5_323_517, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_499_616, // SlayTheData event choice guidance has no unique live command on floor 3.
    6_631_797, // SlayTheData event/card history leaves guided event choice unusable on floor 5.
    7_341_478, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    7_500_976, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_884_973, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    2_895_735, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_275_979, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    4_328_424, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_491_486, // SlayTheData expects a card reward while live game is already on map.
    4_529_608, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_552_471, // SlayTheData event choice guidance has no unique live command on floor 2.
    4_675_621, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    5_447_936, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    5_701_137, // SlayTheData campfire wants to smith Inflame, but the live deck has no Inflame.
    7_451_068, // SlayTheData event choice guidance has no unique live command on floor 2.
    39_362,    // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    370_213,   // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    1_207_317, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_797_107, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_065_296, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    2_216_264, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    2_673_852, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_701_512, // SlayTheData event choice guidance has no unique live command on floor 4.
    6_589_612, // SlayTheData event choice guidance has no unique live command on floor 4.
    400_651,   // SlayTheData event choice guidance has no unique live command on floor 2.
    953_689,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_218_613, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_369_867, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_880_400, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_971_511, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_297_271, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    4_098_408, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_133_329, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_382_135, // SlayTheData event choice guidance has no unique live command on floor 2.
    4_746_721, // SlayTheData event choice guidance has no unique live command on floor 3.
    4_808_178, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_383_086, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_528_686, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_493_058, // SlayTheData event choice guidance has no unique live command on floor 3.
    3_788_727, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    4_133_950, // SlayTheData event choice guidance has no unique live command on floor 3.
    4_536_411, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    4_612_179, // SlayTheData expects a card reward while live game is already on map.
    4_932_361, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    5_167_920, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    5_482_205, // SlayTheData event choice guidance has no unique live command on floor 3.
    6_514_341, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_526_291, // SlayTheData expects a card reward while live game is already on map.
    6_921_539, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    294_261,   // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_139_415, // SlayTheData card reward guidance has no unique live command on floor 9.
    5_653_607, // SlayTheData route expects a monster on floor 2; live map had no matching monster node.
    5_819_708, // SlayTheData expects a card reward while live game is already on map.
    6_842_465, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    7_075_093, // SlayTheData expects a card reward while live game is already on map.
    7_490_292, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_055_315, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    1_484_063, // SlayTheData card reward guidance has no unique live command on floor 3.
    1_491_672, // SlayTheData card reward guidance has no unique live command on floor 1.
    2_286_877, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_560_938, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    2_781_289, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    2_958_925, // SlayTheData campfire guidance has no unique live command on floor 7.
    4_263_397, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_646_465, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_821_931, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    4_916_001, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    6_331_494, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    6_475_634, // SlayTheData expects a card reward while live game is already on map.
    6_498_799, // SlayTheData expects a card reward while live game is already on map.
    6_920_086, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    7_220_238, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    7_270_518, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    211_067,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    270_662,   // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    1_121_217, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    1_524_829, // SlayTheData event choice guidance has no unique live command on floor 2.
    1_868_239, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_216_602, // SlayTheData event choice guidance has no unique live command on floor 4.
    2_476_799, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_988_714, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    3_244_356, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    3_776_321, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    3_991_417, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_125_442, // SlayTheData card reward guidance has no unique live command on floor 4.
    4_447_235, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    4_550_829, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    4_582_987, // SlayTheData expects a card reward while live game is already on map.
    4_767_526, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    4_822_415, // SlayTheData route expects a shop on floor 4; live map had no matching shop node.
    5_261_836, // SlayTheData route expects a shop on floor 2; live map had no matching shop node.
    6_240_630, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    6_429_434, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    6_433_099, // SlayTheData expects a card reward while live game is already on map.
    6_707_101, // SlayTheData expects a card reward while live game is already on map.
    214_723,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    243_145, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    295_118, // SlayTheData route expects an event on floor 3; live map had no matching event node.
    404_479, // SlayTheData campfire guidance has no unique live command on floor 6.
    456_073, // SlayTheData expects a card reward while live game is already on map.
    855_646, // SlayTheData Neow ONE_RARE_RELIC/TEN_PERCENT_HP_LOSS mismatch.
    1_245_559, // SlayTheData route expects a monster on floor 1; live map had no matching monster node.
    2_916_011, // SlayTheData expects a card reward while live game is already on map.
    3_006_574, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    3_906_165, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_049_699, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    4_478_648, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    5_288_331, // SlayTheData expects a card reward while live game is already on map.
    5_330_786, // SlayTheData Neow THREE_ENEMY_KILL/NONE mismatch.
    5_334_712, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    5_432_230, // SlayTheData event choice guidance has no unique live command on floor 2.
    6_746_700, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    7_054_233, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    19_496,    // SlayTheData card reward guidance has no unique live command on floor 2.
    664_929,   // SlayTheData card reward guidance has no unique live command on floor 2.
    864_528,   // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    955_043,   // SlayTheData expects a card reward while live game is already on map.
    1_028_405, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    1_192_977, // SlayTheData event choice guidance has no unique live command on floor 2.
    1_250_775, // SlayTheData expects a card reward while live game is already on map.
    1_382_181, // SlayTheData expects a card reward while live game is already on map.
    1_830_839, // SlayTheData card reward guidance has no unique live command on floor 1.
    1_888_759, // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
    2_000_242, // SlayTheData event choice guidance has no unique live command on floor 3.
    2_486_390, // SlayTheData event choice guidance has no unique live command on floor 2.
    2_632_712, // SlayTheData route expects an event on floor 2; live map had no matching event node.
    2_841_522, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    3_087_185, // SlayTheData event choice guidance has no unique live command on floor 2.
    3_138_717, // SlayTheData event choice guidance has no unique live command on floor 3.
    3_444_496, // SlayTheData expects immediate Neow leave while live game is in a reward screen.
    3_607_482, // SlayTheData event choice guidance has no unique live command on floor 3.
    3_638_812, // SlayTheData route expects an event on floor 3; live map had no matching event node.
    3_910_650, // SlayTheData route expects an event on floor 1; live map had no matching event node.
    4_303_696, // SlayTheData automatic reward guidance expected a potion when no potion reward existed.
    4_335_158, // SlayTheData route expects a shop on floor 1; live map had no matching shop node.
    4_359_866, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "M" has no live map match.
    4_566_572, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    4_621_663, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "M" has no live map match.
    5_094_412, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    5_563_600, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    5_628_966, // SlayTheData pending_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    5_750_344, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("TEN_PERCENT_HP_BONUS") cost Some("NONE") is not among generated options.
    5_953_259, // SlayTheData guided_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    6_310_212, // SlayTheData slaythedata_auto_action_limit: SlayTheData guided auto-play reached its action limit.
    7_220_004, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    153_254, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("TEN_PERCENT_HP_BONUS") cost Some("NONE") is not among generated options.
    390_882, // SlayTheData pending_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    711_465, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    2_053_272, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    2_122_814, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    2_145_604, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("TEN_PERCENT_HP_BONUS") cost Some("NONE") is not among generated options.
    2_305_748, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    2_642_284, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    3_004_240, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "$" has no live map match.
    3_510_812, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    3_998_654, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    5_078_130, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    5_289_662, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    5_867_118, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    6_202_355, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    6_434_892, // SlayTheData pending_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    6_682_962, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    7_359_859, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    180_016, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "$" has no live map match.
    543_650, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    798_511, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    810_313, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    1_172_401, // SlayTheData guided_campfire: next SlayTheData step is guidance-only and has no unique bridge command.
    1_338_722, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    1_596_162, // SlayTheData pending_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    2_171_670, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    2_173_805, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_neow_leave has no dynamic binding.
    2_323_465, // SlayTheData pending_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    2_921_362, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("TEN_PERCENT_HP_BONUS") cost Some("NONE") is not among generated options.
    3_023_243, // SlayTheData pending_card_reward: next SlayTheData step is guidance-only and has no unique bridge command.
    3_228_419, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    3_578_915, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    3_804_800, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    4_231_204, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    4_240_085, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    4_427_894, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "M" has no live map match.
    4_496_672, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    4_507_237, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    4_736_941, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    4_898_865, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    4_974_913, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    5_035_672, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    5_100_710, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "M" has no live map match.
    5_150_813, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    5_392_296, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "$" has no live map match.
    5_447_191, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    5_755_803, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    5_844_532, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    5_905_390, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_neow_leave has no dynamic binding.
    5_922_361, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    6_422_350, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "M" has no live map match.
    7_315_935, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    7_383_684, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    5_043_653, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    353_157, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_card_reward has no dynamic binding.
    832_741, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    924_982, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "M" has no live map match.
    1_227_355, // SlayTheData neow_option_not_available: SlayTheData Neow bonus Some("THREE_ENEMY_KILL") cost Some("NONE") is not among generated options.
    1_654_353, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "$" has no live map match.
    2_106_765, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    2_224_624, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    2_570_943, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    3_061_538, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    3_386_774, // SlayTheData guided_event_choice: next SlayTheData step is guidance-only and has no unique bridge command.
    3_658_179, // SlayTheData map_symbol_unmatched: pending room resolution route symbol "?" has no live map match.
    3_778_508, // SlayTheData slaythedata_action_mismatch: SlayTheData guided step legal_neow_leave has no dynamic binding.
    284_637,   // SlayTheData Neow TEN_PERCENT_HP_BONUS/NONE mismatch.
];

#[derive(Debug, Clone)]
pub struct SlayTheDataIndex {
    db_path: PathBuf,
    chunks_dir: PathBuf,
}

impl SlayTheDataIndex {
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        let db_path = db_path.as_ref().to_path_buf();
        let chunks_dir = db_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.join("chunks"))
            .unwrap_or_else(|| PathBuf::from("chunks"));
        Self {
            db_path,
            chunks_dir,
        }
    }

    pub fn with_chunks_dir(mut self, chunks_dir: impl AsRef<Path>) -> Self {
        self.chunks_dir = chunks_dir.as_ref().to_path_buf();
        self
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn default_local() -> Self {
        Self::new(
            env::var(SLAYTHEDATA_DB_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_SLAYTHEDATA_DB)),
        )
    }

    pub fn search(
        &self,
        filters: &SlayTheDataSearchFilters,
    ) -> LiveResult<Vec<SlayTheDataRunSummary>> {
        self.search_with_corpus(filters, false)
    }

    pub fn search_with_corpus(
        &self,
        filters: &SlayTheDataSearchFilters,
        include_corpus: bool,
    ) -> LiveResult<Vec<SlayTheDataRunSummary>> {
        let conn = open_readonly(&self.db_path)?;
        require_tables(&conn, &["runs", "chunk_runs"])?;
        let tables = table_names(&conn)?;
        let run_columns = table_columns(&conn, "runs")?;
        let has_run_outcome = run_columns.iter().any(|column| column == "run_outcome");
        let has_outcome_filter = filters.run_outcome.is_some() || filters.victory.is_some();
        let has_seed_filter = filters
            .seed_played
            .as_deref()
            .is_some_and(|seed| !seed.trim().is_empty());
        let runs_source = if has_seed_filter && index_exists(&conn, "idx_runs_live_seed_lookup")? {
            "runs INDEXED BY idx_runs_live_seed_lookup"
        } else if has_seed_filter && index_exists(&conn, "idx_runs_seed")? {
            "runs INDEXED BY idx_runs_seed"
        } else if has_run_outcome
            && has_outcome_filter
            && index_exists(&conn, "idx_runs_search_v3")?
        {
            "runs INDEXED BY idx_runs_search_v3"
        } else if index_exists(&conn, "idx_runs_search_v2")? {
            "runs INDEXED BY idx_runs_search_v2"
        } else {
            "runs"
        };
        let neow_bonus_expr = if run_columns.iter().any(|column| column == "neow_bonus") {
            "runs.neow_bonus"
        } else {
            "NULL"
        };
        let build_version_expr = if run_columns.iter().any(|column| column == "build_version") {
            "runs.build_version"
        } else {
            "NULL"
        };
        let neow_cost_expr = if run_columns.iter().any(|column| column == "neow_cost") {
            "runs.neow_cost"
        } else {
            "NULL"
        };
        let materialized_expr = if tables.iter().any(|table| table == "run_materialized_json") {
            "EXISTS (SELECT 1 FROM run_materialized_json m WHERE m.run_id = ranked.id)"
        } else {
            "0"
        };
        let run_outcome_expr = slaythedata_run_outcome_expr(&run_columns, "runs");

        let mut clauses = vec![
            "runs.character_chosen = ?".to_owned(),
            "runs.floor_reached >= ?".to_owned(),
            "runs.floor_reached <= ?".to_owned(),
            "COALESCE(runs.is_daily, 0) = 0".to_owned(),
            "COALESCE(runs.is_endless, 0) = 0".to_owned(),
            "COALESCE(runs.is_trial, 0) = 0".to_owned(),
        ];
        let mut values: Vec<Box<dyn ToSql>> = vec![
            Box::new(filters.character.to_ascii_uppercase()),
            Box::new(i64::from(filters.min_floor_reached)),
            Box::new(i64::from(SLAYTHEDATA_NORMAL_MAX_FLOOR_REACHED)),
        ];
        if run_columns.iter().any(|column| column == "is_beta") {
            clauses.push("COALESCE(runs.is_beta, 0) = 0".to_owned());
        }
        if run_columns.iter().any(|column| column == "build_version") {
            clauses.push("runs.build_version = ?".to_owned());
            values.push(Box::new(SLAYTHEDATA_SEARCH_BUILD_VERSION));
        }
        if run_columns.iter().any(|column| column == "gold") {
            clauses.push("COALESCE(runs.gold, 0) <= ?".to_owned());
            values.push(Box::new(SLAYTHEDATA_MAX_REASONABLE_FINAL_GOLD));
        }
        if run_columns.iter().any(|column| column == "neow_bonus") {
            clauses.push("TRIM(COALESCE(runs.neow_bonus, '')) <> ''".to_owned());
        }
        if let Some(ascension) = filters.ascension {
            clauses.push("runs.ascension_level = ?".to_owned());
            values.push(Box::new(i64::from(ascension)));
        }
        if let Some(max_floor) = filters.max_floor_reached {
            clauses.push("runs.floor_reached <= ?".to_owned());
            values.push(Box::new(i64::from(max_floor)));
        }
        let run_outcome = filters
            .run_outcome
            .clone()
            .or_else(|| filters.victory.map(SlayTheDataRunOutcome::from_victory));
        if let Some(run_outcome) = run_outcome {
            let filter_expression = if has_run_outcome {
                "runs.run_outcome"
            } else {
                &run_outcome_expr
            };
            clauses.push(format!("{filter_expression} = ?"));
            values.push(Box::new(run_outcome.as_str().to_owned()));
        }
        if let Some(seed) = filters
            .seed_played
            .as_deref()
            .filter(|seed| !seed.trim().is_empty())
        {
            clauses.push("runs.seed_played = ?".to_owned());
            values.push(Box::new(seed.trim().to_owned()));
        }
        if let Some(run_id) = filters.run_id {
            clauses.push("runs.id = ?".to_owned());
            values.push(Box::new(run_id));
        }
        if let Some(neow_bonus) = filters
            .neow_bonus
            .as_deref()
            .filter(|bonus| !bonus.trim().is_empty())
        {
            if run_columns.iter().any(|column| column == "neow_bonus") {
                clauses.push("runs.neow_bonus = ?".to_owned());
                values.push(Box::new(neow_bonus.trim().to_owned()));
            } else {
                clauses.push("0 = 1".to_owned());
            }
        }
        if filters.require_supported && run_columns.iter().any(|column| column == "unsupported_any")
        {
            clauses.push("COALESCE(runs.unsupported_any, 0) = 0".to_owned());
        }
        if !ILLEGAL_SLAYTHEDATA_RUN_IDS.is_empty() {
            clauses.push(format!(
                "runs.id NOT IN ({})",
                std::iter::repeat_n("?", ILLEGAL_SLAYTHEDATA_RUN_IDS.len())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            for run_id in ILLEGAL_SLAYTHEDATA_RUN_IDS {
                values.push(Box::new(*run_id));
            }
        }
        if tables
            .iter()
            .any(|table| table == BROKEN_SLAYTHEDATA_RUNS_TABLE)
        {
            clauses.push(
                "NOT EXISTS (
                    SELECT 1
                    FROM broken_slaythedata_runs broken
                    WHERE broken.run_id = runs.id
                       OR (broken.seed_played IS NOT NULL AND broken.seed_played = runs.seed_played)
                )"
                .to_owned(),
            );
        }
        if !include_corpus
            && tables
                .iter()
                .any(|table| table == CORPUS_SLAYTHEDATA_RUNS_TABLE)
        {
            clauses.push(
                "NOT EXISTS (
                    SELECT 1
                    FROM corpus_slaythedata_runs corpus
                    WHERE corpus.run_id = runs.id
                )"
                .to_owned(),
            );
        }
        let limit = filters.limit.max(1);
        let candidate_limit = slaythedata_search_candidate_limit(filters);
        values.push(Box::new(i64::try_from(candidate_limit).unwrap_or(2_500)));
        values.push(Box::new(i64::try_from(limit).unwrap_or(50)));

        let query = format!(
            r#"
            WITH candidates AS (
                SELECT runs.id,
                       runs.seed_played,
                       {build_version_expr} AS build_version,
                       runs.ascension_level,
                       runs.floor_reached,
                       ({run_outcome_expr} = 'win') AS victory,
                       {run_outcome_expr} AS run_outcome,
                       runs.path_length,
                       runs.card_choice_count,
                       runs.event_choice_count,
                       runs.shop_purchase_count,
                       runs.potion_usage_count,
                       {neow_bonus_expr} AS neow_bonus,
                       {neow_cost_expr} AS neow_cost,
                       COALESCE(runs.card_choice_count, 0)
                         + COALESCE(runs.event_choice_count, 0) * 2
                         + COALESCE(runs.shop_purchase_count, 0) * 3
                         + COALESCE(runs.potion_usage_count, 0) AS guided_score
                FROM {runs_source}
                WHERE {}
                  AND EXISTS (SELECT 1 FROM chunk_runs WHERE chunk_runs.run_id = runs.id)
                LIMIT ?
            ),
            ranked AS (
                SELECT *
                FROM candidates
                ORDER BY COALESCE(path_length, 0) DESC,
                         guided_score DESC,
                         COALESCE(floor_reached, 0) DESC,
                         id ASC
                LIMIT ?
            )
            SELECT ranked.id,
                   ranked.seed_played,
                   ranked.build_version,
                   ranked.ascension_level,
                   ranked.floor_reached,
                   ranked.victory,
                   ranked.run_outcome,
                   ranked.path_length,
                   ranked.card_choice_count,
                   ranked.event_choice_count,
                   ranked.shop_purchase_count,
                   ranked.potion_usage_count,
                   ranked.neow_bonus,
                   ranked.neow_cost,
                   ranked.guided_score,
                   {materialized_expr} AS materialized
            FROM ranked
            "#,
            clauses.join(" AND "),
            runs_source = runs_source
        );
        let params = values
            .iter()
            .map(|value| value.as_ref())
            .collect::<Vec<_>>();
        let mut stmt = conn.prepare(&query).map_err(sql_error)?;
        let mut rows = stmt
            .query_map(&params[..], summary_from_row)
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        if tables.iter().any(|table| table == "run_materialized_json") {
            for summary in &mut rows {
                if summary.materialized {
                    if let Some(raw) = materialized_raw_json(&conn, summary.id)? {
                        summary.build_version = build_version_from_raw_json(&raw);
                    }
                }
            }
        }
        Ok(rows)
    }

    pub fn load_or_materialize_run(
        &self,
        run_id: i64,
    ) -> LiveResult<(SlayTheDataRunSummary, String)> {
        let conn = open_readwrite(&self.db_path)?;
        require_tables(&conn, &["runs"])?;
        let mut summary = self
            .summary_by_id_with_conn(&conn, run_id)?
            .ok_or_else(|| LiveError::NotFound(format!("SlayTheData run {run_id}")))?;
        require_tables(&conn, &["run_materialized_json"])?;
        let raw = match materialized_raw_json(&conn, run_id)? {
            Some(raw) => raw,
            None => self.materialize_from_chunk(&conn, run_id)?,
        };
        summary.materialized = true;
        summary.build_version = build_version_from_raw_json(&raw);
        Ok((summary, raw))
    }

    pub fn mark_broken(
        &self,
        run_id: i64,
        reason: Option<&str>,
    ) -> LiveResult<BrokenSlayTheDataRun> {
        let conn = open_readwrite(&self.db_path)?;
        require_tables(&conn, &["runs"])?;
        ensure_broken_slaythedata_runs_table(&conn)?;
        let seed_played = conn
            .query_row(
                "SELECT seed_played FROM runs WHERE id = ?",
                params![run_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| LiveError::NotFound(format!("SlayTheData run {run_id}")))?;
        let reason = reason
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
            .map(str::to_owned);
        conn.execute(
            r#"
            INSERT INTO broken_slaythedata_runs(run_id, seed_played, reason, marked_at)
            VALUES(?, ?, ?, datetime('now'))
            ON CONFLICT(run_id) DO UPDATE SET
                seed_played = excluded.seed_played,
                reason = excluded.reason,
                marked_at = excluded.marked_at
            "#,
            params![run_id, seed_played, reason],
        )
        .map_err(sql_error)?;
        Ok(BrokenSlayTheDataRun {
            run_id,
            seed_played,
            reason,
        })
    }

    pub fn unmark_broken(&self, run_id: i64) -> LiveResult<bool> {
        let conn = open_readwrite(&self.db_path)?;
        ensure_broken_slaythedata_runs_table(&conn)?;
        Ok(conn
            .execute(
                "DELETE FROM broken_slaythedata_runs WHERE run_id = ?",
                params![run_id],
            )
            .map_err(sql_error)?
            > 0)
    }

    pub fn mark_in_corpus(&self, run_id: i64, trace_path: &Path) -> LiveResult<()> {
        let conn = open_readwrite(&self.db_path)?;
        require_tables(&conn, &["runs"])?;
        let exists = conn
            .query_row("SELECT 1 FROM runs WHERE id = ?", params![run_id], |_| {
                Ok(())
            })
            .optional()
            .map_err(sql_error)?
            .is_some();
        if !exists {
            return Err(LiveError::NotFound(format!("SlayTheData run {run_id}")));
        }
        ensure_corpus_slaythedata_runs_table(&conn)?;
        conn.execute(
            r#"
            INSERT INTO corpus_slaythedata_runs(run_id, trace_path, added_at)
            VALUES(?, ?, datetime('now'))
            ON CONFLICT(run_id) DO UPDATE SET
                trace_path = excluded.trace_path,
                added_at = excluded.added_at
            "#,
            params![run_id, trace_path.display().to_string()],
        )
        .map_err(sql_error)?;
        Ok(())
    }

    fn materialize_from_chunk(&self, conn: &Connection, run_id: i64) -> LiveResult<String> {
        require_tables(conn, &["chunk_runs", "chunk_files"])?;
        let (line_number, chunk_path): (i64, String) = conn
            .query_row(
                r#"
                SELECT cr.line_number, cf.chunk_path
                FROM chunk_runs cr
                JOIN chunk_files cf ON cf.chunk_id = cr.chunk_id
                WHERE cr.run_id = ?
                "#,
                params![run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| {
                LiveError::Blocked(format!(
                    "SlayTheData run {run_id} is not materialized and has no chunk locator row"
                ))
            })?;
        let line_number = usize::try_from(line_number).map_err(|_| {
            LiveError::Blocked(format!(
                "SlayTheData run {run_id} has invalid chunk line number {line_number}"
            ))
        })?;
        let path = self.chunks_dir.join(
            Path::new(&chunk_path)
                .file_name()
                .ok_or_else(|| LiveError::Blocked(format!("invalid chunk path {chunk_path}")))?,
        );
        let file = File::open(&path).map_err(|error| {
            LiveError::Blocked(format!(
                "SlayTheData chunk {} could not be opened: {error}",
                path.display()
            ))
        })?;
        let decoder = ruzstd::decoding::StreamingDecoder::new(file).map_err(|error| {
            LiveError::Blocked(format!(
                "SlayTheData chunk {} could not be decompressed: {error}",
                path.display()
            ))
        })?;
        let mut lines = BufReader::new(decoder).lines();
        let line = lines
            .nth(line_number)
            .transpose()
            .map_err(|error| {
                LiveError::Blocked(format!(
                    "SlayTheData chunk {} could not be read: {error}",
                    path.display()
                ))
            })?
            .ok_or_else(|| {
                LiveError::Blocked(format!(
                    "SlayTheData chunk {} does not contain line {line_number}",
                    path.display()
                ))
            })?;
        let value: serde_json::Value = serde_json::from_str(&line)?;
        let event = value.get("event").ok_or_else(|| {
            LiveError::Blocked(format!(
                "SlayTheData chunk line for run {run_id} does not contain an event object"
            ))
        })?;
        let raw = serde_json::to_string(event)?;
        conn.execute(
            "INSERT OR REPLACE INTO run_materialized_json(run_id, raw_event_json, materialized_at) VALUES(?, ?, datetime('now'))",
            params![run_id, raw],
        )
        .map_err(sql_error)?;
        Ok(raw)
    }

    fn summary_by_id_with_conn(
        &self,
        conn: &Connection,
        run_id: i64,
    ) -> LiveResult<Option<SlayTheDataRunSummary>> {
        let tables = table_names(conn)?;
        let run_columns = table_columns(conn, "runs")?;
        let neow_bonus_expr = if run_columns.iter().any(|column| column == "neow_bonus") {
            "runs.neow_bonus"
        } else {
            "NULL"
        };
        let build_version_expr = if run_columns.iter().any(|column| column == "build_version") {
            "runs.build_version"
        } else {
            "NULL"
        };
        let neow_cost_expr = if run_columns.iter().any(|column| column == "neow_cost") {
            "runs.neow_cost"
        } else {
            "NULL"
        };
        let materialized_expr = if tables.iter().any(|table| table == "run_materialized_json") {
            "EXISTS (SELECT 1 FROM run_materialized_json m WHERE m.run_id = runs.id)"
        } else {
            "0"
        };
        let run_outcome_expr = slaythedata_run_outcome_expr(&run_columns, "runs");
        let query = format!(
            r#"
            SELECT runs.id,
                   runs.seed_played,
                   {build_version_expr},
                   runs.ascension_level,
                   runs.floor_reached,
                   ({run_outcome_expr} = 'win'),
                   {run_outcome_expr},
                   runs.path_length,
                   runs.card_choice_count,
                   runs.event_choice_count,
                   runs.shop_purchase_count,
                   runs.potion_usage_count,
                   {neow_bonus_expr},
                   {neow_cost_expr},
                   COALESCE(runs.card_choice_count, 0)
                     + COALESCE(runs.event_choice_count, 0) * 2
                     + COALESCE(runs.shop_purchase_count, 0) * 3
                     + COALESCE(runs.potion_usage_count, 0) AS guided_score,
                   {materialized_expr} AS materialized
            FROM runs
            WHERE runs.id = ?
            "#
        );
        conn.query_row(&query, params![run_id], summary_from_row)
            .optional()
            .map_err(sql_error)
    }
}

#[derive(Debug, Clone)]
pub struct AttachedSlayTheDataRun {
    pub summary: SlayTheDataRunSummary,
    pub report: SlayTheDataPreflightReport,
    pub next_step_index: usize,
    pub blocked: Option<BlockedState>,
    pub last_message: Option<String>,
    pub auto_play_paused: bool,
}

impl AttachedSlayTheDataRun {
    pub fn from_raw(summary: SlayTheDataRunSummary, raw_run_json: &str) -> LiveResult<Self> {
        let imported = import_slaythedata_run_json(raw_run_json).map_err(|error| {
            LiveError::InvalidAction(format!("SlayTheData import failed: {error}"))
        })?;
        let mut summary = summary;
        summary.build_version = imported.config.build_version.clone();
        let plan = slaythedata_replay_plan(&imported);
        let report = slaythedata_replay_preflight(&plan);
        if report.steps.iter().any(|step| step.intent.is_none()) {
            return Err(LiveError::InvalidAction(
                "SlayTheData preflight omitted typed replay intent".to_owned(),
            ));
        }
        Ok(Self {
            summary,
            report,
            next_step_index: 0,
            blocked: None,
            last_message: Some("SlayTheData run attached".to_owned()),
            auto_play_paused: false,
        })
    }

    pub fn snapshot(&self, state: Option<&LiveState>) -> SlayTheDataSessionSnapshot {
        SlayTheDataSessionSnapshot {
            attached_run: Some(self.summary.clone()),
            advisor: self.advisor_step(state),
            next_step_index: self.next_step_index,
            blocked: self.blocked.clone(),
            last_message: self.last_message.clone(),
            auto_play_paused: self.auto_play_paused,
        }
    }

    pub fn advisor_step(&self, state: Option<&LiveState>) -> Option<SlayTheDataAdvisorStep> {
        for (index, step) in self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
        {
            if is_combat_only_guidance(&step.code) {
                continue;
            }
            if state.is_some_and(|state| step_already_satisfied_by_live_state(step, state)) {
                continue;
            }
            let mut advisor = SlayTheDataAdvisorStep {
                floor: step.floor,
                ordinal: step.ordinal,
                intent: step.intent.clone(),
                status: status_name(step.status).to_owned(),
                code: step.code.clone(),
                message: step.message.clone(),
                command: step
                    .bridge_command
                    .as_ref()
                    .map(|hint| hint.command.clone()),
                action_id: None,
                action_label: None,
            };
            if index < self.next_step_index {
                continue;
            }
            if let Some(state) = state {
                if step.code == "pending_room_resolution" && is_unsettled_neow_map_state(state) {
                    return None;
                }
                if let Ok(action) = self.bind_step_to_live_action(state, index, step) {
                    advisor.action_id = Some(action.id.clone());
                    advisor.action_label = Some(action.label.clone());
                }
            }
            return Some(advisor);
        }
        None
    }

    pub fn ready_action(&self, state: &LiveState) -> Result<(usize, LegalAction), BlockedState> {
        if let Some(blocked) = self.blocked.clone() {
            return Err(blocked);
        }
        let Some((index, step)) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| {
                !is_combat_only_guidance(&step.code)
                    && !step_already_satisfied_by_live_state(step, state)
            })
        else {
            return Err(blocked(
                "slaythedata_done",
                "SlayTheData plan has no remaining guided step",
            ));
        };
        if step.status == SlayTheDataPreflightStatus::Blocked {
            return Err(blocked(&step.code, &step.message));
        }
        let action = self
            .bind_step_to_live_action(state, index, step)
            .map_err(|message| {
                if step.code == "pending_room_resolution"
                    && (message.contains("has no live map match")
                        || message.contains("cannot match remaining SlayTheData route")
                        || message.contains("remaining route")
                        || message.contains("multiple live map matches"))
                {
                    blocked("map_symbol_unmatched", &message)
                } else if step.bridge_command.is_none() {
                    blocked(&step.code, &format!("{}: {message}", step.message))
                } else {
                    blocked("slaythedata_action_mismatch", &message)
                }
            })?;
        Ok((index, action.clone()))
    }

    pub fn mark_sent(&mut self, index: usize) {
        self.next_step_index = index.saturating_add(1);
        self.blocked = None;
        self.last_message = Some("SlayTheData guided action sent".to_owned());
    }

    pub fn skip_unavailable_pending_card_reward(&mut self, state: &LiveState) -> Option<usize> {
        if matches!(state.phase, LivePhase::Combat | LivePhase::Reward) {
            return None;
        }
        let (index, step) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))?;
        if !matches!(
            step.code.as_str(),
            "pending_card_reward" | "guided_card_reward"
        ) {
            return None;
        }
        let live_floor = state
            .raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|floor| u32::try_from(floor).ok());
        let reward_is_definitively_past = match state.phase {
            LivePhase::Map | LivePhase::Rest => live_floor.is_some_and(|floor| floor >= step.floor),
            LivePhase::Unknown if live_screen_type(state) == Some("SHOP_ROOM") => {
                live_floor.is_some_and(|floor| floor >= step.floor)
            }
            _ => live_floor.is_some_and(|floor| floor > step.floor),
        };
        if !reward_is_definitively_past {
            return None;
        }
        self.next_step_index = index.saturating_add(1);
        self.blocked = None;
        self.last_message = Some(format!(
            "Skipped unavailable SlayTheData card reward because live phase is {:?}",
            state.phase
        ));
        Some(index)
    }

    pub fn skip_manually_resolved_unavailable_neow(&mut self, state: &LiveState) -> Option<usize> {
        if state.phase != LivePhase::Map {
            return None;
        }
        let (index, step) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))?;
        if step.code != "neow_option_not_available" {
            return None;
        }
        self.next_step_index = index.saturating_add(1);
        self.blocked = None;
        self.last_message =
            Some("Accepted manually resolved unavailable SlayTheData Neow option".to_owned());
        Some(index)
    }

    pub fn rewind_future_card_reward_to_live_map(&mut self, state: &LiveState) -> Option<usize> {
        if state.phase != LivePhase::Map {
            return None;
        }
        let live_floor = state
            .raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|floor| u32::try_from(floor).ok())?;
        let current = self.report.steps.get(self.next_step_index)?;
        if !matches!(
            current.code.as_str(),
            "pending_card_reward" | "guided_card_reward"
        ) || current.floor <= live_floor
        {
            return None;
        }
        let reward_floor = current.floor;
        let route_index = (0..self.next_step_index).rev().find(|index| {
            let step = &self.report.steps[*index];
            if step.floor + 1 < live_floor
                || step.floor > reward_floor
                || !matches!(
                    step.code.as_str(),
                    "legal_map_room" | "pending_room_resolution"
                )
            {
                return false;
            }
            let Some(symbol) = route_symbol_from_step(step) else {
                return false;
            };
            state.legal_actions.iter().any(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseMapNode
                    && map_action_matches_symbol(state, action, symbol)
            })
        })?;
        self.next_step_index = route_index;
        self.blocked = None;
        self.last_message = Some(format!(
            "Aligned SlayTheData guidance to floor {} route before its future card reward",
            reward_floor
        ));
        Some(route_index)
    }

    pub fn skip_completed_route_on_live_map(&mut self, state: &LiveState) -> Option<usize> {
        if state.phase != LivePhase::Map {
            return None;
        }
        let live_floor = state
            .raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|floor| u32::try_from(floor).ok())?;
        let start = self.next_step_index;
        while self
            .report
            .steps
            .get(self.next_step_index)
            .is_some_and(|step| {
                step.floor <= live_floor
                    && matches!(
                        step.code.as_str(),
                        "legal_map_room" | "pending_room_resolution"
                    )
            })
        {
            self.next_step_index += 1;
        }
        if self.next_step_index == start {
            return None;
        }
        self.blocked = None;
        self.last_message = Some(format!(
            "Skipped completed SlayTheData route guidance through floor {live_floor}"
        ));
        Some(start)
    }

    pub fn rewind_future_unmatched_route_to_live_map(
        &mut self,
        state: &LiveState,
    ) -> Option<usize> {
        if state.phase != LivePhase::Map {
            return None;
        }
        let live_floor = state
            .raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|floor| u32::try_from(floor).ok())?;
        let current = self.report.steps.get(self.next_step_index)?;
        if current.floor <= live_floor
            || !matches!(
                current.code.as_str(),
                "legal_map_room" | "pending_room_resolution"
            )
        {
            return None;
        }
        let current_symbol = route_symbol_from_step(current)?;
        let current_matches = state.legal_actions.iter().any(|action| {
            action.enabled
                && action.kind == LegalActionKind::ChooseMapNode
                && map_action_matches_symbol(state, action, current_symbol)
        });
        if current_matches {
            return None;
        }
        let route_index = (0..self.next_step_index).rev().find(|index| {
            let step = &self.report.steps[*index];
            if step.floor + 1 < live_floor
                || step.floor > current.floor
                || !matches!(
                    step.code.as_str(),
                    "legal_map_room" | "pending_room_resolution"
                )
            {
                return false;
            }
            let Some(symbol) = route_symbol_from_step(step) else {
                return false;
            };
            state.legal_actions.iter().any(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseMapNode
                    && map_action_matches_symbol(state, action, symbol)
            })
        })?;
        self.next_step_index = route_index;
        self.blocked = None;
        self.last_message =
            Some("Restored a legal SlayTheData route from shifted act-floor guidance".to_owned());
        Some(route_index)
    }

    pub fn unavailable_shop_purchase(&self, state: &LiveState) -> Option<(usize, String)> {
        if state.phase != LivePhase::Shop {
            return None;
        }
        let (index, step) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))?;
        if step.code != "guided_shop_purchase" {
            return None;
        }
        let purchase = shop_purchase_from_step(step)?;
        let available = state.legal_actions.iter().any(|action| {
            action.enabled
                && action.kind == LegalActionKind::ShopBuy
                && shop_label_matches_purchase(&action.label, purchase)
        });
        (!available).then(|| (index, purchase.to_owned()))
    }

    pub fn skip_current_shop_purchases(&mut self) -> Vec<(usize, String)> {
        let mut skipped = Vec::new();
        while let Some((index, step)) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))
        {
            if step.code != "guided_shop_purchase" {
                break;
            }
            let purchase = shop_purchase_from_step(step)
                .unwrap_or("unknown shop item")
                .to_owned();
            skipped.push((index, purchase));
            self.next_step_index = index.saturating_add(1);
        }
        self.blocked = None;
        self.last_message = Some(format!(
            "User skipped remaining SlayTheData shop purchases: {}",
            skipped
                .iter()
                .map(|(_, item)| item.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        skipped
    }

    pub fn skip_completed_shop_purge(&mut self, floor: u32) -> Option<usize> {
        let (index, step) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))?;
        if step.code != "guided_shop_purge" || step.floor != floor {
            return None;
        }
        self.next_step_index = index.saturating_add(1);
        self.blocked = None;
        self.last_message = Some(format!(
            "Accepted a manually completed shop purge on floor {floor}"
        ));
        Some(index)
    }

    pub fn skip_unavailable_shop_purge(&mut self, state: &LiveState) -> Option<(usize, String)> {
        if state.phase != LivePhase::Shop {
            return None;
        }
        let deck = state
            .raw
            .pointer("/current_state/message/game_state/deck")?
            .as_array()?;
        let (index, step) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))?;
        if step.code != "guided_shop_purge" {
            return None;
        }
        let target = shop_purge_target_from_step(step)?;
        let target_is_present = deck.iter().any(|card| {
            card.get("name")
                .or_else(|| card.get("id"))
                .and_then(Value::as_str)
                .is_some_and(|label| campfire_grid_label_matches_target(label, target))
        });
        if target_is_present {
            return None;
        }

        let target = target.to_owned();
        self.next_step_index = index.saturating_add(1);
        self.blocked = None;
        self.last_message = Some(format!(
            "Skipped unavailable SlayTheData shop purge target {target:?}"
        ));
        Some((index, target))
    }

    pub fn restore_next_step_index(&mut self, next_step_index: usize) {
        self.next_step_index = next_step_index.min(self.report.steps.len());
        self.blocked = None;
        self.last_message = Some(
            "SlayTheData guidance cursor aligned from recorded actions; simulator state unchanged"
                .to_owned(),
        );
    }

    pub fn guided_divergence(
        &self,
        step_index: usize,
        kind: SlayTheDataGuidedDivergenceKind,
        reason: impl Into<String>,
    ) -> Option<SlayTheDataGuidedDivergence> {
        let step = self.report.steps.get(step_index)?;
        Some(SlayTheDataGuidedDivergence {
            kind,
            step_index,
            floor: step.floor,
            intent: step.intent.clone()?,
            source_build_version: self.summary.build_version.clone(),
            reason: reason.into(),
        })
    }

    pub fn restore_progress_from_recorded_action(&mut self, action: &LegalAction) -> bool {
        if let Some((index, step)) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))
        {
            if step.code == "pending_room_resolution"
                && action.kind == LegalActionKind::ChooseMapNode
            {
                // A guided map action can reach the live game before fidelity
                // verification finishes.  If that verification fails, there
                // is no sent_action checkpoint, but the recorded action still
                // proves that this route step was consumed.
                self.mark_sent(index);
                self.last_message = Some(
                    "SlayTheData guidance cursor aligned from recorded actions; simulator state unchanged"
                        .to_owned(),
                );
                return true;
            }
        }
        for (index, step) in self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
        {
            if is_combat_only_guidance(&step.code) {
                continue;
            }
            if recorded_action_matches_step(action, step) {
                if recorded_action_advances_step(&step.code, action) {
                    self.mark_sent_after_action(index, action);
                }
                self.last_message = Some(
                    "SlayTheData guidance cursor aligned from recorded actions; simulator state unchanged"
                        .to_owned(),
                );
                return true;
            }
        }
        false
    }

    pub fn align_progress_to_live_state(&mut self, state: &LiveState) -> bool {
        let live_floor = state
            .raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|floor| u32::try_from(floor).ok());
        let entered_act_boss = self
            .report
            .steps
            .get(self.next_step_index)
            .is_some_and(|step| {
                step.code == "pending_room_resolution"
                    && route_symbol_from_step(step)
                        .is_some_and(|symbol| symbol.eq_ignore_ascii_case("B"))
                    && live_floor.is_some_and(|floor| floor == step.floor)
                    && is_act_boss_room(state)
            });
        if entered_act_boss {
            self.next_step_index = self.next_step_index.saturating_add(1);
            self.blocked = None;
            self.last_message = Some("Skipped already-entered act-boss route guidance".to_owned());
            return true;
        }
        let completed_act_boss = self
            .report
            .steps
            .get(self.next_step_index)
            .is_some_and(|step| {
                step.code == "pending_room_resolution"
                    && route_symbol_from_step(step)
                        .is_some_and(|symbol| symbol.eq_ignore_ascii_case("B"))
                    // Boss reward/chest guidance can be attributed to the
                    // first floor of the following act by imported runs.  A
                    // freshly opened act map is authoritative evidence that
                    // the previous boss room resolved, but do not consume a
                    // genuinely future act-boss step.
                    && live_floor
                        .is_some_and(|floor| step.floor <= floor.saturating_add(1))
                    && is_new_act_entry_map(state)
            });
        if completed_act_boss {
            self.next_step_index = self.next_step_index.saturating_add(1);
            self.blocked = None;
            self.last_message =
                Some("Skipped completed act-boss route guidance on the new act map".to_owned());
            return true;
        }
        let current_is_implausibly_future = live_floor.is_some_and(|floor| {
            self.report
                .steps
                .get(self.next_step_index)
                .is_some_and(|step| step.floor > floor.saturating_add(1))
        });
        if current_is_implausibly_future {
            let floor = live_floor.expect("checked above");
            for (index, step) in self.report.steps.iter().enumerate() {
                if step.floor < floor || step.floor > floor.saturating_add(1) {
                    continue;
                }
                if is_combat_only_guidance(&step.code)
                    || step_already_satisfied_by_live_state(step, state)
                {
                    continue;
                }
                if self.bind_step_to_live_action(state, index, step).is_ok() {
                    self.restore_next_step_index(index);
                    self.last_message = Some(
                        "SlayTheData progress rewound from an implausible future floor".to_owned(),
                    );
                    return true;
                }
            }
        }
        for (index, step) in self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
        {
            if live_floor.is_some_and(|floor| step.floor > floor.saturating_add(1)) {
                continue;
            }
            if is_combat_only_guidance(&step.code) {
                continue;
            }
            if step_already_satisfied_by_live_state(step, state) {
                continue;
            }
            let matches = self.bind_step_to_live_action(state, index, step).is_ok();
            if matches {
                self.restore_next_step_index(index);
                self.last_message =
                    Some("SlayTheData progress aligned to current live state".to_owned());
                return true;
            }
        }
        false
    }

    pub fn align_past_completed_non_shop_guidance(
        &mut self,
        state: &LiveState,
    ) -> Option<(usize, usize, String)> {
        if state.phase != LivePhase::Map {
            return None;
        }
        let live_floor = state
            .raw
            .pointer("/summary/floor")
            .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|floor| u32::try_from(floor).ok())?;
        let (current_index, current_step) = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(_, step)| !is_combat_only_guidance(&step.code))?;
        if current_step.floor > live_floor
            || matches!(
                current_step.code.as_str(),
                "guided_shop_purchase" | "guided_shop_purge"
            )
        {
            return None;
        }
        let skipped_floor = current_step.floor;
        let skipped_code = current_step.code.clone();
        let aligned_index = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(current_index.saturating_add(1))
            .find(|(index, step)| {
                step.floor >= live_floor
                    && step.floor <= live_floor.saturating_add(1)
                    && !is_combat_only_guidance(&step.code)
                    && !step_already_satisfied_by_live_state(step, state)
                    && self.bind_step_to_live_action(state, *index, step).is_ok()
            })
            .map(|(index, _)| index)
            .or_else(|| {
                (current_step.code == "guided_campfire").then_some(current_index.saturating_add(1))
            })?;
        self.next_step_index = aligned_index;
        self.blocked = None;
        self.last_message = Some(format!(
            "Skipped completed floor {skipped_floor} {skipped_code} guidance and aligned to live floor {live_floor}"
        ));
        Some((current_index, aligned_index, skipped_code))
    }

    pub fn rewind_to_unresolved_shop_purchase(
        &mut self,
        floor: u32,
        explicitly_skipped_steps: &std::collections::HashSet<usize>,
    ) -> bool {
        let unresolved = self
            .report
            .steps
            .iter()
            .enumerate()
            .skip(self.next_step_index)
            .find(|(index, step)| {
                step.floor == floor
                    && step.code == "guided_shop_purchase"
                    && !explicitly_skipped_steps.contains(index)
            })
            .map(|(index, _)| index);
        let Some(index) = unresolved else {
            return false;
        };
        self.next_step_index = index;
        self.blocked = None;
        self.last_message = Some(
            "Aligned SlayTheData guidance to unresolved shop purchase before allowing shop exit"
                .to_owned(),
        );
        true
    }

    pub fn mark_blocked(&mut self, blocked: BlockedState) {
        self.last_message = Some(blocked.message.clone());
        self.blocked = Some(blocked);
    }

    fn bind_step_to_live_action<'a>(
        &self,
        state: &'a LiveState,
        index: usize,
        step: &SlayTheDataPreflightStep,
    ) -> Result<&'a LegalAction, String> {
        if matches!(
            step.code.as_str(),
            "legal_map_room" | "pending_room_resolution"
        ) && state.phase == LivePhase::Map
        {
            if let Some(symbol) = route_symbol_from_step(step) {
                return bind_map_step_to_live_action_with_route_suffix(
                    state,
                    &self.report.steps,
                    index,
                    symbol,
                );
            }
        }
        if step.code == "legal_neow_leave" && is_grid_screen(state) {
            return bind_neow_followup_grid_action(state);
        }
        if step.code == "pending_room_resolution" && live_screen_type(state) == Some("SHOP_ROOM") {
            let live_floor = state
                .raw
                .pointer("/summary/floor")
                .and_then(|value| value.as_u64());
            let unresolved_shop_work = live_floor.is_some_and(|floor| {
                self.report.steps.iter().skip(index).any(|candidate| {
                    u64::from(candidate.floor) == floor
                        && matches!(
                            candidate.code.as_str(),
                            "guided_shop_purchase" | "guided_shop_purge"
                        )
                })
            });
            let (command, label) = if unresolved_shop_work {
                ("CHOOSE 0", "shop")
            } else {
                ("PROCEED", "proceed")
            };
            return bind_matching_live_action(state, command, |action| {
                action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case(label)
            });
        }
        bind_step_to_live_action(state, step)
            .or_else(|_| bind_dynamic_guided_step_to_live_action(state, step))
    }
}

fn recorded_action_matches_step(action: &LegalAction, step: &SlayTheDataPreflightStep) -> bool {
    let Some(hint) = step.bridge_command.as_ref() else {
        return false;
    };
    action
        .command
        .get("command")
        .and_then(serde_json::Value::as_str)
        == Some(hint.command.as_str())
}

pub fn bind_command_to_live_action<'a>(
    state: &'a LiveState,
    expected_command: &str,
) -> Result<&'a LegalAction, String> {
    bind_matching_live_action(state, expected_command, |_| true)
}

pub fn bind_step_to_live_action<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
) -> Result<&'a LegalAction, String> {
    if step.code == "legal_neow_leave" && (state.phase != LivePhase::Neow || is_grid_screen(state))
    {
        return Err("Neow leave cannot bind while a Neow follow-up screen is open".to_owned());
    }
    let Some(hint) = step.bridge_command.as_ref() else {
        return Err("SlayTheData step has no bridge command".to_owned());
    };
    let expected = expected_live_context(step, &hint.descriptor);
    bind_matching_live_action(state, &hint.command, |action| {
        expected
            .as_ref()
            .is_none_or(|context| context.matches(state, action))
    })
    .map_err(|message| {
        if let Some(context) = expected {
            format!(
                "{message}; expected live context phase {:?} and action kind {:?} for SlayTheData step {}",
                context.phase, context.kind, step.code
            )
        } else {
            message
        }
    })
}

fn bind_dynamic_guided_step_to_live_action<'a>(
    state: &'a LiveState,
    step: &SlayTheDataPreflightStep,
) -> Result<&'a LegalAction, String> {
    #[cfg(test)]
    let legacy_intent;
    let intent = match step.intent.as_ref() {
        Some(intent) => intent,
        None => {
            #[cfg(test)]
            {
                legacy_intent = legacy_test_intent(step)?;
                &legacy_intent
            }
            #[cfg(not(test))]
            {
                return Err(format!(
                    "SlayTheData step {} is missing its typed replay intent",
                    step.code
                ));
            }
        }
    };
    if step.code == "pending_room_resolution"
        && state
            .raw
            .pointer("/summary/screen_name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|screen| screen.eq_ignore_ascii_case("FTUE"))
    {
        return bind_matching_live_action(state, "CLICK LEFT 1080 700 250", |action| {
            action.kind == LegalActionKind::Confirm
                && action.label.eq_ignore_ascii_case("Dismiss tutorial")
        });
    }
    let is_dynamic_card_reward = matches!(
        step.code.as_str(),
        "pending_card_reward" | "guided_card_reward" | "legal_card_reward"
    );
    let card_reward = match intent {
        SlayTheDataReplayStepKind::CardReward { picked, skipped } => Some((picked, *skipped)),
        _ => None,
    };
    if is_dynamic_card_reward
        && card_reward.is_some_and(|(_, skipped)| skipped)
        && state.phase == LivePhase::Reward
    {
        return reward_flush_action_before_high_level_step(state, "pending skipped card reward");
    }
    if is_dynamic_card_reward && state.phase == LivePhase::Reward {
        let Some(target) = card_reward
            .and_then(|(picked, _)| picked.as_ref())
            .map(|card| card.raw.as_str())
        else {
            return Err("pending card reward has no concrete SlayTheData pick".to_owned());
        };
        if is_card_reward_screen(state) {
            if grid_confirm_up(state) {
                return bind_matching_live_action(state, "CONFIRM", |action| {
                    action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("confirm")
                });
            }
            return first_card_label_match(state, target).ok_or_else(|| {
                format!("pending card reward target {target:?} has no live grid label match")
            });
        }
        if let Some(action) = reward_choice_by_label(state, "card") {
            return Ok(action);
        }
        return reward_flush_action_before_high_level_step(state, "pending card reward");
    }
    if step.code == "pending_room_resolution" && state.phase == LivePhase::Map {
        if let Some(symbol) = route_symbol_from_step(step) {
            if symbol.eq_ignore_ascii_case("B") {
                let boss_actions = state
                    .legal_actions
                    .iter()
                    .filter(|action| {
                        action.enabled
                            && action.kind == LegalActionKind::ChooseMapNode
                            && action.label.eq_ignore_ascii_case("boss")
                    })
                    .collect::<Vec<_>>();
                return match boss_actions.as_slice() {
                    [action] => Ok(action),
                    [] => Err("pending boss room has no enabled live boss action".to_owned()),
                    _ => Err("pending boss room has multiple enabled live boss actions".to_owned()),
                };
            }
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
                .filter(|action| map_action_matches_symbol(state, action, symbol))
                .collect::<Vec<_>>();
            if let Some(action) = matches.into_iter().next() {
                return Ok(action);
            }
            return Err(format!(
                "pending room resolution route symbol {symbol:?} has no live map match"
            ));
        }
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending room resolution has no live map choices".to_owned()),
            _ => Err("pending room resolution has multiple live map choices".to_owned()),
        };
    }
    if step.code == "pending_room_resolution" && state.phase == LivePhase::Event {
        if current_event_name(state).is_some_and(|name| name == "Golden Idol") {
            if let Some(action) = unique_event_choice_by_label(state, "outrun") {
                return Ok(action);
            }
        }
        if let Some(action) = unique_event_choice_by_label(state, "continue") {
            return Ok(action);
        }
        if let Some(action) = unique_event_choice_by_label(state, "play") {
            return Ok(action);
        }
        if let Some(action) = unique_event_choice_by_label(state, "spin") {
            return Ok(action);
        }
        if current_event_name(state).is_some_and(|name| name == "Wheel of Change") {
            if let Some(action) = unique_enabled_event_choice(state) {
                return Ok(action);
            }
        }
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::EventChoice
                    && action.label.eq_ignore_ascii_case("leave")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending room resolution has no live event leave choice".to_owned()),
            _ => Err("pending room resolution has multiple live event leave choices".to_owned()),
        };
    }
    if step.code == "pending_room_resolution" && state.phase == LivePhase::Neow {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseNeow
                    && action.label.eq_ignore_ascii_case("leave")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending room resolution has no live Neow leave choice".to_owned()),
            _ => Err("pending room resolution has multiple live Neow leave choices".to_owned()),
        };
    }
    if step.code == "pending_room_resolution"
        && state.phase == LivePhase::Reward
        && is_grid_screen(state)
        && grid_confirm_up(state)
    {
        return bind_matching_live_action(state, "CONFIRM", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("confirm")
        });
    }
    if step.code == "pending_room_resolution" && state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "pending room resolution");
    }
    if step.code == "pending_room_resolution" && state.phase == LivePhase::Rest {
        return bind_matching_live_action(state, "PROCEED", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("proceed")
        });
    }
    if step.code == "pending_room_resolution"
        && live_screen_type(state).is_some_and(|screen| screen == "CHEST")
    {
        if let Ok(action) = bind_matching_live_action(state, "CHOOSE 0", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("open")
        }) {
            return Ok(action);
        }
        return bind_matching_live_action(state, "PROCEED", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("proceed")
        });
    }
    if step.code == "pending_room_resolution"
        && (state.phase == LivePhase::Shop
            || live_screen_type(state).is_some_and(|screen| screen == "SHOP_SCREEN"))
    {
        return unique_leave_shop_action(state).ok_or_else(|| {
            "pending room resolution has no unique live shop leave choice".to_owned()
        });
    }
    if step.code == "pending_room_resolution"
        && live_screen_type(state).is_some_and(|screen| screen == "SHOP_ROOM")
    {
        return bind_matching_live_action(state, "PROCEED", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("proceed")
        });
    }
    if step.code == "pending_neow_followup" && is_grid_screen(state) {
        return bind_neow_followup_grid_action(state);
    }
    if step.code == "pending_neow_followup" && state.phase == LivePhase::Reward {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseReward)
            .collect::<Vec<_>>();
        if let Some(action) = matches.into_iter().next() {
            return Ok(action);
        }
        return reward_flush_action_before_high_level_step(state, "pending Neow follow-up");
    }
    if step.code == "pending_neow_followup" && state.phase == LivePhase::Neow {
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseNeow
                    && action.label.eq_ignore_ascii_case("leave")
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err("pending Neow follow-up has no live Neow leave choice".to_owned()),
            _ => Err("pending Neow follow-up has multiple live Neow leave choices".to_owned()),
        };
    }
    if step.code == "legal_neow_leave" && state.phase == LivePhase::Reward {
        if let Some(action) = first_enabled_reward_choice(state) {
            return Ok(action);
        }
        return reward_flush_action_before_high_level_step(state, "legal Neow leave");
    }
    if is_guided_event_step(&step.code) && is_grid_screen(state) {
        if grid_confirm_up(state) {
            return bind_matching_live_action(state, "CONFIRM", |action| {
                action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("confirm")
            });
        }
        let targets = guided_event_grid_targets(intent);
        if targets.is_empty() {
            return Err("guided event grid has no target card".to_owned());
        }
        let selected_count = grid_selected_card_count(state);
        if let Some(action) = targets
            .iter()
            .skip(selected_count)
            .chain(targets.iter().take(selected_count))
            .find_map(|target| first_card_label_match(state, target))
        {
            return Ok(action);
        }
        if drug_dealer_test_subject_step(intent) {
            // SlayTheData records the two transformed outputs, not the two
            // source cards removed from the deck.  Prefer expendable starter
            // cards while the two-card selection grid remains open. Selected
            // cards remain clickable in CommunicationMod, so advance through
            // the ordered candidates by the selected-card count instead of
            // toggling the first card on and off.
            let selected_count = grid_selected_card_count(state);
            let starter_choices = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::ChooseReward
                        && ["strike", "defend"]
                            .into_iter()
                            .any(|target| campfire_grid_label_matches_target(&action.label, target))
                })
                .collect::<Vec<_>>();
            let all_choices = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseReward)
                .collect::<Vec<_>>();
            if let Some(action) = starter_choices
                .get(selected_count)
                .or_else(|| all_choices.get(selected_count))
                .copied()
            {
                return Ok(action);
            }
        }
        return Err(format!(
            "guided event grid targets {targets:?} have no enabled live grid label match"
        ));
    }
    if is_guided_event_step(&step.code) && state.phase == LivePhase::Reward {
        return reward_flush_action_before_high_level_step(state, "guided event choice");
    }
    if is_guided_event_step(&step.code) && state.phase == LivePhase::Event {
        if let Some(action) = unique_event_choice_by_label(state, "continue") {
            return Ok(action);
        }
        // Wheel of Change has no player-selected outcome: RNG has already
        // chosen the recorded result, and each stage exposes exactly one
        // button (Play, spin, prize!, then Leave). Bind that sole action even
        // though SlayTheData names the result rather than the button label.
        if current_event_name(state).is_some_and(|name| name == "Wheel of Change") {
            if let Some(action) = unique_enabled_event_choice(state) {
                return Ok(action);
            }
        }
        let enabled_event_choices = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
            .collect::<Vec<_>>();
        if let [action] = enabled_event_choices.as_slice() {
            if action.label.eq_ignore_ascii_case("leave") {
                return Ok(action);
            }
        }
        if current_event_name(state).is_some_and(|name| name == "Golden Shrine") {
            if let Some(action) = unique_event_choice_by_label(state, "leave") {
                return Ok(action);
            }
        }
        if current_event_name(state).is_some_and(|name| name == "Big Fish") {
            if let Some(action) = unique_event_choice_by_label(state, "leave") {
                return Ok(action);
            }
        }
        let event_intent = match intent {
            SlayTheDataReplayStepKind::EventChoice {
                event_name,
                player_choice,
                relics_lost,
                ..
            } => Some((
                event_name.as_deref(),
                player_choice.as_deref(),
                relics_lost.as_slice(),
            )),
            _ => None,
        };
        if event_intent
            .and_then(|(event_name, _, _)| event_name)
            .is_some_and(|event_name| event_name == "Match and Keep!")
        {
            if let Some(action) = unique_event_choice_by_label(state, "leave") {
                return Ok(action);
            }
            if let Some(action) = unique_event_choice_by_label(state, "play") {
                return Ok(action);
            }
            if let Some(action) = bind_match_and_keep_action(state, intent)? {
                return Ok(action);
            }
        }
        let Some(choice) = event_intent.and_then(|(_, choice, _)| choice) else {
            return Err("guided event choice has no concrete SlayTheData choice".to_owned());
        };
        if current_event_name(state).is_some_and(|name| name == "Golden Idol") {
            if let Some(action) = unique_event_choice_by_label(state, "take") {
                return Ok(action);
            }
        }
        if normalize_live_label(current_event_name(state).unwrap_or_default()).replace(' ', "")
            == "nloth"
            && normalize_live_label(choice).replace(' ', "") == "tradedrelic"
        {
            let Some(relic) = event_intent
                .and_then(|(_, _, relics_lost)| relics_lost.first())
                .map(String::as_str)
            else {
                return Err("N'loth trade has no recorded lost relic".to_owned());
            };
            let target = normalize_live_label(relic).replace(' ', "");
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::EventChoice
                        && normalize_live_label(&action.label)
                            .replace(' ', "")
                            .contains(&target)
                })
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err(format!(
                    "N'loth trade has no live action for lost relic {relic:?}"
                )),
                _ => Err(format!(
                    "N'loth trade has multiple live actions for lost relic {relic:?}"
                )),
            };
        }
        let event_name = current_event_name(state)
            .filter(|name| !name.trim().is_empty())
            .or_else(|| event_intent.and_then(|(event_name, _, _)| event_name))
            .unwrap_or_default();
        let matches = state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
            .filter(|action| {
                event_label_matches_choice_for_event(event_name, &action.label, choice)
            })
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            if let Some(preferred) = preferred_event_label(event_name, choice) {
                let preferred_matches = matches
                    .iter()
                    .copied()
                    .filter(|action| {
                        normalize_live_label(&action.label).replace(' ', "") == preferred
                    })
                    .collect::<Vec<_>>();
                if let [action] = preferred_matches.as_slice() {
                    return Ok(action);
                }
            }
        }
        return match matches.as_slice() {
            [action] => Ok(action),
            [] => Err(format!(
                "guided event choice {choice:?} has no live label match"
            )),
            _ => Err(format!(
                "guided event choice {choice:?} matched multiple live actions"
            )),
        };
    }
    if step.code == "guided_shop_purchase" {
        let SlayTheDataReplayStepKind::ShopPurchase { item: purchase, .. } = intent else {
            return Err("guided shop purchase has no concrete SlayTheData item".to_owned());
        };
        if state.phase == LivePhase::Map {
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
                .filter(|action| map_action_matches_symbol(state, action, "$"))
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err("guided shop purchase has no live shop map node".to_owned()),
                _ => Err("guided shop purchase matched multiple live shop map nodes".to_owned()),
            };
        }
        if state.phase == LivePhase::Reward {
            return reward_flush_action_before_high_level_step(state, "guided shop purchase");
        }
        if live_screen_type(state).is_some_and(|screen| screen == "SHOP_ROOM") {
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("shop")
                })
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err("guided shop purchase has no live shop entry action".to_owned()),
                _ => Err("guided shop purchase matched multiple shop entry actions".to_owned()),
            };
        }
        if state.phase == LivePhase::Shop {
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ShopBuy)
                .filter(|action| shop_label_matches_purchase(&action.label, purchase))
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err(format!(
                    "guided shop purchase {purchase:?} has no enabled live shop label match"
                )),
                _ => Err(format!(
                    "guided shop purchase {purchase:?} matched multiple live shop actions"
                )),
            };
        }
    }
    if step.code == "guided_shop_purge" {
        let SlayTheDataReplayStepKind::ShopPurge { card } = intent else {
            return Err("guided shop purge has no concrete SlayTheData target".to_owned());
        };
        let target = card.raw.as_str();
        if state.phase == LivePhase::Map {
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
                .filter(|action| map_action_matches_symbol(state, action, "$"))
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err("guided shop purge has no live shop map node".to_owned()),
                _ => Err("guided shop purge matched multiple live shop map nodes".to_owned()),
            };
        }
        if live_screen_type(state).is_some_and(|screen| screen == "SHOP_ROOM") {
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("shop")
                })
                .collect::<Vec<_>>();
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err("guided shop purge has no live shop entry action".to_owned()),
                _ => Err("guided shop purge matched multiple shop entry actions".to_owned()),
            };
        }
        if is_grid_screen(state) {
            if grid_confirm_up(state) {
                return bind_matching_live_action(state, "CONFIRM", |action| {
                    action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("confirm")
                });
            }
            return first_card_label_match(state, target).ok_or_else(|| {
                format!("guided shop purge target {target:?} has no live grid label match")
            });
        }
        if state.phase == LivePhase::Reward {
            return reward_flush_action_before_high_level_step(state, "guided shop purge");
        }
        if state.phase == LivePhase::Shop {
            return state
                .legal_actions
                .iter()
                .find(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::ShopBuy
                        && action.label.eq_ignore_ascii_case("purge")
                })
                .ok_or_else(|| "guided shop purge has no enabled live purge action".to_owned());
        }
    }
    if step.code == "guided_campfire" {
        if state.phase == LivePhase::Reward && !is_grid_screen(state) {
            return reward_flush_action_before_high_level_step(state, "guided campfire");
        }
        if state.phase == LivePhase::Rest
            && !state
                .legal_actions
                .iter()
                .any(|action| action.enabled && action.kind == LegalActionKind::RestSite)
        {
            let proceed = state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled
                        && action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("proceed")
                })
                .collect::<Vec<_>>();
            if !proceed.is_empty() {
                return match proceed.as_slice() {
                    [action] => Ok(action),
                    _ => Err("guided campfire matched multiple live Proceed actions".to_owned()),
                };
            }
        }
        let SlayTheDataReplayStepKind::Campfire { key, target_card } = intent else {
            return Err("guided campfire has no typed campfire intent".to_owned());
        };
        let Some(key) = key.as_deref() else {
            return Err("guided campfire has no concrete SlayTheData key".to_owned());
        };
        if state.phase == LivePhase::Rest {
            if should_override_campfire_with_rest(state) {
                let rest = state
                    .legal_actions
                    .iter()
                    .filter(|action| action.enabled && action.kind == LegalActionKind::RestSite)
                    .filter(|action| action.label.eq_ignore_ascii_case("rest"))
                    .collect::<Vec<_>>();
                return match rest.as_slice() {
                    [action] => Ok(action),
                    [] => Err("low-HP campfire override has no enabled Rest action".to_owned()),
                    _ => Err("low-HP campfire override matched multiple Rest actions".to_owned()),
                };
            }
            let matches = state
                .legal_actions
                .iter()
                .filter(|action| action.enabled && action.kind == LegalActionKind::RestSite)
                .filter(|action| campfire_label_matches_key(&action.label, key))
                .collect::<Vec<_>>();
            if matches.is_empty()
                && key.eq_ignore_ascii_case("SMITH")
                && live_state_has_relic(state, "Fusion Hammer")
            {
                let rest = state
                    .legal_actions
                    .iter()
                    .filter(|action| action.enabled && action.kind == LegalActionKind::RestSite)
                    .filter(|action| action.label.eq_ignore_ascii_case("rest"))
                    .collect::<Vec<_>>();
                return match rest.as_slice() {
                    [action] => Ok(action),
                    [] => Err("Fusion Hammer fallback has no enabled Rest action".to_owned()),
                    _ => Err("Fusion Hammer fallback matched multiple Rest actions".to_owned()),
                };
            }
            return match matches.as_slice() {
                [action] => Ok(action),
                [] => Err(format!(
                    "guided campfire key {key:?} has no live rest label match"
                )),
                _ => Err(format!(
                    "guided campfire key {key:?} matched multiple live rest actions"
                )),
            };
        }
        if is_grid_screen(state) {
            if grid_confirm_up(state) {
                return bind_matching_live_action(state, "CONFIRM", |action| {
                    action.kind == LegalActionKind::Confirm
                        && action.label.eq_ignore_ascii_case("confirm")
                });
            }
            let Some(target) = target_card.as_ref().map(|card| card.raw.as_str()) else {
                return Err("guided campfire grid has no target card".to_owned());
            };
            return first_card_label_match(state, target).ok_or_else(|| {
                format!("guided campfire target {target:?} has no live grid label match")
            });
        }
    }
    Err(format!(
        "SlayTheData guided step {} has no dynamic binding",
        step.code
    ))
}

fn should_override_campfire_with_rest(state: &LiveState) -> bool {
    let summary = state.raw.get("summary");
    let current_hp = summary
        .and_then(|value| value.get("current_hp"))
        .and_then(serde_json::Value::as_i64);
    let max_hp = summary
        .and_then(|value| value.get("max_hp"))
        .and_then(serde_json::Value::as_i64);
    let below_half = matches!((current_hp, max_hp), (Some(current), Some(maximum)) if maximum > 0 && current * 2 < maximum);

    below_half && !live_state_has_relic(state, "Dream Catcher")
}

fn live_state_has_relic(state: &LiveState, target: &str) -> bool {
    let target = normalize_live_label(target).replace(' ', "");
    [
        state.raw.pointer("/summary/relics"),
        state.raw.pointer("/current_state/message/relics"),
        state
            .raw
            .pointer("/current_state/message/game_state/relics"),
    ]
    .into_iter()
    .flatten()
    .filter_map(serde_json::Value::as_array)
    .flatten()
    .any(|relic| {
        [relic.get("id"), relic.get("name")]
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .any(|value| normalize_live_label(value).replace(' ', "") == target)
    })
}

fn bind_neow_followup_grid_action(state: &LiveState) -> Result<&LegalAction, String> {
    if grid_confirm_up(state) {
        return bind_matching_live_action(state, "CONFIRM", |action| {
            action.kind == LegalActionKind::Confirm && action.label.eq_ignore_ascii_case("confirm")
        });
    }
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| {
            action.enabled
                && matches!(
                    action.kind,
                    LegalActionKind::ChooseReward | LegalActionKind::ChooseNeow
                )
        })
        .collect::<Vec<_>>();
    let selected_count = grid_selected_card_count(state);
    matches.get(selected_count).copied().ok_or_else(|| {
        format!(
            "Neow follow-up grid has no unselected live card choice after {selected_count} selections"
        )
    })
}

fn first_enabled_reward_choice(state: &LiveState) -> Option<&LegalAction> {
    state
        .legal_actions
        .iter()
        .find(|action| action.enabled && action.kind == LegalActionKind::ChooseReward)
}

fn reward_flush_action_before_high_level_step<'a>(
    state: &'a LiveState,
    step_label: &str,
) -> Result<&'a LegalAction, String> {
    reward_choice_by_label(state, "gold")
        .or_else(|| reward_choice_by_label(state, "potion"))
        .or_else(|| first_reward_choice_by_label(state, "relic"))
        .or_else(|| {
            (live_screen_type(state) == Some("BOSS_REWARD"))
                .then(|| first_enabled_reward_choice(state))
                .flatten()
        })
        .or_else(|| skip_reward_action(state))
        .ok_or_else(|| {
            format!(
            "{step_label} is waiting behind a reward screen with no gold, potion, or skip action"
        )
        })
}

fn first_reward_choice_by_label<'a>(state: &'a LiveState, label: &str) -> Option<&'a LegalAction> {
    state.legal_actions.iter().find(|action| {
        action.enabled
            && action.kind == LegalActionKind::ChooseReward
            && action.label.eq_ignore_ascii_case(label)
    })
}

fn reward_choice_by_label<'a>(state: &'a LiveState, label: &str) -> Option<&'a LegalAction> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| {
            action.enabled
                && action.kind == LegalActionKind::ChooseReward
                && action.label.eq_ignore_ascii_case(label)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Some(action),
        _ => None,
    }
}

fn skip_reward_action(state: &LiveState) -> Option<&LegalAction> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| {
            action.enabled
                && (action.kind == LegalActionKind::SkipReward
                    || action.label.eq_ignore_ascii_case("skip")
                    || action.label.eq_ignore_ascii_case("proceed"))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Some(action),
        _ => None,
    }
}

fn bind_map_step_to_live_action_with_route_suffix<'a>(
    state: &'a LiveState,
    steps: &[SlayTheDataPreflightStep],
    index: usize,
    symbol: &str,
) -> Result<&'a LegalAction, String> {
    if symbol.eq_ignore_ascii_case("B") {
        let boss_actions = state
            .legal_actions
            .iter()
            .filter(|action| {
                action.enabled
                    && action.kind == LegalActionKind::ChooseMapNode
                    && action.label.eq_ignore_ascii_case("boss")
            })
            .collect::<Vec<_>>();
        return match boss_actions.as_slice() {
            [action] => Ok(action),
            [] => Err("pending boss room has no enabled live boss action".to_owned()),
            _ => Err("pending boss room has multiple enabled live boss actions".to_owned()),
        };
    }
    let symbol_matches = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
        .filter(|action| map_action_matches_symbol(state, action, symbol))
        .collect::<Vec<_>>();
    match symbol_matches.as_slice() {
        [] => {
            return Err(format!(
                "pending room resolution route symbol {symbol:?} has no live map match"
            ));
        }
        [action] => return Ok(action),
        _ => {}
    }

    let future_route = remaining_route_symbols(steps, index);
    if future_route.is_empty() {
        return Ok(symbol_matches[0]);
    }
    let live_route_matches = symbol_matches
        .iter()
        .copied()
        .filter(|action| live_map_action_can_match_route(state, action, &future_route))
        .collect::<Vec<_>>();
    match live_route_matches.as_slice() {
        [action] => return Ok(action),
        [action, ..] => return Ok(action),
        [] => {}
    }
    // Historical SlayTheData runs can have a different live map layout even
    // when the current room symbol is still available. Preserve as much of the
    // recorded route as the live map permits instead of stopping solely because
    // no branch can reproduce the complete future suffix.
    if let Some((action, _)) = symbol_matches
        .iter()
        .copied()
        .filter_map(|action| {
            live_map_action_route_prefix_len(state, action, &future_route)
                .map(|prefix_len| (action, prefix_len))
        })
        .max_by_key(|(_, prefix_len)| *prefix_len)
    {
        return Ok(action);
    }
    let run = sim_run_state(state).ok_or_else(|| {
        format!(
            "pending room resolution route symbol {symbol:?} has multiple live map matches and no simulator map snapshot for remaining route check"
        )
    })?;
    let idle = run_completed_for_route_lookahead(run);
    let legal_map_actions = core_map_actions(&idle).map_err(|error| {
        format!("simulator rejected route lookahead state before map choice: {error}")
    })?;
    let mut route_matches = Vec::new();
    for action in symbol_matches {
        let Some(slot) = live_map_action_slot(action) else {
            continue;
        };
        let Some(map_action) = legal_map_actions.get(slot).copied() else {
            continue;
        };
        if !core_map_action_matches_symbol(&idle, map_action, symbol) {
            continue;
        }
        let next = apply_run_decision_action(&idle, RunDecisionAction::Map(map_action))
            .map_err(|error| format!("simulator rejected route lookahead map choice: {error}"))?;
        if run_state_can_match_route_suffix(next, &future_route).map_err(|error| {
            format!("simulator rejected remaining route lookahead state: {error}")
        })? {
            route_matches.push(action);
        }
    }
    match route_matches.as_slice() {
        [action] => Ok(action),
        [] => Err(format!(
            "pending room resolution route symbol {symbol:?} cannot match remaining SlayTheData route {:?} from any live map choice",
            future_route
        )),
        [action, ..] => Ok(action),
    }
}

fn live_map_action_route_prefix_len(
    state: &LiveState,
    action: &LegalAction,
    route: &[String],
) -> Option<usize> {
    let next_nodes = state
        .raw
        .pointer("/current_state/message/game_state/screen_state/next_nodes")
        .or_else(|| state.raw.pointer("/summary/screen_state/next_nodes"))?
        .as_array()?;
    let map = state
        .raw
        .pointer("/current_state/message/game_state/map")
        .or_else(|| state.raw.pointer("/summary/map"))?
        .as_array()?;
    let slot = live_map_action_slot(action)?;
    let start = next_nodes.get(slot)?;
    let start = if start.get("children").is_some() {
        start
    } else {
        let start_x = start.get("x").and_then(serde_json::Value::as_i64);
        let start_symbol = start.get("symbol").and_then(serde_json::Value::as_str);
        map.iter()
            .filter(|candidate| {
                candidate.get("x").and_then(serde_json::Value::as_i64) == start_x
                    && candidate.get("symbol").and_then(serde_json::Value::as_str) == start_symbol
            })
            .min_by_key(|candidate| {
                candidate
                    .get("y")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(i64::MAX)
            })?
    };
    Some(live_map_node_route_prefix_len(map, start, route))
}

fn live_map_node_route_prefix_len(
    map: &[serde_json::Value],
    node: &serde_json::Value,
    route: &[String],
) -> usize {
    let Some((symbol, rest)) = route.split_first() else {
        return 0;
    };
    let Some(children) = node.get("children").and_then(serde_json::Value::as_array) else {
        return usize::from(symbol.eq_ignore_ascii_case("B"));
    };
    children
        .iter()
        .filter_map(|child| {
            let child_x = child.get("x").and_then(serde_json::Value::as_i64);
            let child_y = child.get("y").and_then(serde_json::Value::as_i64);
            map.iter().find(|candidate| {
                candidate.get("x").and_then(serde_json::Value::as_i64) == child_x
                    && candidate.get("y").and_then(serde_json::Value::as_i64) == child_y
                    && candidate
                        .get("symbol")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|candidate_symbol| candidate_symbol == symbol)
            })
        })
        .map(|child| 1 + live_map_node_route_prefix_len(map, child, rest))
        .max()
        .unwrap_or(0)
}

fn live_map_action_can_match_route(
    state: &LiveState,
    action: &LegalAction,
    route: &[String],
) -> bool {
    let Some(next_nodes) = state
        .raw
        .pointer("/current_state/message/game_state/screen_state/next_nodes")
        .or_else(|| state.raw.pointer("/summary/screen_state/next_nodes"))
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    let Some(map) = state
        .raw
        .pointer("/current_state/message/game_state/map")
        .or_else(|| state.raw.pointer("/summary/map"))
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    let Some(slot) = live_map_action_slot(action) else {
        return false;
    };
    let Some(start) = next_nodes.get(slot) else {
        return false;
    };
    let start = if start.get("children").is_some() {
        start
    } else {
        let start_x = start.get("x").and_then(serde_json::Value::as_i64);
        let start_symbol = start.get("symbol").and_then(serde_json::Value::as_str);
        let Some(start) = map
            .iter()
            .filter(|candidate| {
                candidate.get("x").and_then(serde_json::Value::as_i64) == start_x
                    && candidate.get("symbol").and_then(serde_json::Value::as_str) == start_symbol
            })
            .min_by_key(|candidate| {
                candidate
                    .get("y")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(i64::MAX)
            })
        else {
            return false;
        };
        start
    };
    live_map_node_can_match_route(map, start, route)
}

fn live_map_node_can_match_route(
    map: &[serde_json::Value],
    node: &serde_json::Value,
    route: &[String],
) -> bool {
    let Some((symbol, rest)) = route.split_first() else {
        return true;
    };
    let Some(children) = node.get("children").and_then(serde_json::Value::as_array) else {
        return symbol.eq_ignore_ascii_case("B");
    };
    children.iter().any(|child| {
        let child_x = child.get("x").and_then(serde_json::Value::as_i64);
        let child_y = child.get("y").and_then(serde_json::Value::as_i64);
        let Some(next) = map.iter().find(|candidate| {
            candidate.get("x").and_then(serde_json::Value::as_i64) == child_x
                && candidate.get("y").and_then(serde_json::Value::as_i64) == child_y
        }) else {
            return false;
        };
        next.get("symbol")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|next_symbol| next_symbol == symbol)
            && live_map_node_can_match_route(map, next, rest)
    })
}

fn remaining_route_symbols(steps: &[SlayTheDataPreflightStep], index: usize) -> Vec<String> {
    let mut route = Vec::new();
    for symbol in steps
        .iter()
        .skip(index.saturating_add(1))
        .filter(|step| {
            matches!(
                step.code.as_str(),
                "pending_room_resolution" | "legal_map_room"
            )
        })
        .filter_map(route_symbol_from_step)
    {
        route.push(symbol.to_owned());
        if symbol.eq_ignore_ascii_case("B") {
            break;
        }
    }
    route
}

fn route_symbol_from_step(step: &SlayTheDataPreflightStep) -> Option<&str> {
    match step.intent.as_ref() {
        Some(SlayTheDataReplayStepKind::MapRoom { symbol }) => Some(symbol),
        Some(_) => None,
        None => {
            #[cfg(test)]
            {
                legacy_quoted_after(&step.message, "route symbol \"")
            }
            #[cfg(not(test))]
            {
                None
            }
        }
    }
}

fn shop_purchase_from_step(step: &SlayTheDataPreflightStep) -> Option<&str> {
    match step.intent.as_ref() {
        Some(SlayTheDataReplayStepKind::ShopPurchase { item, .. }) => Some(item),
        Some(_) => None,
        None => {
            #[cfg(test)]
            {
                legacy_quoted_after(&step.message, "shop purchase \"")
            }
            #[cfg(not(test))]
            {
                None
            }
        }
    }
}

fn shop_purge_target_from_step(step: &SlayTheDataPreflightStep) -> Option<&str> {
    match step.intent.as_ref() {
        Some(SlayTheDataReplayStepKind::ShopPurge { card }) => Some(&card.raw),
        Some(_) => None,
        None => {
            #[cfg(test)]
            {
                legacy_quoted_after(&step.message, "shop purge target \"")
            }
            #[cfg(not(test))]
            {
                None
            }
        }
    }
}

pub(crate) fn is_new_act_entry_map(state: &LiveState) -> bool {
    if state.phase != LivePhase::Map {
        return false;
    }
    let game_state = state
        .raw
        .pointer("/current_state/message/game_state")
        .or_else(|| state.raw.get("summary"));
    let Some(game_state) = game_state else {
        return false;
    };
    let act = game_state
        .get("act")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    if act <= 1 {
        return false;
    }
    let Some(screen_state) = game_state.get("screen_state") else {
        return false;
    };
    screen_state
        .get("first_node_chosen")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|chosen| !chosen)
        || screen_state
            .pointer("/current_node/y")
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|y| y < 0)
}

fn is_act_boss_room(state: &LiveState) -> bool {
    matches!(state.phase, LivePhase::Combat | LivePhase::Reward)
        && state
            .raw
            .pointer("/current_state/message/game_state/room_type")
            .or_else(|| state.raw.pointer("/summary/room_type"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|room| room.eq_ignore_ascii_case("MonsterRoomBoss"))
}

fn sim_run_state(state: &LiveState) -> Option<RunState> {
    state
        .raw
        .pointer("/sim_run_state")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn live_map_action_slot(action: &LegalAction) -> Option<usize> {
    action
        .command
        .get("command")
        .and_then(serde_json::Value::as_str)
        .and_then(command_slot)
}

fn command_slot(command: &str) -> Option<usize> {
    command
        .trim()
        .strip_prefix("CHOOSE ")
        .and_then(|slot| slot.parse().ok())
}

fn run_state_can_match_route_suffix(run: RunState, route: &[String]) -> SimResult<bool> {
    let Some((symbol, rest)) = route.split_first() else {
        return Ok(true);
    };
    let idle = run_completed_for_route_lookahead(run);
    let actions = core_map_actions(&idle)?;
    if actions.is_empty() {
        return Ok(current_run_room_kind(&idle) == Some(RoomKind::Boss));
    }
    for action in actions {
        if !core_map_action_matches_symbol(&idle, action, symbol) {
            continue;
        }
        let next = apply_run_decision_action(&idle, RunDecisionAction::Map(action))?;
        if run_state_can_match_route_suffix(next, rest)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn core_map_actions(run: &RunState) -> SimResult<Vec<MapAction>> {
    legal_run_decision_actions(run)?
        .into_iter()
        .map(|action| match action {
            RunDecisionAction::Map(action) => Ok(action),
            _ => Err(SimError::InvalidState(
                "idle route lookahead exposed a non-map decision",
            )),
        })
        .collect()
}

fn current_run_room_kind(run: &RunState) -> Option<RoomKind> {
    run.map
        .as_ref()
        .and_then(|map| map.map.node(map.current_node))
        .map(|node| node.room_kind)
}

fn run_completed_for_route_lookahead(mut run: RunState) -> RunState {
    run.phase = RunPhase::Idle;
    run.combat = None;
    run.reward = None;
    run.event = None;
    run.shop = None;
    run.shop_merchant_open = false;
    run.card_grid = None;
    run.match_and_keep = None;
    run.treasure_room = None;
    run.rest_room_complete = false;
    run
}

fn core_map_action_matches_symbol(run: &RunState, action: MapAction, symbol: &str) -> bool {
    core_map_action_room_kind(run, action).is_some_and(|kind| room_kind_symbol(kind) == symbol)
}

fn core_map_action_room_kind(run: &RunState, action: MapAction) -> Option<RoomKind> {
    let MapAction::ChooseNode { node_id } = action;
    run.map
        .as_ref()
        .and_then(|map| map.map.node(node_id))
        .map(|node| node.room_kind)
}

fn room_kind_symbol(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Combat => "M",
        RoomKind::Elite => "E",
        RoomKind::Event => "?",
        RoomKind::Rest => "R",
        RoomKind::Shop => "$",
        RoomKind::Treasure => "T",
        RoomKind::Boss => "B",
        RoomKind::Victory => "V",
    }
}

fn map_action_matches_symbol(state: &LiveState, action: &LegalAction, symbol: &str) -> bool {
    let Some(nodes) = state
        .raw
        .pointer("/current_state/message/game_state/screen_state/next_nodes")
        .or_else(|| state.raw.pointer("/summary/screen_state/next_nodes"))
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };

    if let Some(slot) = live_map_action_slot(action) {
        if nodes.get(slot).is_some_and(|node| {
            node.get("symbol")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|node_symbol| node_symbol == symbol)
        }) {
            return true;
        }
    }

    let Some(x) = map_action_x(action) else {
        return false;
    };
    nodes.iter().any(|node| {
        node.get("x")
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|node_x| node_x == x)
            && node
                .get("symbol")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|node_symbol| node_symbol == symbol)
    })
}

fn map_action_x(action: &LegalAction) -> Option<i64> {
    let label = action.label.trim();
    let x = label.strip_prefix("x=")?;
    x.parse().ok()
}

fn live_screen_type(state: &LiveState) -> Option<&str> {
    state
        .raw
        .pointer("/summary/screen_type")
        .or_else(|| state.raw.pointer("/summary/screen_name"))
        .and_then(serde_json::Value::as_str)
}

fn current_event_name(state: &LiveState) -> Option<&str> {
    state
        .raw
        .pointer("/current_state/message/game_state/screen_state/event_name")
        .or_else(|| state.raw.pointer("/summary/screen_state/event_name"))
        .and_then(serde_json::Value::as_str)
}

fn unique_event_choice_by_label<'a>(state: &'a LiveState, label: &str) -> Option<&'a LegalAction> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| {
            action.enabled
                && action.kind == LegalActionKind::EventChoice
                && action.label.eq_ignore_ascii_case(label)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Some(action),
        _ => None,
    }
}

fn unique_enabled_event_choice(state: &LiveState) -> Option<&LegalAction> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Some(action),
        _ => None,
    }
}

fn is_grid_screen(state: &LiveState) -> bool {
    live_screen_type(state).is_some_and(|screen| screen == "GRID")
}

fn is_card_reward_screen(state: &LiveState) -> bool {
    if state
        .raw
        .pointer("/current_state/message/game_state/combat_state")
        .is_some()
    {
        return false;
    }
    live_screen_type(state).is_some_and(|screen| screen == "CARD_REWARD" || screen == "GRID")
}

fn grid_confirm_up(state: &LiveState) -> bool {
    state
        .raw
        .pointer("/summary/screen_state/confirm_up")
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/game_state/screen_state/confirm_up")
        })
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
        || state.legal_actions.iter().any(|action| {
            action.enabled
                && action.kind == LegalActionKind::Confirm
                && action.label.eq_ignore_ascii_case("confirm")
        })
}

fn grid_selected_card_count(state: &LiveState) -> usize {
    state
        .raw
        .pointer("/summary/screen_state/selected_cards")
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/game_state/screen_state/selected_cards")
        })
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn campfire_label_matches_key(label: &str, key: &str) -> bool {
    let label = normalize_live_label(label);
    let key = normalize_live_label(key);
    match key.as_str() {
        "smith" => label == "smith" || label == "upgrade",
        "rest" => label == "rest",
        "recall" => label == "recall",
        _ => label == key,
    }
}

fn campfire_grid_label_matches_target(label: &str, target: &str) -> bool {
    let label = normalize_live_label(label);
    let target = normalize_live_label(target);
    let target_base = normalize_card_target_label(&target);
    let compact_label = label.replace(' ', "");
    let compact_target = target.replace(' ', "");
    let compact_target_base = target_base.replace(' ', "");
    label == target
        || label == target_base
        || label.strip_suffix(" +").is_some_and(|base| base == target)
        || label
            .strip_suffix(" +")
            .is_some_and(|base| base == target_base)
        || compact_label == compact_target
        || compact_label == compact_target_base
        || compact_label
            .strip_suffix('+')
            .is_some_and(|base| base == compact_target || base == compact_target_base)
}

fn first_card_label_match<'a>(state: &'a LiveState, target: &str) -> Option<&'a LegalAction> {
    state
        .legal_actions
        .iter()
        .filter(|action| action.enabled)
        .find(|action| campfire_grid_label_matches_target(&action.label, target))
}

fn bind_match_and_keep_action<'a>(
    state: &'a LiveState,
    intent: &SlayTheDataReplayStepKind,
) -> Result<Option<&'a LegalAction>, String> {
    let SlayTheDataReplayStepKind::EventChoice { cards_obtained, .. } = intent else {
        return Err("Match and Keep binding requires typed event guidance".to_owned());
    };
    let targets = cards_obtained
        .iter()
        .map(|card| card.base.as_str())
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Ok(None);
    }
    let cards = state
        .raw
        .pointer("/sim_run_state/match_and_keep/cards")
        .ok_or_else(|| "Match and Keep simulator state is unavailable".to_owned())?
        .as_array()
        .ok_or_else(|| "Match and Keep simulator card group is not an array".to_owned())?;
    let first_flipped = state
        .raw
        .pointer("/sim_run_state/match_and_keep/first_flipped_index")
        .and_then(serde_json::Value::as_u64)
        .map(|index| index as usize);
    let second_flipped = state
        .raw
        .pointer("/sim_run_state/match_and_keep/second_flipped_index")
        .and_then(serde_json::Value::as_u64)
        .map(|index| index as usize);
    for target in targets {
        let target_positions = cards
            .iter()
            .enumerate()
            .filter(|(_, card)| !json_bool(card, "matched") && content_matches_target(card, target))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if target_positions.len() >= 2 {
            if first_flipped
                .zip(second_flipped)
                .is_some_and(|(first, second)| {
                    target_positions.contains(&first)
                        && target_positions.contains(&second)
                        && cards
                            .get(first)
                            .zip(cards.get(second))
                            .is_some_and(|(first, second)| {
                                card_content_id(first) == card_content_id(second)
                            })
                })
            {
                continue;
            }
            let candidates = target_positions.iter().copied().filter(|index| {
                first_flipped != Some(*index)
                    && second_flipped != Some(*index)
                    && event_card_action_by_index(state, *index, cards.len()).is_some()
            });
            if let Some(index) = candidates.into_iter().next() {
                return Ok(event_card_action_by_index(state, index, cards.len()));
            }
            return Err("Match and Keep target has no enabled live card slot".to_owned());
        }
        if !match_and_keep_already_matched_target(state, target) {
            return Err(format!(
                "Match and Keep target {target:?} is not present in the simulated card group"
            ));
        }
    }

    let miss_index = choose_match_and_keep_miss_index(cards, first_flipped)
        .ok_or_else(|| "Match and Keep has no safe remaining miss choice".to_owned())?;
    Ok(event_card_action_by_index(state, miss_index, cards.len()))
}

fn match_and_keep_already_matched_target(state: &LiveState, target: &str) -> bool {
    state
        .raw
        .pointer("/sim_run_state/match_and_keep/matched_cards")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .any(|content_id| content_id_matches_target(content_id, target))
}

fn choose_match_and_keep_miss_index(
    cards: &[serde_json::Value],
    first_flipped: Option<usize>,
) -> Option<usize> {
    if let Some(first) = first_flipped {
        let first_content = card_content_id(cards.get(first)?)?;
        return cards
            .iter()
            .enumerate()
            .filter(|(index, card)| {
                *index != first
                    && !json_bool(card, "matched")
                    && !json_bool(card, "revealed")
                    && card_content_id(card).is_some_and(|content| content != first_content)
            })
            .map(|(index, _)| index)
            .next();
    }

    cards
        .iter()
        .enumerate()
        .find(|(_, card)| !json_bool(card, "matched") && !json_bool(card, "revealed"))
        .map(|(index, _)| index)
}

fn event_card_label_index_for_group(
    _state: &LiveState,
    group_index: usize,
    card_count: usize,
) -> Option<usize> {
    match_and_keep_label_index_for_group(group_index, card_count)
}

fn event_card_action_by_index(
    state: &LiveState,
    group_index: usize,
    card_count: usize,
) -> Option<&LegalAction> {
    let label_index = event_card_label_index_for_group(state, group_index, card_count)?;
    let label = format!("card{label_index}");
    if let Some(action) = unique_event_choice_by_label(state, &label) {
        return Some(action);
    }

    let cards = state
        .raw
        .pointer("/sim_run_state/match_and_keep/cards")?
        .as_array()?;
    let visible_ordinal = cards
        .iter()
        .enumerate()
        .filter(|(_, card)| !json_bool(card, "matched"))
        .filter_map(|(candidate_group, _)| {
            event_card_label_index_for_group(state, candidate_group, card_count)
        })
        .filter(|candidate_label| *candidate_label < label_index)
        .count();
    state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
        .nth(visible_ordinal)
}

fn content_matches_target(card: &serde_json::Value, target: &str) -> bool {
    let Some(content_id) = card_content_id(card) else {
        return false;
    };
    card_definition_matches_target(content_id, target)
}

fn content_id_matches_target(content_id: &serde_json::Value, target: &str) -> bool {
    content_id
        .as_u64()
        .map(ContentId::new)
        .is_some_and(|content_id| card_definition_matches_target(content_id, target))
}

fn card_definition_matches_target(content_id: ContentId, target: &str) -> bool {
    get_card_definition(content_id).is_some_and(|definition| {
        campfire_grid_label_matches_target(definition.name, target)
            || campfire_grid_label_matches_target(definition.key, target)
    })
}

fn card_content_id(card: &serde_json::Value) -> Option<ContentId> {
    card.get("content_id")
        .and_then(serde_json::Value::as_u64)
        .map(ContentId::new)
}

fn json_bool(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn normalize_card_target_label(target: &str) -> &str {
    let target = target
        .strip_suffix(" r")
        .or_else(|| target.strip_suffix(" g"))
        .or_else(|| target.strip_suffix(" b"))
        .or_else(|| target.strip_suffix(" p"))
        .unwrap_or(target);
    target
        .strip_suffix("+1")
        .or_else(|| target.strip_suffix(" +1"))
        .unwrap_or(target)
}

fn guided_event_grid_targets(intent: &SlayTheDataReplayStepKind) -> Vec<&str> {
    let SlayTheDataReplayStepKind::EventChoice {
        cards_removed,
        cards_transformed,
        cards_upgraded,
        cards_obtained,
        ..
    } = intent
    else {
        return Vec::new();
    };
    cards_removed
        .iter()
        .chain(cards_transformed)
        .chain(cards_upgraded)
        .chain(cards_obtained)
        .map(|card| card.base.as_str())
        .collect()
}

fn drug_dealer_test_subject_step(intent: &SlayTheDataReplayStepKind) -> bool {
    let SlayTheDataReplayStepKind::EventChoice {
        event_name,
        player_choice,
        ..
    } = intent
    else {
        return false;
    };
    let event = event_name
        .as_deref()
        .map(normalize_live_label)
        .unwrap_or_default()
        .replace(' ', "");
    let choice = player_choice
        .as_deref()
        .map(normalize_live_label)
        .unwrap_or_default()
        .replace(' ', "");
    matches!(event.as_str(), "drugdealer" | "augmenter") && choice == "becametestsubject"
}

fn event_label_matches_choice_for_event(event_name: &str, label: &str, choice: &str) -> bool {
    let event = normalize_live_label(event_name).replace(' ', "");
    let choice = normalize_live_label(choice).replace(' ', "");
    let label = normalize_live_label(label).replace(' ', "");
    if label == choice {
        return true;
    }
    if choice == "ignored" {
        let target = match event.as_str() {
            "thesssserpent" | "thessssserpent" => "disagree",
            "ghosts" | "councilofghosts" => "refuse",
            "wemeetagain" => "attack",
            "anoteforyourself" | "noteforyourself" => "ignore",
            _ => "leave",
        };
        return label == target;
    }
    let targets: &[&str] = match (event.as_str(), choice.as_str()) {
        ("accursedblacksmith" | "ominousforge", "forge") => &["forge"],
        ("accursedblacksmith" | "ominousforge", "rummage") => &["rummage"],
        ("bigfish", "banana") => &["banana"],
        ("bigfish", "donut") => &["donut"],
        ("bigfish", "box") => &["box"],
        ("bigfish", "heal") => &["banana"],
        ("bigfish", "maxhp") => &["donut"],
        ("bigfish", "relic") => &["box"],
        ("bonfireelementals", choice) if choice.starts_with("offered") => {
            &["continue", "offer", "offeracard"]
        }
        ("designer", "singleremove") => &["cleanup"],
        ("designer", "upgradeandremove" | "removal") => &["fullservice"],
        ("designer", "transformedcards" | "transform") => &["cleanup"],
        ("designer", "upgrade" | "triedtoupgrade" | "upgradedtwo") => &["adjustments"],
        ("designer", "punched") => &["getpunched"],
        ("duplicator", "copied") => &["duplicate"],
        ("duplicator", "ignored") => &["leave"],
        ("fountainofcleansing" | "thedivinefountain", "removedcurses") => &["drink"],
        ("goldenshrine", "pray" | "prayed") => &["pray"],
        ("goldenshrine", "desecrate" | "desecrated") => &["desecrate"],
        ("goldenshrine", "ignored") => &["leave"],
        ("deadadventurer", choice) if choice.starts_with("searched") => {
            &["search", "continue", "fight"]
        }
        ("goldenidol", "takewound") => &["take", "outrun"],
        ("goldenidol", "takedamage") => &["smash"],
        ("goldenidol", "losemaxhp") => &["hide"],
        ("wingstatue" | "goldenwing", "gainedgold") => &["destroy"],
        ("wingstatue" | "goldenwing", "cardremoval") => &["pray"],
        ("worldofgoop", "gathergold") => &["gathergold"],
        ("worldofgoop", "leftgold") => &["leaveit"],
        ("thesssserpent" | "thessssserpent", "agreed") => &["agree"],
        ("thesssserpent" | "thessssserpent", "ignored") => &["disagree"],
        ("shininglight", "enteredlight") => &["enter", "enterthelight"],
        ("hypnotizingcoloredmushrooms" | "mushrooms", "foughtmushrooms") => &["stomp", "fight"],
        ("hypnotizingcoloredmushrooms" | "mushrooms", "healed") => &["eat", "heal"],
        ("scrapooze", "success") => &["reachinside", "deeper"],
        ("scrapooze", "fled") => &["leave"],
        ("facetrader", "touch" | "touched") => &["touch"],
        ("facetrader", "trade" | "traded") => &["trade"],
        ("facetrader", "tookdamage") => &["touch"],
        ("anoteforyourself" | "noteforyourself", "tookcard") => &["continue", "takeandgive"],
        ("anoteforyourself" | "noteforyourself", "ignored") => &["ignore"],
        ("secretportal", "tookportal") => &["taketheportal", "continue"],
        ("thejoust", "betonowner") => &["continue", "betfor"],
        ("thejoust", "betonmurderer") => &["continue", "betagainst"],
        ("thewomaninblue", "bought1potion") => &["buy1potion"],
        ("thewomaninblue", "bought2potions") => &["buy2potions"],
        ("thewomaninblue", "bought3potions") => &["buy3potions"],
        ("thewomaninblue", "bought0potions") => &["leave", "getpunched"],
        ("secretportal", "ignored") => &["leave"],
        ("transmorgrifier" | "transmogrifier", "transformed") => &["pray"],
        ("purifier", "purged") => &["pray"],
        ("upgradeshrine", "upgraded") => &["pray"],
        ("wemeetagain", "gavepotion") => &["givepotion"],
        ("wemeetagain", "paidgold") => &["givegold"],
        ("wemeetagain", "gavecard") => &["givecard"],
        ("addict" | "pleadingvagrant", "obtainedrelic") => &["offergold", "buyrelic"],
        ("addict" | "pleadingvagrant", "stolerelic") => &["rob", "stealrelic"],
        ("beggar", "gavegold") => &["givegold"],
        ("colosseum", "foughtnobs") => &["fightnobs"],
        ("colosseum", "fledfromnobs") => &["flee"],
        ("colosseum", "fight") => &["continue", "fight"],
        ("cursedtome", "stopped") => &["stop", "stopreading"],
        ("cursedtome", "obtainedbook") => &["read", "continue", "take", "takethebook"],
        ("drugdealer" | "augmenter", "obtainjax") => &["takejax", "testjax"],
        ("drugdealer" | "augmenter", "becametestsubject") => &["becometestsubject"],
        ("drugdealer" | "augmenter", "injectmutagens") => &["ingestmutagens"],
        ("forgottenaltar", "shedblood") => &["shedblood", "sacrifice"],
        ("forgottenaltar", "smashedaltar") => &["smashaltar"],
        ("forgottenaltar", "gaveidol") => &["giveidol"],
        ("ghosts" | "councilofghosts", "becameaghost") => &["accept"],
        ("ghosts" | "councilofghosts", "ignored") => &["refuse"],
        ("maskedbandits", "foughtbandits") => &["fight"],
        ("maskedbandits", "paidfearfully") => &["pay"],
        ("nest" | "thenest", "stolefromcult") => &["smashandgrab"],
        ("nest" | "thenest", "joinedthecult") => &["stayinline"],
        ("thelibrary", "heal") => &["sleep"],
        ("thelibrary", "read") => &["read"],
        ("themausoleum", "opened") => &["opencoffin"],
        ("vampires", "becameavampire") => &["accept"],
        ("vampires", "becameavampirevial") => &["givebloodvial"],
        ("lab", "gotpotions") => &["search"],
        ("falling", "removedskill") => &["continue", "removeaskill"],
        ("falling", "removedpower") => &["continue", "removeapower"],
        ("falling", "removedattack") => &["continue", "removeanattack"],
        ("mindbloom", "fight") => &["iamwar"],
        ("mindbloom", "upgrade") => &["iamawake"],
        ("mindbloom", "gold" | "heal") => &["iamhealthy"],
        ("moaihead" | "themoaihead", "heal") => &["losemaxhpandheal"],
        ("moaihead" | "themoaihead", "gaveidol") => &["givegoldenidol"],
        ("mysterioussphere", "fight") => &["fight", "continue"],
        ("sensorystone", "memory1") => &["continue", "memory1"],
        ("sensorystone", "memory2") => &["continue", "memory2"],
        ("sensorystone", "memory3") => &["continue", "memory3"],
        ("tomboflordredmask", "woremask") => &["wearmask"],
        ("tomboflordredmask", "paid") => &["offer"],
        ("windinghalls", "embracemadness") => &["continue", "embracemadness"],
        ("windinghalls", "writhe") => &["continue", "becomewhole"],
        ("windinghalls", "maxhp") => &["continue", "rejectthecall"],
        ("wheelofchange", "gold") => &["play", "spin", "claimgold"],
        ("wheelofchange", "relic") => &["play", "spin", "claimrelic"],
        ("wheelofchange", "fullheal") => &["play", "spin", "heal"],
        ("wheelofchange", "cursed") => &["play", "spin", "takecurse"],
        ("wheelofchange", "cardremoval") => &["play", "spin", "removecard"],
        ("wheelofchange", "damaged") => &["play", "spin", "takedamage"],
        ("thecleric", "healed") => &["heal", "leave"],
        ("thecleric", "cardremoval") => &["purify"],
        ("backtobasics", "elegance") => &["elegance"],
        ("backtobasics", "simplicity") => &["simplicity"],
        ("livingwall", "forgot") => &["forget"],
        ("livingwall", "changed") => &["change"],
        ("livingwall", "grow" | "grew") => &["grow", "reachinside"],
        ("knowingskull", "potion") => &["apickmeup"],
        ("knowingskull", "gold") => &["riches"],
        ("knowingskull", "card") => &["success"],
        ("knowingskull", "leave") => &["howdoileave"],
        _ => return false,
    };

    targets
        .iter()
        .any(|target| label == *target || label.starts_with(target))
}

fn is_guided_event_step(code: &str) -> bool {
    matches!(code, "guided_event_choice" | "guided_event_sequence")
}

fn preferred_event_label(event_name: &str, choice: &str) -> Option<&'static str> {
    let event = normalize_live_label(event_name).replace(' ', "");
    let choice = normalize_live_label(choice).replace(' ', "");
    match (event.as_str(), choice.as_str()) {
        ("thecleric", "healed") => Some("heal"),
        _ => None,
    }
}

fn shop_label_matches_purchase(label: &str, purchase: &str) -> bool {
    let label = normalize_live_label(label);
    let purchase = normalize_live_label(purchase);
    let purchase_words = purchase.split_whitespace().count();
    label == purchase
        || (purchase_words > 0
            && label
                .split_whitespace()
                .collect::<Vec<_>>()
                .windows(purchase_words)
                .any(|window| window.join(" ") == purchase))
        || compact_shop_label_matches_purchase(&label, &purchase)
}

fn compact_shop_label_matches_purchase(label: &str, purchase: &str) -> bool {
    let compact_label = label.replace(' ', "");
    let mut compact_purchase = purchase.replace(' ', "");
    if compact_purchase.is_empty() {
        return false;
    }
    let mut purchase_without_upgrade_count = None;
    if let Some(plus) = compact_purchase.rfind('+') {
        if compact_purchase[plus + 1..]
            .chars()
            .all(|ch| ch.is_ascii_digit())
        {
            purchase_without_upgrade_count = Some(compact_purchase[..plus].to_owned());
            compact_purchase.truncate(plus + 1);
        }
    }
    if compact_label.starts_with(&compact_purchase)
        || purchase_without_upgrade_count
            .as_deref()
            .is_some_and(|base| compact_label.starts_with(base))
    {
        return true;
    }
    if matches!(compact_purchase.as_str(), "steroidpotion")
        && compact_label.starts_with("flexpotion")
    {
        return true;
    }
    compact_purchase
        .strip_suffix("potion")
        .is_some_and(|without_suffix| {
            !without_suffix.is_empty() && compact_label.starts_with(without_suffix)
        })
}

fn normalize_live_label(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '+' {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_leave_shop_action(state: &LiveState) -> Option<&LegalAction> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::Confirm)
        .filter(|action| {
            action.label.eq_ignore_ascii_case("leave shop")
                || action
                    .command
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|command| command.eq_ignore_ascii_case("LEAVE"))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Some(action),
        _ => None,
    }
}

impl AttachedSlayTheDataRun {
    pub fn mark_sent_after_action(&mut self, index: usize, action: &LegalAction) {
        let Some(step) = self.report.steps.get(index) else {
            return;
        };
        let typed_target_card = matches!(
            step.intent.as_ref(),
            Some(SlayTheDataReplayStepKind::Campfire {
                target_card: Some(_),
                ..
            })
        );
        #[cfg(test)]
        let has_target_card = typed_target_card
            || (step.intent.is_none()
                && legacy_quoted_value(&step.message, "target Some(\"").is_some());
        #[cfg(not(test))]
        let has_target_card = typed_target_card;
        if step.code == "guided_campfire"
            && action.kind == LegalActionKind::RestSite
            && has_target_card
            && !action.label.eq_ignore_ascii_case("rest")
        {
            // Entering Smith/Toke is only a prerequisite when the recorded step
            // names a concrete card. Advance after that card is selected.
            return;
        }
        if recorded_action_advances_step(&step.code, action) {
            self.mark_sent(index);
        }
    }
}

pub(crate) fn is_unsettled_neow_map_state(state: &LiveState) -> bool {
    state.phase == LivePhase::Map
        && state
            .raw
            .pointer("/summary/room_type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|room_type| room_type.eq_ignore_ascii_case("NeowRoom"))
        && !state
            .legal_actions
            .iter()
            .any(|action| action.enabled && action.kind == LegalActionKind::ChooseMapNode)
}

fn recorded_action_advances_step(step_code: &str, action: &LegalAction) -> bool {
    if step_code == "pending_room_resolution" {
        return action.kind == LegalActionKind::ChooseMapNode;
    }
    if matches!(step_code, "pending_card_reward" | "guided_card_reward") {
        // Opening the card-reward grid is only a prerequisite. Keep the recorded
        // pick current until the concrete card (or skip) is sent from that grid.
        return !action.label.eq_ignore_ascii_case("card");
    }
    true
}

#[cfg(test)]
fn legacy_test_intent(
    step: &SlayTheDataPreflightStep,
) -> Result<SlayTheDataReplayStepKind, String> {
    let card = |raw: &str| SlayTheDataCardName {
        raw: raw.to_owned(),
        base: raw.trim_end_matches("+1").to_owned(),
        upgraded: raw.ends_with("+1"),
    };
    let intent = match step.code.as_str() {
        "legal_neow_talk" | "illegal_neow_talk" | "neow_talk_apply_failed" => {
            SlayTheDataReplayStepKind::NeowTalk
        }
        "legal_neow_bonus"
        | "neow_option_not_available"
        | "illegal_neow_bonus_slot"
        | "neow_bonus_apply_failed" => SlayTheDataReplayStepKind::NeowBonus {
            bonus: None,
            cost: None,
        },
        "legal_neow_leave"
        | "pending_neow_followup"
        | "illegal_neow_leave"
        | "neow_leave_apply_failed" => SlayTheDataReplayStepKind::NeowLeave,
        "legal_map_room"
        | "pending_room_resolution"
        | "map_symbol_unmatched"
        | "map_action_apply_failed" => SlayTheDataReplayStepKind::MapRoom {
            symbol: legacy_quoted_after(&step.message, "route symbol \"")
                .unwrap_or_default()
                .to_owned(),
        },
        "legal_card_reward" | "pending_card_reward" | "guided_card_reward" => {
            let picked = legacy_quoted_value(&step.message, "picked=Some(\"")
                .or_else(|| legacy_quoted_after(&step.message, "card reward picked \""))
                .map(card);
            SlayTheDataReplayStepKind::CardReward {
                picked,
                skipped: step.message.contains("skipped=true"),
            }
        }
        "guided_event_choice" | "guided_event_sequence" => {
            let cards = |prefix| {
                legacy_quoted_list(&step.message, prefix)
                    .into_iter()
                    .map(card)
                    .collect::<Vec<_>>()
            };
            SlayTheDataReplayStepKind::EventChoice {
                event_name: legacy_quoted_value(&step.message, "event Some(\"").map(str::to_owned),
                player_choice: legacy_quoted_value(&step.message, "choice Some(\"")
                    .map(str::to_owned),
                cards_obtained: cards("obtained "),
                cards_removed: cards("removed "),
                cards_transformed: cards("transformed "),
                cards_upgraded: cards("upgraded "),
                relics_obtained: legacy_quoted_list(&step.message, "relics obtained ")
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                relics_lost: legacy_quoted_list(&step.message, "lost ")
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            }
        }
        "guided_shop_purchase" => SlayTheDataReplayStepKind::ShopPurchase {
            item: legacy_quoted_after(&step.message, "shop purchase \"")
                .unwrap_or_default()
                .to_owned(),
            base_item: String::new(),
        },
        "guided_shop_purge" => SlayTheDataReplayStepKind::ShopPurge {
            card: card(
                legacy_quoted_after(&step.message, "shop purge target \"").unwrap_or_default(),
            ),
        },
        "guided_campfire" => SlayTheDataReplayStepKind::Campfire {
            key: legacy_quoted_value(&step.message, "campfire key Some(\"").map(str::to_owned),
            target_card: legacy_quoted_value(&step.message, "target Some(\"").map(card),
        },
        "guided_boss_relic" => SlayTheDataReplayStepKind::BossRelic {
            act: step.floor.saturating_sub(1) / 17 + 1,
            picked: None,
        },
        "guided_potion_budget" => SlayTheDataReplayStepKind::PotionBudget { uses_allowed: 0 },
        "combat_encounter_evidence" => SlayTheDataReplayStepKind::CombatEncounter { enemies: None },
        code => {
            return Err(format!(
                "test fixture has no typed intent mapping for {code}"
            ))
        }
    };
    Ok(intent)
}

#[cfg(test)]
fn legacy_quoted_value<'a>(message: &'a str, prefix: &str) -> Option<&'a str> {
    let start = message.find(prefix)? + prefix.len();
    let rest = &message[start..];
    Some(&rest[..rest.find("\")")?])
}

#[cfg(test)]
fn legacy_quoted_after<'a>(message: &'a str, prefix: &str) -> Option<&'a str> {
    let start = message.find(prefix)? + prefix.len();
    let rest = &message[start..];
    Some(&rest[..rest.find('"')?])
}

#[cfg(test)]
fn legacy_quoted_list<'a>(message: &'a str, prefix: &str) -> Vec<&'a str> {
    let Some((_, suffix)) = message.split_once(prefix) else {
        return Vec::new();
    };
    let Some((list, _)) = suffix
        .strip_prefix('[')
        .and_then(|value| value.split_once(']'))
    else {
        return Vec::new();
    };
    list.split(',')
        .filter_map(|value| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .collect()
}

fn bind_matching_live_action<'a>(
    state: &'a LiveState,
    expected_command: &str,
    action_filter: impl Fn(&LegalAction) -> bool,
) -> Result<&'a LegalAction, String> {
    let matches = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled)
        .filter(|action| action_filter(action))
        .filter(|action| {
            action
                .command
                .get("command")
                .and_then(|value| value.as_str())
                .is_some_and(|command| command.eq_ignore_ascii_case(expected_command))
                || action.command == json!({"kind": "choose_neow", "choice": 0})
                    && expected_command.eq_ignore_ascii_case("CHOOSE 0")
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [action] => Ok(action),
        [] => Err(format!(
            "no current live legal action matches SlayTheData command {expected_command:?}"
        )),
        _ => Err(format!(
            "SlayTheData command {expected_command:?} matches multiple live actions"
        )),
    }
}

#[derive(Debug, Clone)]
struct ExpectedLiveContext {
    phase: LivePhase,
    kind: LegalActionKind,
}

impl ExpectedLiveContext {
    fn matches(&self, state: &LiveState, action: &LegalAction) -> bool {
        state.phase == self.phase && action.kind == self.kind
    }
}

fn expected_live_context(
    step: &SlayTheDataPreflightStep,
    descriptor: &SlayTheDataBridgeDescriptor,
) -> Option<ExpectedLiveContext> {
    match (step.code.as_str(), descriptor) {
        (
            "legal_neow_talk" | "legal_neow_bonus" | "legal_neow_leave",
            SlayTheDataBridgeDescriptor::ChooseVisibleOption { .. },
        ) => Some(ExpectedLiveContext {
            phase: LivePhase::Neow,
            kind: LegalActionKind::ChooseNeow,
        }),
        ("legal_map_room", SlayTheDataBridgeDescriptor::ChooseVisibleOption { .. }) => {
            Some(ExpectedLiveContext {
                phase: LivePhase::Map,
                kind: LegalActionKind::ChooseMapNode,
            })
        }
        ("legal_card_reward", SlayTheDataBridgeDescriptor::ChooseVisibleOption { .. }) => {
            Some(ExpectedLiveContext {
                phase: LivePhase::Reward,
                kind: LegalActionKind::ChooseReward,
            })
        }
        ("legal_card_reward", SlayTheDataBridgeDescriptor::SkipVisibleReward) => {
            Some(ExpectedLiveContext {
                phase: LivePhase::Reward,
                kind: LegalActionKind::SkipReward,
            })
        }
        _ => None,
    }
}

fn open_readonly(path: &Path) -> LiveResult<Connection> {
    let conn =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql_error)?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(sql_error)?;
    Ok(conn)
}

fn open_readwrite(path: &Path) -> LiveResult<Connection> {
    let conn =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(sql_error)?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(sql_error)?;
    Ok(conn)
}

fn materialized_raw_json(conn: &Connection, run_id: i64) -> LiveResult<Option<String>> {
    conn.query_row(
        "SELECT raw_event_json FROM run_materialized_json WHERE run_id = ?",
        params![run_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(sql_error)
}

fn build_version_from_raw_json(raw: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let object = value
        .get("event")
        .and_then(Value::as_object)
        .or_else(|| value.as_object())?;
    object
        .get("build_version")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn require_tables(conn: &Connection, required: &[&str]) -> LiveResult<()> {
    let tables = table_names(conn)?;
    let missing = required
        .iter()
        .filter(|name| !tables.iter().any(|table| table == **name))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(LiveError::Blocked(format!(
            "SlayTheData database is missing required table(s): {}",
            missing.join(", ")
        )))
    }
}

fn table_names(conn: &Connection) -> LiveResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .map_err(sql_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(rows)
}

fn index_exists(conn: &Connection, index: &str) -> LiveResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?)",
        params![index],
        |row| row.get(0),
    )
    .map_err(sql_error)
}

fn table_columns(conn: &Connection, table: &str) -> LiveResult<Vec<String>> {
    if !table
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return Err(LiveError::InvalidAction(format!(
            "unsafe table name {table}"
        )));
    }
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(sql_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(rows)
}

fn slaythedata_run_outcome_expr(run_columns: &[String], alias: &str) -> String {
    if run_columns.iter().any(|column| column == "run_outcome") {
        format!(
            "COALESCE({alias}.run_outcome, {})",
            slaythedata_derived_run_outcome_expr(alias)
        )
    } else {
        slaythedata_derived_run_outcome_expr(alias)
    }
}

fn slaythedata_derived_run_outcome_expr(alias: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({alias}.victory, 0) = 0 THEN 'loss'
            WHEN COALESCE({alias}.floor_reached, 0) >= {SLAYTHEDATA_WIN_MIN_FLOOR_REACHED} THEN 'win'
            ELSE 'abandon'
        END"
    )
}

fn ensure_broken_slaythedata_runs_table(conn: &Connection) -> LiveResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS broken_slaythedata_runs (
            run_id INTEGER PRIMARY KEY,
            seed_played TEXT,
            reason TEXT,
            marked_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_broken_slaythedata_runs_seed
        ON broken_slaythedata_runs(seed_played);
        "#,
    )
    .map_err(sql_error)?;
    Ok(())
}

fn ensure_corpus_slaythedata_runs_table(conn: &Connection) -> LiveResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS corpus_slaythedata_runs (
            run_id INTEGER PRIMARY KEY,
            trace_path TEXT NOT NULL,
            added_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .map_err(sql_error)?;
    Ok(())
}

fn slaythedata_search_candidate_limit(filters: &SlayTheDataSearchFilters) -> usize {
    if filters.run_id.is_some()
        || filters
            .seed_played
            .as_deref()
            .is_some_and(|seed| !seed.trim().is_empty())
    {
        return filters.limit.max(1);
    }
    filters.limit.max(1).saturating_mul(100).clamp(250, 2_500)
}

fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<SlayTheDataRunSummary> {
    Ok(SlayTheDataRunSummary {
        id: row.get(0)?,
        seed_played: row.get(1)?,
        build_version: row.get(2)?,
        ascension_level: optional_u8(row, 3)?,
        floor_reached: optional_u32(row, 4)?,
        victory: row.get::<_, i64>(5)? != 0,
        run_outcome: row
            .get::<_, String>(6)?
            .try_into()
            .map_err(rusqlite::Error::InvalidParameterName)?,
        path_length: optional_u32(row, 7)?,
        card_choice_count: optional_u32(row, 8)?,
        event_choice_count: optional_u32(row, 9)?,
        shop_purchase_count: optional_u32(row, 10)?,
        potion_usage_count: optional_u32(row, 11)?,
        neow_bonus: row.get(12)?,
        neow_cost: row.get(13)?,
        guided_score: row.get(14)?,
        materialized: row.get::<_, i64>(15)? != 0,
    })
}

fn optional_u8(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u8>> {
    Ok(row
        .get::<_, Option<i64>>(index)?
        .and_then(|value| u8::try_from(value).ok()))
}

fn optional_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u32>> {
    Ok(row
        .get::<_, Option<i64>>(index)?
        .and_then(|value| u32::try_from(value).ok()))
}

fn is_combat_only_guidance(code: &str) -> bool {
    matches!(code, "combat_encounter_evidence" | "guided_potion_budget")
}

fn step_already_satisfied_by_live_state(
    step: &SlayTheDataPreflightStep,
    state: &LiveState,
) -> bool {
    step.code == "legal_neow_leave" && state.phase == LivePhase::Map
}

fn status_name(status: SlayTheDataPreflightStatus) -> &'static str {
    match status {
        SlayTheDataPreflightStatus::Checked => "checked",
        SlayTheDataPreflightStatus::Guided => "guided",
        SlayTheDataPreflightStatus::Blocked => "blocked",
    }
}

fn blocked(reason_code: &str, message: &str) -> BlockedState {
    BlockedState {
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    }
}

fn sql_error(error: rusqlite::Error) -> LiveError {
    LiveError::Blocked(format!("SlayTheData SQLite error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActionId, LegalActionKind, LivePhase};
    use rusqlite::Connection;
    use std::{
        fs,
        io::Write,
        sync::{Mutex, OnceLock},
        time::SystemTime,
    };
    use sts_core::{FixedMap, MapNode, MapNodeId, MapRunState};

    #[test]
    fn route_lookahead_propagates_invalid_core_state() {
        let mut run = RunState::map_fixture();
        run.deck.push(run.deck[0]);

        let result = run_state_can_match_route_suffix(run, &["M".to_owned()]);

        assert!(matches!(result, Err(SimError::InvalidState(_))));
    }

    #[test]
    fn event_choice_mapping_covers_source_metric_labels() {
        let cases = [
            ("Ominous Forge", "Forge", "Forge"),
            ("Ominous Forge", "Rummage", "Rummage"),
            ("Bonfire Elementals", "Offered Rare", "Offer"),
            ("Designer", "Single Remove", "Clean Up (40 gold)"),
            ("Designer", "Upgrade and Remove", "Full Service (90 gold)"),
            ("Duplicator", "Copied", "Duplicate"),
            ("The Divine Fountain", "Removed Curses", "Drink"),
            ("Golden Shrine", "Pray", "Pray"),
            ("Big Fish", "Banana", "Banana"),
            (
                "Dead Adventurer",
                "Searched '2' times",
                "Search (50%: monster returns)",
            ),
            ("Golden Idol", "Take Wound", "Outrun (obtain Injury)"),
            ("Golden Wing", "Card Removal", "Pray"),
            (
                "World of Goop",
                "Gather Gold",
                "Gather gold (gain 75 gold, lose 11 HP)",
            ),
            ("Shining Light", "Entered Light", "Enter the light"),
            ("The Sssserpent", "Agreed", "Agree"),
            ("Living Wall", "Changed", "Change"),
            ("Hypnotizing Colored Mushrooms", "Fought Mushrooms", "Stomp"),
            ("Mushrooms", "Fought Mushrooms", "Fight"),
            ("Scrap Ooze", "Success", "Reach Inside"),
            ("Scrap Ooze", "Success", "Deeper"),
            ("Face Trader", "Trade", "Trade"),
            (
                "A Note For Yourself",
                "Took Card",
                "Take and Give (Iron Wave)",
            ),
            ("The Joust", "Bet on Owner", "Bet for (50 gold)"),
            ("Secret Portal", "Took Portal", "Take the Portal"),
            (
                "The Woman in Blue",
                "Bought 3 Potions",
                "Buy 3 potions (40 gold)",
            ),
            ("Transmogrifier", "Transformed", "Pray"),
            ("Purifier", "Purged", "Pray"),
            ("Upgrade Shrine", "Upgraded", "Pray"),
            ("We Meet Again!", "Gave Potion", "Give Potion"),
            ("Back to Basics", "Elegance", "Elegance"),
            ("Addict", "Obtained Relic", "Buy Relic"),
            ("Addict", "Obtained Relic", "Offer Gold"),
            ("Beggar", "Gave Gold", "Give Gold"),
            ("Colosseum", "Fought Nobs", "Fight Nobs"),
            ("Cursed Tome", "Stopped", "Stop Reading"),
            ("Cursed Tome", "Stopped", "Stop"),
            ("Drug Dealer", "Obtain JAX", "Take JAX"),
            ("Forgotten Altar", "Shed Blood", "Shed Blood"),
            ("Council of Ghosts", "Became a Ghost", "Accept"),
            ("Masked Bandits", "Fought Bandits", "Fight"),
            (
                "The Nest",
                "Stole From Cult",
                "Smash and grab (gain 50 gold)",
            ),
            ("The Library", "Heal", "Sleep"),
            ("The Mausoleum", "Opened", "Open Coffin"),
            ("Vampires(?)", "Became a vampire (Vial)", "Give Blood Vial"),
            ("Lab", "Got Potions", "Search"),
            ("Falling", "Removed Power", "Remove a Power"),
            ("Mind Bloom", "Upgrade", "I am Awake"),
            ("Mysterious Sphere", "Fight", "Fight"),
            (
                "The Moai Head",
                "Gave Idol",
                "Give Golden Idol (gain 333 gold)",
            ),
            (
                "Sensory Stone",
                "Memory 2",
                "Memory 2 (lose 5 HP, gain 2 colorless cards)",
            ),
            ("Tomb of Lord Red Mask", "Paid", "Offer: 99 Gold"),
            ("Winding Halls", "Max HP", "Reject the Call (lose 4 max HP)"),
            ("Wheel of Change", "Full Heal", "Heal"),
            ("The Cleric", "Card Removal", "Purify"),
            ("Knowing Skull", "POTION", "A Pick Me Up"),
            ("Knowing Skull", "GOLD", "Riches"),
            ("Knowing Skull", "CARD", "Success"),
            ("Knowing Skull", "LEAVE", "How Do I Leave?"),
            ("Designer", "Transformed Cards", "Clean Up"),
            ("Designer", "Upgrade", "Adjustments"),
            ("Designer", "Punched", "Get Punched"),
            ("Golden Idol", "Take Damage", "Smash"),
            ("Golden Idol", "Lose Max HP", "Hide"),
            ("Golden Wing", "Gained Gold", "Destroy"),
            ("World of Goop", "Left Gold", "Leave It"),
            ("The Sssserpent", "Ignored", "Disagree"),
            ("Living Wall", "Forgot", "Forget"),
            ("Living Wall", "Grew", "Grow"),
            ("Living Wall", "Grow", "Reach Inside"),
            ("Hypnotizing Colored Mushrooms", "Healed", "Eat"),
            ("Scrap Ooze", "Fled", "Leave"),
            ("Face Trader", "Touch", "Touch"),
            ("Golden Shrine", "Desecrated", "Desecrate"),
            ("A Note For Yourself", "Ignored", "Ignore"),
            ("The Joust", "Bet on Murderer", "Bet Against"),
            ("The Woman in Blue", "Bought 1 Potion", "Buy 1 Potion"),
            ("The Woman in Blue", "Bought 2 Potions", "Buy 2 Potions"),
            ("The Woman in Blue", "Bought 0 Potions", "Leave"),
            ("We Meet Again!", "Paid Gold", "Give Gold"),
            ("We Meet Again!", "Gave Card", "Give Card"),
            ("We Meet Again!", "Ignored", "Attack"),
            ("Addict", "Stole Relic", "Steal Relic"),
            ("Addict", "Stole Relic", "Rob"),
            ("Back to Basics", "Simplicity", "Simplicity"),
            ("Colosseum", "Fled From Nobs", "Flee"),
            ("Cursed Tome", "Obtained Book", "Take the Book"),
            ("Cursed Tome", "Obtained Book", "Take"),
            ("Drug Dealer", "Became Test Subject", "Become Test Subject"),
            ("Augmenter", "Became Test Subject", "Become Test Subject"),
            ("Drug Dealer", "Inject Mutagens", "Ingest Mutagens"),
            ("Forgotten Altar", "Shed Blood", "Sacrifice"),
            ("Forgotten Altar", "Smashed Altar", "Smash Altar"),
            ("Forgotten Altar", "Gave Idol", "Give Idol"),
            ("Council of Ghosts", "Ignored", "Refuse"),
            ("Masked Bandits", "Paid Fearfully", "Pay"),
            ("The Nest", "Joined the Cult", "Stay in Line"),
            ("The Library", "Read", "Read"),
            ("Vampires(?)", "Became a vampire", "Accept"),
            ("Falling", "Removed Skill", "Remove a Skill"),
            ("Falling", "Removed Attack", "Remove an Attack"),
            ("Mind Bloom", "Fight", "I am War"),
            ("Mind Bloom", "Gold", "I am Healthy"),
            ("Mind Bloom", "Heal", "I am Healthy"),
            ("The Moai Head", "Heal", "Lose Max HP and Heal"),
            ("Sensory Stone", "Memory 1", "Memory 1"),
            ("Sensory Stone", "Memory 3", "Memory 3"),
            ("Tomb of Lord Red Mask", "Wore Mask", "Wear Mask"),
            ("Winding Halls", "Embrace Madness", "Embrace Madness"),
            ("Winding Halls", "Writhe", "Become Whole"),
            ("Wheel of Change", "Gold", "Claim Gold"),
            ("Wheel of Change", "Relic", "Claim Relic"),
            ("Wheel of Change", "Cursed", "Take Curse"),
            ("Wheel of Change", "Card Removal", "Remove Card"),
            ("Wheel of Change", "Damaged", "Take Damage"),
            ("The Cleric", "Healed", "Heal"),
        ];

        for (event, source_choice, live_label) in cases {
            assert!(
                event_label_matches_choice_for_event(event, live_label, source_choice),
                "missing source/live choice mapping: {event} / {source_choice} -> {live_label}"
            );
        }
    }

    #[test]
    fn guided_shining_light_entered_light_binds_live_enter_action() {
        let state = LiveState {
            sequence: 895,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "enter".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_state": {"event_name": "Shining Light"}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 11,
            ordinal: 32,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Shining Light\") choice Some(\"Entered Light\") obtained [] removed [] upgraded [\"Bludgeon\", \"Anger\"] relics obtained [] lost [] is high-level guidance until event choice label/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");

        let selected_state = LiveState {
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Bludgeon".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Anger".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {
                "screen_type": "GRID",
                "screen_state": {
                    "num_cards": 2,
                    "confirm_up": false,
                    "selected_cards": [{"uuid": "first-strike"}]
                }
            }}),
            ..state
        };
        let second_action =
            bind_dynamic_guided_step_to_live_action(&selected_state, &step).unwrap();

        assert_eq!(second_action.id.0, "choose-1");
    }

    #[test]
    fn guided_addict_obtained_relic_uses_source_event_when_live_name_is_empty() {
        let state = LiveState {
            sequence: 1609,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "offer gold".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "rob".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-2".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 2"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "EVENT", "event_name": ""}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 19,
            ordinal: 46,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Addict\") choice Some(\"Obtained Relic\") obtained [] removed [] upgraded [] relics obtained [\"Self Forming Clay\"] lost []".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_addict_obtained_relic_accepts_live_pleading_vagrant_name() {
        let state = LiveState {
            sequence: 3847,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "offer gold".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "rob".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {
                "screen_type": "EVENT",
                "screen_state": {"event_name": "Pleading Vagrant"}
            }}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 19,
            ordinal: 46,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Addict\") choice Some(\"Obtained Relic\") obtained [] removed [] upgraded [] relics obtained [\"Self Forming Clay\"] lost []".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_shining_light_binds_single_leave_followup() {
        let state = LiveState {
            sequence: 896,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "leave".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "EVENT"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 11,
            ordinal: 32,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Shining Light\") choice Some(\"Entered Light\") obtained [] removed [] upgraded [\"Bludgeon\", \"Anger\"] relics obtained [] lost [] is high-level guidance until event choice label/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_selects_nloth_trade_by_recorded_lost_relic() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "Trade Burning Blood".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "Trade Vajra".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "summary": {"screen_state": {"event_name": "N'loth"}}
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 12,
            ordinal: 4,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"N'loth\") choice Some(\"Traded Relic\") obtained [] removed [] upgraded [] relics obtained [\"N'loth's Gift\"] lost [\"Vajra\"]".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step)
            .expect("N'loth trade should bind by the recorded lost relic");
        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_event_choice_binds_event_grid_before_reward_phase() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("card-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "Strike".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 9,
            ordinal: 3,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"The Cleric\") choice Some(\"Card Removal\") obtained [] removed [\"Strike\"] upgraded []".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step)
            .expect("event grid should bind while live phase is Event");
        assert_eq!(action.id.0, "card-0");
    }

    #[test]
    fn search_filters_exportable_supported_runs() {
        let db = temp_db("search");
        create_locator_schema(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (1, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'A', '2020-07-30', 0, 20, 3, 2, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (2, 'IRONCLAD', 0, 40, 0, 0, 0, 1, 'B', '2020-07-30', 1, 40, 9, 4, 2, 1, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (3, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'C', '2020-07-30', 0, 20, 3, 2, 1, 0, '', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (1)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (2)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (3)", [])
            .unwrap();
        drop(conn);

        let rows = SlayTheDataIndex::new(&db)
            .search(&SlayTheDataSearchFilters {
                ascension: Some(0),
                min_floor_reached: 1,
                ..SlayTheDataSearchFilters {
                    character: "IRONCLAD".to_owned(),
                    ascension: None,
                    min_floor_reached: 1,
                    max_floor_reached: None,
                    victory: None,
                    run_outcome: None,
                    neow_bonus: None,
                    seed_played: None,
                    run_id: None,
                    limit: 10,
                    require_supported: true,
                }
            })
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
        assert_eq!(rows[0].guided_score, 10);
        assert!(!rows[0].materialized);
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn search_filters_by_neow_bonus() {
        let db = temp_db("search-neow-bonus");
        create_locator_schema(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (1, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'A', '2020-07-30', 0, 20, 3, 2, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (2, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'B', '2020-07-30', 0, 20, 3, 2, 1, 0, 'TEN_PERCENT_HP_BONUS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (1), (2)", [])
            .unwrap();
        drop(conn);

        let rows = SlayTheDataIndex::new(&db)
            .search(&SlayTheDataSearchFilters {
                character: "IRONCLAD".to_owned(),
                ascension: Some(0),
                min_floor_reached: 1,
                neow_bonus: Some("TEN_PERCENT_HP_BONUS".to_owned()),
                max_floor_reached: None,
                victory: None,
                run_outcome: None,
                seed_played: None,
                run_id: None,
                limit: 10,
                require_supported: true,
            })
            .unwrap();

        assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![2]);
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn search_filters_indexed_run_modes_and_over_max_floor() {
        let db = temp_db("search-standard-runs");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_beta INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_prod INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            INSERT INTO runs VALUES (1, 'IRONCLAD', 0, 56, 0, 0, 0, 1, 0, 0, 'OK', '2020-07-30', 1, 56, 1, 1, 1, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (2, 'IRONCLAD', 0, 56, 1, 0, 0, 1, 0, 0, 'BETA', '2020-07-30', 1, 56, 9, 9, 9, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (3, 'IRONCLAD', 0, 56, 0, 1, 0, 1, 0, 0, 'DAILY', '2020-07-30', 1, 56, 9, 9, 9, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (4, 'IRONCLAD', 0, 56, 0, 0, 1, 1, 0, 0, 'ENDLESS', '2020-07-30', 1, 56, 9, 9, 9, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (5, 'IRONCLAD', 0, 56, 0, 0, 0, 0, 0, 0, 'NONPROD', '2020-07-30', 1, 1, 0, 0, 0, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (6, 'IRONCLAD', 0, 56, 0, 0, 0, 1, 1, 0, 'TRIAL', '2020-07-30', 1, 56, 9, 9, 9, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (7, 'IRONCLAD', 0, 107, 0, 0, 0, 1, 0, 0, 'HIGHFLOOR', '2020-07-30', 1, 56, 9, 9, 9, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO runs VALUES (8, 'IRONCLAD', 0, 56, 0, 0, 0, 1, 0, 0, 'OLD_BUILD', '2022-12-18', 1, 56, 9, 9, 9, 0, 'THREE_CARDS', 'NONE');
            INSERT INTO chunk_runs VALUES (1);
            INSERT INTO chunk_runs VALUES (2);
            INSERT INTO chunk_runs VALUES (3);
            INSERT INTO chunk_runs VALUES (4);
            INSERT INTO chunk_runs VALUES (5);
            INSERT INTO chunk_runs VALUES (6);
            INSERT INTO chunk_runs VALUES (7);
            INSERT INTO chunk_runs VALUES (8);
            "#,
        )
        .unwrap();
        drop(conn);

        let rows = SlayTheDataIndex::new(&db)
            .search(&SlayTheDataSearchFilters {
                character: "IRONCLAD".to_owned(),
                ascension: Some(0),
                min_floor_reached: 1,
                max_floor_reached: None,
                victory: None,
                run_outcome: None,
                neow_bonus: None,
                seed_played: None,
                run_id: None,
                limit: 10,
                require_supported: true,
            })
            .unwrap();

        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 5]
        );
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn search_filters_unrealistic_final_gold_when_index_has_gold() {
        let db = temp_db("search-gold-outliers");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT,
                gold INTEGER
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            INSERT INTO runs VALUES
                (1, 'IRONCLAD', 0, 51, 0, 0, 0, 0, 'LOW', '2020-07-30', 1, 51, 12, 4, 2, 1, 'THREE_CARDS', 'NONE', 2999),
                (2, 'IRONCLAD', 0, 51, 0, 0, 0, 0, 'BOUNDARY', '2020-07-30', 1, 51, 12, 4, 2, 1, 'THREE_CARDS', 'NONE', 3000),
                (3, 'IRONCLAD', 0, 51, 0, 0, 0, 0, 'OUTLIER', '2020-07-30', 1, 51, 12, 4, 2, 1, 'THREE_CARDS', 'NONE', 3001);
            INSERT INTO chunk_runs VALUES (1), (2), (3);
            "#,
        )
        .unwrap();
        drop(conn);

        let rows = SlayTheDataIndex::new(&db)
            .search(&SlayTheDataSearchFilters {
                character: "IRONCLAD".to_owned(),
                ascension: Some(0),
                min_floor_reached: 1,
                max_floor_reached: None,
                victory: None,
                run_outcome: None,
                neow_bonus: None,
                seed_played: None,
                run_id: None,
                limit: 10,
                require_supported: true,
            })
            .unwrap();

        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn search_filters_outcomes_without_migrating_the_database() {
        let db = temp_db("search-outcomes");
        create_locator_schema(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (1, 'IRONCLAD', 0, 57, 0, 0, 0, 0, 'WIN', '2020-07-30', 1, 57, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (2, 'IRONCLAD', 0, 35, 0, 0, 0, 0, 'LOSS', '2020-07-30', 0, 35, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (3, 'IRONCLAD', 0, 19, 0, 0, 0, 0, 'EARLY', '2020-07-30', 1, 19, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        for run_id in [1, 2, 3] {
            conn.execute("INSERT INTO chunk_runs VALUES (?)", params![run_id])
                .unwrap();
        }
        drop(conn);

        let index = SlayTheDataIndex::new(&db);
        let filters_for = |run_outcome| SlayTheDataSearchFilters {
            character: "IRONCLAD".to_owned(),
            ascension: Some(0),
            min_floor_reached: 1,
            max_floor_reached: None,
            victory: None,
            run_outcome: Some(run_outcome),
            neow_bonus: None,
            seed_played: None,
            run_id: None,
            limit: 10,
            require_supported: true,
        };

        let wins = index
            .search(&filters_for(SlayTheDataRunOutcome::Win))
            .unwrap();
        assert_eq!(wins.iter().map(|row| row.id).collect::<Vec<_>>(), vec![1]);
        assert!(wins[0].victory);
        assert_eq!(wins[0].run_outcome, SlayTheDataRunOutcome::Win);

        let losses = index
            .search(&filters_for(SlayTheDataRunOutcome::Loss))
            .unwrap();
        assert_eq!(losses.iter().map(|row| row.id).collect::<Vec<_>>(), vec![2]);
        assert_eq!(losses[0].run_outcome, SlayTheDataRunOutcome::Loss);

        let abandons = index
            .search(&filters_for(SlayTheDataRunOutcome::Abandon))
            .unwrap();
        assert_eq!(
            abandons.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![3]
        );
        assert!(!abandons[0].victory);
        assert_eq!(abandons[0].run_outcome, SlayTheDataRunOutcome::Abandon);

        let conn = Connection::open(&db).unwrap();
        let columns = table_columns(&conn, "runs").unwrap();
        assert!(!columns.iter().any(|column| column == "run_outcome"));
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn mark_broken_hides_matching_seed_from_search() {
        let db = temp_db("mark-broken");
        create_locator_schema(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (1, 'IRONCLAD', 0, 20, 0, 0, 0, 0, 'SAMESEED', '2020-07-30', 0, 20, 3, 2, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (2, 'IRONCLAD', 0, 40, 0, 0, 0, 0, 'SAMESEED', '2020-07-30', 1, 40, 9, 4, 2, 1, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (3, 'IRONCLAD', 0, 30, 0, 0, 0, 0, 'OTHERSEED', '2020-07-30', 0, 30, 5, 1, 0, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        for run_id in [1, 2, 3] {
            conn.execute("INSERT INTO chunk_runs VALUES (?)", params![run_id])
                .unwrap();
        }
        drop(conn);

        let index = SlayTheDataIndex::new(&db);
        let broken = index.mark_broken(1, Some("bad seed")).unwrap();
        assert_eq!(broken.seed_played.as_deref(), Some("SAMESEED"));
        assert_eq!(broken.reason.as_deref(), Some("bad seed"));

        let rows = index
            .search(&SlayTheDataSearchFilters {
                ascension: Some(0),
                min_floor_reached: 1,
                limit: 10,
                ..SlayTheDataSearchFilters {
                    character: "IRONCLAD".to_owned(),
                    ascension: None,
                    min_floor_reached: 1,
                    max_floor_reached: None,
                    victory: None,
                    run_outcome: None,
                    neow_bonus: None,
                    seed_played: None,
                    run_id: None,
                    limit: 10,
                    require_supported: true,
                }
            })
            .unwrap();

        assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![3]);

        assert!(index.unmark_broken(1).unwrap());
        assert!(!index.unmark_broken(1).unwrap());
        let restored = index
            .search(&SlayTheDataSearchFilters {
                character: "IRONCLAD".to_owned(),
                ascension: Some(0),
                min_floor_reached: 1,
                max_floor_reached: None,
                victory: None,
                run_outcome: None,
                neow_bonus: None,
                seed_played: None,
                run_id: None,
                limit: 10,
                require_supported: true,
            })
            .unwrap();
        assert!(restored.iter().any(|row| row.id == 1));
        assert!(restored.iter().any(|row| row.id == 2));
        std::fs::remove_file(db).ok();
    }

    #[test]
    fn corpus_runs_are_excluded_by_default_and_can_be_included() {
        let db = temp_db("mark-corpus");
        create_locator_schema(&db);
        let conn = Connection::open(&db).unwrap();
        for (id, seed) in [(1, "ADDED"), (2, "PENDING")] {
            conn.execute(
                "INSERT INTO runs VALUES (?, 'IRONCLAD', 0, 20, 0, 0, 0, 0, ?, '2020-07-30', 0, 20, 3, 2, 1, 0, 'THREE_CARDS', 'NONE')",
                params![id, seed],
            )
            .unwrap();
            conn.execute("INSERT INTO chunk_runs VALUES (?)", params![id])
                .unwrap();
        }
        drop(conn);

        let index = SlayTheDataIndex::new(&db);
        index
            .mark_in_corpus(1, Path::new("permanent/trace-session-1.jsonl"))
            .unwrap();
        let filters = SlayTheDataSearchFilters {
            ascension: Some(0),
            min_floor_reached: 1,
            limit: 10,
            ..SlayTheDataSearchFilters {
                character: "IRONCLAD".to_owned(),
                ascension: None,
                min_floor_reached: 1,
                max_floor_reached: None,
                victory: None,
                run_outcome: None,
                neow_bonus: None,
                seed_played: None,
                run_id: None,
                limit: 10,
                require_supported: true,
            }
        };

        let default_rows = index.search(&filters).unwrap();
        assert_eq!(
            default_rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2]
        );
        let included_rows = index.search_with_corpus(&filters, true).unwrap();
        assert_eq!(
            included_rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2]
        );

        std::fs::remove_file(db).ok();
    }

    #[test]
    fn command_binding_requires_unique_enabled_action() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Inflame".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Skip".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };

        let action = bind_command_to_live_action(&state, "choose 0").unwrap();
        assert_eq!(action.id.0, "choose-0");
        assert!(bind_command_to_live_action(&state, "choose 2").is_err());
    }

    #[test]
    fn pending_skipped_card_reward_binds_to_proceed() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("proceed".to_owned()),
                kind: LegalActionKind::SkipReward,
                label: "Proceed".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 5,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_card_reward".to_owned(),
            message:
                "card reward choice picked=Some(\"SKIP\") skipped=true is pending because simulator phase is Combat"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn checked_skipped_card_reward_binds_to_proceed_after_live_grid_closes() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("proceed".to_owned()),
                kind: LegalActionKind::SkipReward,
                label: "Proceed".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 11,
            ordinal: 33,
            intent: None,
            status: SlayTheDataPreflightStatus::Checked,
            code: "legal_card_reward".to_owned(),
            message:
                "card reward choice picked=Some(\"SKIP\") skipped=true matched core reward choices"
                    .to_owned(),
            bridge_command: Some(sts_verify::SlayTheDataBridgeCommandHint {
                command: "CHOOSE 3".to_owned(),
                descriptor: sts_verify::SlayTheDataBridgeDescriptor::SkipVisibleReward,
            }),
        };

        let action = bind_step_to_live_action(&state, &step)
            .or_else(|_| bind_dynamic_guided_step_to_live_action(&state, &step))
            .unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn pending_picked_card_reward_opens_card_reward() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-card".to_owned()),
                kind: LegalActionKind::ChooseReward,
                label: "card".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 5,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_card_reward".to_owned(),
            message:
                "card reward choice picked=Some(\"Clothesline\") skipped=false is pending because simulator phase is Combat"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-card");
    }

    #[test]
    fn guided_picked_card_reward_opens_card_reward() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-card".to_owned()),
                kind: LegalActionKind::ChooseReward,
                label: "card".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 2"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 5,
            intent: Some(SlayTheDataReplayStepKind::CardReward {
                picked: Some(SlayTheDataCardName {
                    raw: "Flex".to_owned(),
                    base: "Flex".to_owned(),
                    upgraded: false,
                }),
                skipped: false,
            }),
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_card_reward".to_owned(),
            message: "display text deliberately contains no binding data".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-card");
    }

    #[test]
    fn pending_picked_card_reward_selects_matching_grid_card() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("card-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Pommel Strike".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("card-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Clothesline".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "CARD_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 5,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_card_reward".to_owned(),
            message:
                "card reward choice picked=Some(\"Clothesline\") skipped=false is pending because simulator phase is Combat"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "card-1");
    }

    #[test]
    fn guided_picked_card_reward_selects_matching_card() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("card-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Headbutt".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("card-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Flex".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "CARD_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 5,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_card_reward".to_owned(),
            message: "card reward picked \"Flex\" is not among the current core reward choices"
                .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "card-1");
    }

    #[test]
    fn pending_room_resolution_binds_unique_live_map_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=0".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 6,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "next SlayTheData room is pending until live map choices appear".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_dismisses_ftue_before_waiting_for_map() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Unknown,
            legal_actions: vec![LegalAction {
                id: ActionId("dismiss-ftue".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Dismiss tutorial".to_owned(),
                enabled: true,
                command: json!({"command": "CLICK LEFT 1080 700 250"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "NONE", "screen_name": "FTUE"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 18,
            ordinal: 42,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "route symbol \"M\" cannot be checked until the map appears".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "dismiss-ftue");
    }

    #[test]
    fn pending_room_resolution_rejects_single_map_choice_with_wrong_symbol() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=0".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"}
                ]}}}}
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 16,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"R\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let error = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap_err();

        assert!(error.contains("has no live map match"));
    }

    #[test]
    fn pending_room_resolution_binds_unique_event_leave_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "leave".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 11,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"$\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_binds_unique_event_continue_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "continue".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 11,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"$\" cannot be checked until phase Event resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_binds_mandatory_event_play_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "play".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_state": {"event_name": "Drug Dealer"}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 27,
            ordinal: 72,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"?\" cannot be checked until phase Event resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_binds_mandatory_event_spin_choice() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "spin".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_state": {"event_name": "Wheel of Change"}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 27,
            ordinal: 72,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"M\" cannot be checked until phase Event resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_flushes_unique_wheel_result_stage() {
        let state = LiveState {
            sequence: 9,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "prize?".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_state": {"event_name": "Wheel of Change"}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 27,
            ordinal: 72,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "next room waits for the event".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_binds_neow_leave_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Neow,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseNeow,
                label: "leave".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "EVENT", "choices": ["leave"]}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 5,
            ordinal: 13,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"$\" cannot be checked until phase Event resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_flushes_golden_idol_wound_trap_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "outrun".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "smash".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-2".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "hide".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 2"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "summary": {
                    "floor": 6,
                    "screen_type": "EVENT",
                    "screen_state": {"event_name": "Golden Idol"}
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"?\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_defaults_to_proceed_after_shop_work_is_done() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Unknown,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "shop".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("proceed".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "Proceed".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "SHOP_ROOM"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 5,
            ordinal: 70,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"?\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn pending_room_resolution_enters_shop_when_current_floor_work_remains() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Unknown,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "shop".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("proceed".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "Proceed".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "SHOP_ROOM", "floor": 20}}),
        };
        let pending = SlayTheDataPreflightStep {
            floor: 21,
            ordinal: 50,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "route symbol \"M\" is pending".to_owned(),
            bridge_command: None,
        };
        let attached = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    pending.clone(),
                    SlayTheDataPreflightStep {
                        floor: 20,
                        ordinal: 51,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "guided_shop_purchase".to_owned(),
                        message: "shop purchase \"Membership Card\"".to_owned(),
                        bridge_command: None,
                    },
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let action = attached
            .bind_step_to_live_action(&state, 0, &pending)
            .unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_binds_shop_screen_leave() {
        let state = LiveState {
            sequence: 1690,
            phase: LivePhase::Shop,
            legal_actions: vec![LegalAction {
                id: ActionId("leave".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Leave shop".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "LEAVE"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "SHOP_SCREEN"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 7,
            ordinal: 70,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"M\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "leave");
    }

    #[test]
    fn pending_room_resolution_binds_completed_rest_proceed() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Rest,
            legal_actions: vec![LegalAction {
                id: ActionId("proceed".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Proceed".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "REST"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 8,
            ordinal: 21,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"?\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn pending_room_resolution_binds_chest_open() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Unknown,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "open".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "CHEST"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 10,
            ordinal: 25,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"?\" cannot be checked until phase Combat resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_neow_followup_binds_first_reward_choice() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "shrug it off".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "pommel strike".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 0,
            ordinal: 2,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_neow_followup".to_owned(),
            message: "Neow leave is pending because the selected option moved the simulator to phase Reward"
                .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_neow_followup_binds_proceed_after_empty_reward_screen() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("proceed".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Proceed".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "COMBAT_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 0,
            ordinal: 2,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_neow_followup".to_owned(),
            message: "Neow leave is pending because the selected option moved the simulator to phase Reward"
                .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn pending_neow_followup_binds_leave_after_reward() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Neow,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseNeow,
                label: "leave".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 0,
            ordinal: 2,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_neow_followup".to_owned(),
            message: "Neow leave is pending because the selected option moved the simulator to phase Reward"
                .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn legal_neow_leave_flushes_potion_reward_before_leave() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Gambler's Brew".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Gambler's Brew".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-2".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Flex Potion".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 2"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "COMBAT_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 0,
            ordinal: 3,
            intent: None,
            status: SlayTheDataPreflightStatus::Checked,
            code: "legal_neow_leave".to_owned(),
            message: "Neow leave is legal after the selected immediate Neow option".to_owned(),
            bridge_command: Some(sts_verify::SlayTheDataBridgeCommandHint {
                descriptor: sts_verify::SlayTheDataBridgeDescriptor::ChooseVisibleOption {
                    option_slot: 0,
                },
                command: "CHOOSE 0".to_owned(),
            }),
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_neow_followup_grid_beats_later_neow_leave() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Neow,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseNeow,
                    label: "Strike_R".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseNeow,
                    label: "Defend_R".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    SlayTheDataPreflightStep {
                        floor: 0,
                        ordinal: 2,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "pending_neow_followup".to_owned(),
                        message: "Neow leave is pending because the selected option moved the simulator to phase Reward"
                            .to_owned(),
                        bridge_command: None,
                    },
                    SlayTheDataPreflightStep {
                        floor: 0,
                        ordinal: 3,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Checked,
                        code: "legal_neow_leave".to_owned(),
                        message: "Neow leave is legal after the selected immediate Neow option"
                            .to_owned(),
                        bridge_command: Some(sts_verify::SlayTheDataBridgeCommandHint {
                            descriptor:
                                sts_verify::SlayTheDataBridgeDescriptor::ChooseVisibleOption {
                                    option_slot: 0,
                                },
                            command: "CHOOSE 0".to_owned(),
                        }),
                    },
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (index, action) = session.ready_action(&state).unwrap();

        assert_eq!(index, 0);
        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn legal_neow_leave_binds_grid_card_before_leave() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "strike".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "defend".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![SlayTheDataPreflightStep {
                    floor: 0,
                    ordinal: 2,
                    intent: None,
                    status: SlayTheDataPreflightStatus::Checked,
                    code: "legal_neow_leave".to_owned(),
                    message: "Neow leave is legal after the selected immediate Neow option"
                        .to_owned(),
                    bridge_command: Some(sts_verify::SlayTheDataBridgeCommandHint {
                        descriptor: sts_verify::SlayTheDataBridgeDescriptor::ChooseVisibleOption {
                            option_slot: 0,
                        },
                        command: "CHOOSE 0".to_owned(),
                    }),
                }],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (index, action) = session.ready_action(&state).unwrap();

        assert_eq!(index, 0);
        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn legal_neow_leave_selects_a_distinct_second_grid_card() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "strike".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "defend".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "summary": {
                    "screen_type": "GRID",
                    "screen_state": {"selected_cards": [{"uuid": "selected-strike"}]}
                }
            }),
        };
        let action = bind_neow_followup_grid_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_event_choice_binds_matching_live_label() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "forge".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "rummage".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 10,
            intent: Some(SlayTheDataReplayStepKind::EventChoice {
                event_name: Some("Accursed Blacksmith".to_owned()),
                player_choice: Some("Rummage".to_owned()),
                cards_obtained: Vec::new(),
                cards_removed: Vec::new(),
                cards_transformed: Vec::new(),
                cards_upgraded: Vec::new(),
                relics_obtained: Vec::new(),
                relics_lost: Vec::new(),
            }),
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "display text deliberately contains the wrong choice: Forge".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn drug_dealer_transform_uses_starter_cards_when_source_inputs_are_unrecorded() {
        let state = LiveState {
            sequence: 1047,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "strike".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "defend".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {
                "screen_type": "GRID",
                "screen_state": {"num_cards": 2, "confirm_up": false}
            }}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 27,
            ordinal: 72,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Drug Dealer\") choice Some(\"Became Test Subject\") obtained [\"Combust\", \"Dual Wield\"] removed [] upgraded [] relics obtained [] lost [] is high-level guidance until event choice label/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_wheel_result_binds_unique_prize_button() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "prize!".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {"screen_state": {"event_name": "Wheel of Change"}}
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 7,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Wheel of Change\") choice Some(\"Gold\") obtained [] removed [] upgraded [] relics obtained [] lost [] is high-level guidance until event choice label/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_maps_cleric_card_removal_to_purify() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "heal".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "purify".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"The Cleric\") choice Some(\"Card Removal\") is high-level guidance until event choice label mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_event_choice_maps_wing_statue_card_removal_to_pray() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "pray".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 16,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Wing\") choice Some(\"Card Removal\") obtained [] removed [\"Defend_R\"] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_maps_upgrade_shrine_to_pray() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "pray".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Upgrade Shrine\") choice Some(\"Upgraded\") obtained [] removed [] upgraded [\"Bash\"] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_maps_scrap_ooze_success_to_deeper() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "deeper".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Scrap Ooze\") choice Some(\"Success\") obtained [] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_maps_golden_idol_take_wound_to_take() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "take".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 9,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Idol\") choice Some(\"Take Wound\") obtained [\"Injury\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_maps_lab_potions_to_search() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "search".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "floor": 6,
                    "screen_type": "EVENT",
                    "screen_state": {"event_name": "Lab"}
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 7,
            ordinal: 20,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Lab\") choice Some(\"Got Potions\") obtained [] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_flushes_golden_shrine_pray_followup_leave() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "leave".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "screen_type": "EVENT",
                    "screen_state": {"event_name": "Golden Shrine"}
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Shrine\") choice Some(\"Pray\") obtained [] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_flushes_big_fish_box_followup_leave() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "leave".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "screen_type": "EVENT",
                    "screen_state": {"event_name": "Big Fish"}
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 13,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Big Fish\") choice Some(\"Box\") obtained [] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn event_choice_matches_exact_normalized_label_for_unlisted_event() {
        assert!(event_label_matches_choice_for_event(
            "Liars Game",
            "agree",
            "AGREE"
        ));
        assert!(!event_label_matches_choice_for_event(
            "Liars Game",
            "disagree",
            "AGREE"
        ));
    }

    #[test]
    fn dead_adventurer_aggregate_search_choice_matches_fight_stage() {
        assert!(event_label_matches_choice_for_event(
            "Dead Adventurer",
            "fight",
            "Searched '2' times"
        ));
    }

    #[test]
    fn guided_event_choice_maps_golden_idol_take_wound_to_outrun_trap() {
        let state = golden_idol_trap_state();
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 9,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Idol\") choice Some(\"Take Wound\") obtained [\"Injury\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_maps_golden_idol_damage_and_max_hp_traps() {
        let damage_step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 9,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Idol\") choice Some(\"Take Damage\") obtained [] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };
        let max_hp_step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 9,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Idol\") choice Some(\"Lose Max HP\") obtained [] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };
        let damage_state = golden_idol_trap_state();
        let max_hp_state = golden_idol_trap_state();

        let damage_action =
            bind_dynamic_guided_step_to_live_action(&damage_state, &damage_step).unwrap();
        let max_hp_action =
            bind_dynamic_guided_step_to_live_action(&max_hp_state, &max_hp_step).unwrap();

        assert_eq!(damage_action.id.0, "choose-1");
        assert_eq!(max_hp_action.id.0, "choose-2");
    }

    #[test]
    fn guided_golden_idol_trap_choice_first_takes_the_idol() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "take".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_state": {"event_name": "Golden Idol"}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 9,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Golden Idol\") choice Some(\"Take Damage\") obtained [] removed [] upgraded [] relics obtained [\"Golden Idol\"] lost [] is high-level guidance".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_binds_opened_chest_proceed() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Unknown,
            legal_actions: vec![LegalAction {
                id: ActionId("proceed".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Proceed".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "CHEST"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 17,
            ordinal: 44,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "Act route is waiting behind the opened boss chest".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn guided_event_choice_maps_fountain_removed_curses_to_drink() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "drink".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "leave".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 11,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Fountain of Cleansing\") choice Some(\"Removed Curses\") obtained [] removed [\"Injury\"] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_binds_single_continue_before_grid() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "continue".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Golden Wing\") choice Some(\"Card Removal\") obtained [] removed [\"Defend_R\"] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_binds_match_and_keep_play() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::EventChoice,
                label: "play".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 7,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Match and Keep!\") choice Some(\"1 cards matched\") obtained [\"Limit Break\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_binds_match_and_keep_target_pair() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: (0..4)
                .map(|index| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::EventChoice,
                    label: format!("card{index}"),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": format!("CHOOSE {index}")}),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({
                "sim_run_state": {
                    "match_and_keep": {
                        "first_flipped_index": null,
                        "cards": [
                            {"content_id": sts_core::content::cards::DEFEND_R_ID},
                            {"content_id": sts_core::content::cards::LIMIT_BREAK_ID},
                            {"content_id": sts_core::content::cards::DEFEND_R_ID},
                            {"content_id": sts_core::content::cards::LIMIT_BREAK_ID}
                        ]
                    }
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 7,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Match and Keep!\") choice Some(\"1 cards matched\") obtained [\"Limit Break\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_event_choice_binds_match_and_keep_second_target_card() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: (0..4)
                .map(|index| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::EventChoice,
                    label: format!("card{index}"),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": format!("CHOOSE {index}")}),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({
                "sim_run_state": {
                    "match_and_keep": {
                        "first_flipped_index": 1,
                        "cards": [
                            {"content_id": sts_core::content::cards::DEFEND_R_ID},
                            {"content_id": sts_core::content::cards::LIMIT_BREAK_ID, "revealed": true},
                            {"content_id": sts_core::content::cards::DEFEND_R_ID},
                            {"content_id": sts_core::content::cards::LIMIT_BREAK_ID}
                        ]
                    }
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 7,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Match and Keep!\") choice Some(\"1 cards matched\") obtained [\"Limit Break\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-3");
    }

    #[test]
    fn guided_event_choice_advances_to_second_recorded_match() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: (0..4)
                .map(|index| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::EventChoice,
                    label: format!("card{index}"),
                    enabled: true,
                    command: json!({"command": format!("CHOOSE {index}")}),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({
                "sim_run_state": { "match_and_keep": {
                    "first_flipped_index": null,
                    "matched_cards": [sts_core::content::cards::METALLICIZE_ID],
                    "cards": [
                        {"content_id": sts_core::content::cards::METALLICIZE_ID, "matched": true},
                        {"content_id": sts_core::content::cards::CLEAVE_ID},
                        {"content_id": sts_core::content::cards::METALLICIZE_ID, "matched": true},
                        {"content_id": sts_core::content::cards::CLEAVE_ID}
                    ]
                }}
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 12,
            ordinal: 33,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Match and Keep!\") choice Some(\"2 cards matched\") obtained [\"Metallicize\", \"Cleave\"] removed [] upgraded [] relics obtained [] lost []".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_event_choice_uses_third_click_to_resolve_pair_and_start_next_match() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: [0, 1]
                .into_iter()
                .map(|index| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::EventChoice,
                    label: format!("card{index}"),
                    enabled: true,
                    command: json!({"command": format!("CHOOSE {index}")}),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({
                "sim_run_state": { "match_and_keep": {
                    "first_flipped_index": 2,
                    "second_flipped_index": 3,
                    "matched_cards": [],
                    "cards": [
                        {"content_id": sts_core::content::cards::CLEAVE_ID},
                        {"content_id": sts_core::content::cards::CLEAVE_ID},
                        {"content_id": sts_core::content::cards::METALLICIZE_ID, "revealed": true},
                        {"content_id": sts_core::content::cards::METALLICIZE_ID, "revealed": true}
                    ]
                }}
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 12,
            ordinal: 33,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Match and Keep!\") choice Some(\"2 cards matched\") obtained [\"Metallicize\", \"Cleave\"] removed [] upgraded [] relics obtained [] lost []".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_binds_match_and_keep_second_target_with_canonical_permutation() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: (0..11)
                .map(|index| {
                    let label_index = if index < 2 { index } else { index + 1 };
                    LegalAction {
                        id: ActionId(format!("choose-{index}")),
                        kind: LegalActionKind::EventChoice,
                        label: format!("card{label_index}"),
                        enabled: true,
                        command: json!({"transport": "communication_mod", "command": format!("CHOOSE {index}")}),
                        disabled_reason: None,
                    }
                })
                .collect(),
            raw: json!({
                "sim_run_state": {
                    "match_and_keep": {
                        "first_flipped_index": 6,
                        "cards": [
                            {"content_id": sts_core::content::cards::WRITHE_ID},
                            {"content_id": sts_core::content::cards::GOOD_INSTINCTS_ID},
                            {"content_id": sts_core::content::cards::LIMIT_BREAK_ID},
                            {"content_id": sts_core::content::cards::BASH_ID},
                            {"content_id": sts_core::content::cards::GOOD_INSTINCTS_ID},
                            {"content_id": sts_core::content::cards::CLEAVE_ID},
                            {"content_id": sts_core::content::cards::LIMIT_BREAK_ID, "revealed": true},
                            {"content_id": sts_core::content::cards::WRITHE_ID},
                            {"content_id": sts_core::content::cards::SENTINEL_ID},
                            {"content_id": sts_core::content::cards::CLEAVE_ID},
                            {"content_id": sts_core::content::cards::BASH_ID},
                            {"content_id": sts_core::content::cards::SENTINEL_ID}
                        ]
                    }
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 7,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Match and Keep!\") choice Some(\"1 cards matched\") obtained [\"Limit Break\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.label, "card10");
    }

    #[test]
    fn match_and_keep_derives_identity_grid_offset_from_flipped_slot() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: (0..11)
                .map(|index| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::EventChoice,
                    label: format!("card{index}"),
                    enabled: true,
                    command: json!({"command": format!("CHOOSE {index}")}),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({"sim_run_state": {"match_and_keep": {"first_flipped_index": 11}}}),
        };

        assert_eq!(event_card_label_index_for_group(&state, 3, 12), Some(3));
        assert_eq!(event_card_label_index_for_group(&state, 12, 12), None);
    }

    #[test]
    fn guided_event_choice_binds_match_and_keep_safe_miss_after_target() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: (0..4)
                .map(|index| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::EventChoice,
                    label: format!("card{index}"),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": format!("CHOOSE {index}")}),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({
                "sim_run_state": {
                    "match_and_keep": {
                        "first_flipped_index": 0,
                        "matched_cards": [sts_core::content::cards::LIMIT_BREAK_ID],
                        "cards": [
                            {"content_id": sts_core::content::cards::STRIKE_R_ID, "revealed": true},
                            {"content_id": sts_core::content::cards::DEFEND_R_ID},
                            {"content_id": sts_core::content::cards::DEFEND_R_ID}
                        ]
                    }
                }
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 7,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"Match and Keep!\") choice Some(\"1 cards matched\") obtained [\"Limit Break\"] removed [] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_event_choice_binds_removed_card_on_grid() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Strike".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Defend".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"The Cleric\") choice Some(\"Card Removal\") obtained [] removed [\"Strike_R\"] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn transmogrifier_grid_binds_recorded_transformed_source_not_obtained_output() {
        let state = LiveState {
            sequence: 1041,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "strike".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "defend".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 13,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Transmorgrifier\") choice Some(\"Transformed\") obtained [\"Perfected Strike\"] removed [] transformed [\"Strike_R\"] upgraded [] relics obtained [] lost [] is high-level guidance until event choice label/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_event_choice_confirms_selected_grid_card() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CONFIRM"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "GRID", "screen_state": {"confirm_up": true}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 3,
            ordinal: 10,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message:
                "event Some(\"The Cleric\") choice Some(\"Card Removal\") obtained [] removed [\"Strike_R\"] upgraded [] is high-level guidance until event choice label/grid mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "confirm");
    }

    #[test]
    fn guided_event_choice_selects_next_enabled_target_on_multi_card_grid() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Strike".to_owned(),
                    enabled: false,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: Some("selected".to_owned()),
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Defend".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 7,
            ordinal: 20,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Designer\") choice Some(\"Transformed Cards\") obtained [\"Bash\", \"Shrug It Off\"] removed [\"Strike_R\", \"Defend_R\"] upgraded [] relics obtained [] lost [] is high-level guidance until event choice label/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_shop_purchase_binds_shop_entry_before_items_are_visible() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Unknown,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "shop".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "SHOP_ROOM"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message:
                "shop purchase \"Whirlwind\" is high-level guidance until shop slot mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_shop_purchase_binds_shop_map_node() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=6".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_state": {
                "next_nodes": [{"x": 6, "y": 2, "symbol": "$"}]
            }}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 2,
            ordinal: 8,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message: "shop purchase \"Peace Pipe\" is high-level guidance until shop slot mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn guided_shop_purchase_flushes_reward_gold_before_entering_shop() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-gold".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "gold".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-potion".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "potion".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("skip".to_owned()),
                    kind: LegalActionKind::SkipReward,
                    label: "skip".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "SKIP"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message:
                "shop purchase \"Molten Egg 2\" is high-level guidance until shop slot mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-gold");
    }

    #[test]
    fn guided_shop_purchase_flushes_reward_potion_before_skipping_card() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-potion".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "potion".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("skip".to_owned()),
                    kind: LegalActionKind::SkipReward,
                    label: "skip".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "SKIP"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message:
                "shop purchase \"Molten Egg 2\" is high-level guidance until shop slot mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-potion");
    }

    #[test]
    fn guided_shop_purchase_skips_unmatched_card_reward_before_shop() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("skip".to_owned()),
                kind: LegalActionKind::SkipReward,
                label: "skip".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "SKIP"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message:
                "shop purchase \"Molten Egg 2\" is high-level guidance until shop slot mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "skip");
    }

    #[test]
    fn pending_skipped_card_reward_flushes_gold_before_proceed() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-gold".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "gold".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-potion".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "potion".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("proceed".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "Proceed".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 4,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_card_reward".to_owned(),
            message:
                "card reward choice picked=Some(\"SKIP\") skipped=true is pending because simulator phase is Event"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-gold");
    }

    #[test]
    fn pending_skipped_card_reward_flushes_potion_before_proceed() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-potion".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "potion".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("proceed".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "Proceed".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 1,
            ordinal: 4,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_card_reward".to_owned(),
            message:
                "card reward choice picked=Some(\"SKIP\") skipped=true is pending because simulator phase is Event"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-potion");
    }

    #[test]
    fn pending_room_resolution_flushes_reward_before_map() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-gold".to_owned()),
                kind: LegalActionKind::ChooseReward,
                label: "gold".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "next SlayTheData room is pending until live map choices appear".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-gold");
    }

    #[test]
    fn pending_room_resolution_collects_calling_bell_relics_before_proceeding() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("relic-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "relic".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("relic-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "relic".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("proceed".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "Proceed".to_owned(),
                    enabled: true,
                    command: json!({"command": "PROCEED"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "COMBAT_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 18,
            ordinal: 42,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "route waits for the map".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "relic-0");
    }

    #[test]
    fn pending_room_resolution_confirms_calling_bell_curse_grid() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({"command": "CONFIRM"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {"screen_type": "GRID"},
                "current_state": {"message": {"game_state": {
                    "screen_state": {"confirm_up": true}
                }}}
            }),
        };
        let step = SlayTheDataPreflightStep {
            floor: 18,
            ordinal: 42,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message:
                "route symbol \"M\" cannot be checked until phase Reward resolves back to the map"
                    .to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "confirm");
    }

    #[test]
    fn opening_card_reward_does_not_advance_recorded_card_pick() {
        let open_card_reward = LegalAction {
            id: ActionId("choose-card".to_owned()),
            kind: LegalActionKind::ChooseReward,
            label: "card".to_owned(),
            enabled: true,
            command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
            disabled_reason: None,
        };
        let pick_card = LegalAction {
            label: "Shrug It Off".to_owned(),
            ..open_card_reward.clone()
        };

        assert!(!recorded_action_advances_step(
            "pending_card_reward",
            &open_card_reward
        ));
        assert!(recorded_action_advances_step(
            "pending_card_reward",
            &pick_card
        ));
    }

    #[test]
    fn pending_room_resolution_chooses_boss_relic_instead_of_skipping() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "snecko eye".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("skip".to_owned()),
                    kind: LegalActionKind::SkipReward,
                    label: "skip".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "SKIP"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "BOSS_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 17,
            ordinal: 44,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "next Act route is waiting behind the boss reward".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_checks_remaining_route_before_map_choice() {
        let run = ambiguous_route_run_state();
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=0".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=1".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"},
                    {"x": 1, "symbol": "M"}
                ]}}}},
                "sim_run_state": run
            }),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    route_step(1, 0, "M"),
                    route_step(2, 1, "M"),
                    route_step(3, 2, "?"),
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (_, action) = session.ready_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn pending_room_resolution_prefers_observed_live_map_route() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=0".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=4".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"current_state": {"message": {"game_state": {
                "screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"},
                    {"x": 4, "symbol": "M"}
                ]},
                "map": [
                    {"x": 0, "y": 0, "symbol": "M", "children": [{"x": 0, "y": 1}]},
                    {"x": 4, "y": 0, "symbol": "M", "children": [{"x": 4, "y": 1}]},
                    {"x": 0, "y": 1, "symbol": "M", "children": []},
                    {"x": 4, "y": 1, "symbol": "?", "children": []}
                ]
            }}}}),
        };
        let steps = vec![route_step(18, 0, "M"), route_step(19, 1, "?")];

        let action =
            bind_map_step_to_live_action_with_route_suffix(&state, &steps, 0, "M").unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn legal_map_room_checks_remaining_route_before_initial_map_choice() {
        let run = ambiguous_route_run_state();
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=0".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=1".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"},
                    {"x": 1, "symbol": "M"}
                ]}}}},
                "sim_run_state": run
            }),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    legal_map_route_step(1, 0, "M"),
                    route_step(2, 1, "M"),
                    route_step(3, 2, "?"),
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (_, action) = session.ready_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn legal_map_room_uses_longest_live_prefix_when_historical_route_is_impossible() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=0".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=4".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"current_state": {"message": {"game_state": {
                "screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"},
                    {"x": 4, "symbol": "M"}
                ]},
                "map": [
                    {"x": 0, "y": 0, "symbol": "M", "children": [{"x": 0, "y": 1}]},
                    {"x": 4, "y": 0, "symbol": "M", "children": [{"x": 4, "y": 1}]},
                    {"x": 0, "y": 1, "symbol": "M", "children": []},
                    {"x": 4, "y": 1, "symbol": "?", "children": []}
                ]
            }}}}),
        };
        let steps = vec![
            route_step(1, 0, "M"),
            route_step(2, 1, "?"),
            route_step(3, 2, "E"),
        ];

        let action =
            bind_map_step_to_live_action_with_route_suffix(&state, &steps, 0, "M").unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn pending_room_resolution_matches_map_choice_by_command_slot_before_label_x() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=1".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"},
                    {"x": 2, "symbol": "?"}
                ]}}}},
            }),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![route_step(1, 0, "M")],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (_, action) = session.ready_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    fn legal_map_route_step(floor: u32, ordinal: usize, symbol: &str) -> SlayTheDataPreflightStep {
        SlayTheDataPreflightStep {
            floor,
            ordinal,
            intent: None,
            status: SlayTheDataPreflightStatus::Checked,
            code: "legal_map_room".to_owned(),
            message: format!(
                "route symbol \"{symbol}\" matched legal map action ChooseNode {{ node_id: MapNodeId(1) }}; ambiguity accepted by first legal candidate"
            ),
            bridge_command: Some(sts_verify::SlayTheDataBridgeCommandHint {
                descriptor: sts_verify::SlayTheDataBridgeDescriptor::ChooseVisibleOption {
                    option_slot: 0,
                },
                command: "CHOOSE 0".to_owned(),
            }),
        }
    }

    #[test]
    fn pending_room_resolution_allows_ambiguous_route_suffix() {
        let run = equally_valid_route_run_state();
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=0".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=1".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"},
                    {"x": 1, "symbol": "M"}
                ]}}}},
                "sim_run_state": run
            }),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![route_step(1, 0, "M"), route_step(2, 1, "?")],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (_, action) = session.ready_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn pending_room_resolution_stops_suffix_check_at_act_boss() {
        let run = act_boss_boundary_route_run_state();
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=0".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"next_nodes": [
                    {"x": 0, "symbol": "M"}
                ]}}}},
                "sim_run_state": run
            }),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    route_step(1, 0, "M"),
                    route_step(2, 1, "B"),
                    route_step(3, 2, "M"),
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (_, action) = session.ready_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn remaining_route_symbols_end_at_next_boss() {
        let mut steps = vec![
            route_step(1, 0, "M"),
            route_step(2, 1, "?"),
            route_step(16, 2, "B"),
            route_step(18, 3, "M"),
        ];
        steps[1].code = "legal_map_room".to_owned();

        assert_eq!(remaining_route_symbols(&steps, 0), vec!["?", "B"]);
    }

    #[test]
    fn pending_room_resolution_binds_live_boss_action_at_act_boundary() {
        let state = LiveState {
            sequence: 941,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "boss".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "screen_type": "MAP",
                    "screen_state": {"boss_available": true, "next_nodes": []}
                }
            }),
        };
        let step = route_step(16, 43, "B");

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn attached_run_ready_action_binds_live_boss_action_at_act_boundary() {
        let state = LiveState {
            sequence: 941,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "boss".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "MAP", "screen_state": {"boss_available": true}}}),
        };
        let session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![route_step(16, 43, "B")],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let (_, action) = session.ready_action(&state).unwrap();

        assert_eq!(action.id.0, "choose-0");
    }

    #[test]
    fn progress_alignment_skips_completed_boss_route_on_new_act_map() {
        let state = LiveState {
            sequence: 7689,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=3".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({
                "current_state": {"message": {"game_state": {
                    "act": 2,
                    "floor": 17,
                    "screen_state": {
                        "first_node_chosen": false,
                        "current_node": {"x": 0, "y": -1},
                        "next_nodes": [{"x": 3, "symbol": "M"}]
                    }
                }}},
                "summary": {"floor": 17}
            }),
        };
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                // Some imported traces attribute the completed boss route to
                // floor 18 even though the new Act 2 map reports floor 17.
                steps: vec![route_step(18, 39, "B"), route_step(18, 40, "M")],
            },
            next_step_index: 0,
            blocked: Some(blocked("slaythedata_no_live_action", "stale boss route")),
            last_message: None,
            auto_play_paused: false,
        };

        assert!(session.align_progress_to_live_state(&state));
        assert_eq!(session.next_step_index, 1);
        assert!(session.blocked.is_none());
    }

    #[test]
    fn progress_alignment_skips_boss_route_when_reattached_mid_boss() {
        let state = LiveState {
            sequence: 7652,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({
                "current_state": {"message": {"game_state": {
                    "act": 1,
                    "floor": 16,
                    "room_type": "MonsterRoomBoss",
                    "room_phase": "COMBAT"
                }}},
                "summary": {"floor": 16, "room_type": "MonsterRoomBoss"}
            }),
        };
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    route_step(16, 39, "B"),
                    SlayTheDataPreflightStep {
                        floor: 16,
                        ordinal: 40,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "pending_card_reward".to_owned(),
                        message: "card reward choice picked=Some(\"Double Tap\") skipped=false"
                            .to_owned(),
                        bridge_command: None,
                    },
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        assert!(session.align_progress_to_live_state(&state));
        assert_eq!(session.next_step_index, 1);
        assert_eq!(
            session
                .report
                .steps
                .get(session.next_step_index)
                .map(|step| step.code.as_str()),
            Some("pending_card_reward")
        );
    }

    fn route_step(floor: u32, ordinal: usize, symbol: &str) -> SlayTheDataPreflightStep {
        SlayTheDataPreflightStep {
            floor,
            ordinal,
            intent: Some(SlayTheDataReplayStepKind::MapRoom {
                symbol: symbol.to_owned(),
            }),
            status: SlayTheDataPreflightStatus::Guided,
            code: "pending_room_resolution".to_owned(),
            message: "display-only route guidance".to_owned(),
            bridge_command: None,
        }
    }

    fn ambiguous_route_run_state() -> RunState {
        let mut run = RunState::map_fixture();
        run.map = Some(MapRunState {
            act: 1,
            floor: 0,
            current_node: MapNodeId::new(0),
            map: FixedMap {
                nodes: vec![
                    MapNode {
                        id: MapNodeId::new(0),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(1), MapNodeId::new(2)],
                    },
                    MapNode {
                        id: MapNodeId::new(1),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(3)],
                    },
                    MapNode {
                        id: MapNodeId::new(2),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(4)],
                    },
                    MapNode {
                        id: MapNodeId::new(3),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(5)],
                    },
                    MapNode {
                        id: MapNodeId::new(4),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(5)],
                    },
                    MapNode {
                        id: MapNodeId::new(5),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: Vec::new(),
                    },
                ],
            },
        });
        run
    }

    fn equally_valid_route_run_state() -> RunState {
        let mut run = RunState::map_fixture();
        run.map = Some(MapRunState {
            act: 1,
            floor: 0,
            current_node: MapNodeId::new(0),
            map: FixedMap {
                nodes: vec![
                    MapNode {
                        id: MapNodeId::new(0),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(1), MapNodeId::new(2)],
                    },
                    MapNode {
                        id: MapNodeId::new(1),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(3)],
                    },
                    MapNode {
                        id: MapNodeId::new(2),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(4)],
                    },
                    MapNode {
                        id: MapNodeId::new(3),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: Vec::new(),
                    },
                    MapNode {
                        id: MapNodeId::new(4),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: Vec::new(),
                    },
                ],
            },
        });
        run
    }

    fn act_boss_boundary_route_run_state() -> RunState {
        let mut run = RunState::map_fixture();
        run.map = Some(MapRunState {
            act: 1,
            floor: 0,
            current_node: MapNodeId::new(0),
            map: FixedMap {
                nodes: vec![
                    MapNode {
                        id: MapNodeId::new(0),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(1)],
                    },
                    MapNode {
                        id: MapNodeId::new(1),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(2)],
                    },
                    MapNode {
                        id: MapNodeId::new(2),
                        act: 1,
                        room_kind: RoomKind::Boss,
                        children: Vec::new(),
                    },
                ],
            },
        });
        run
    }

    fn golden_idol_trap_state() -> LiveState {
        LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "outrun".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "smash".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-2".to_owned()),
                    kind: LegalActionKind::EventChoice,
                    label: "hide".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 2"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "summary": {
                    "screen_type": "EVENT",
                    "screen_state": {"event_name": "Golden Idol"}
                }
            }),
        }
    }

    fn test_summary() -> SlayTheDataRunSummary {
        SlayTheDataRunSummary {
            id: 1,
            seed_played: None,
            build_version: None,
            ascension_level: Some(0),
            floor_reached: Some(4),
            victory: false,
            run_outcome: SlayTheDataRunOutcome::Loss,
            path_length: None,
            card_choice_count: None,
            event_choice_count: None,
            shop_purchase_count: None,
            potion_usage_count: None,
            neow_bonus: None,
            neow_cost: None,
            guided_score: 0,
            materialized: true,
        }
    }

    #[test]
    fn unavailable_pending_or_guided_card_reward_is_skipped_after_live_room_diverges() {
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![SlayTheDataPreflightStep {
                    floor: 5,
                    ordinal: 16,
                    intent: None,
                    status: SlayTheDataPreflightStatus::Guided,
                    code: "pending_card_reward".to_owned(),
                    message: "card reward choice picked=Some(\"Double Tap\") skipped=false is pending because simulator phase is Combat".to_owned(),
                    bridge_command: None,
                }],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": {
                    "floor": 6,
                    "screen_type": "EVENT",
                    "screen_state": {"event_name": "Wing Statue"}
                }
            }),
        };

        assert_eq!(
            session.skip_unavailable_pending_card_reward(&state),
            Some(0)
        );
        assert_eq!(session.next_step_index, 1);
        assert!(session
            .last_message
            .as_deref()
            .is_some_and(|message| message.contains("unavailable")));

        session.next_step_index = 0;
        session.report.steps[0].code = "guided_card_reward".to_owned();
        assert_eq!(
            session.skip_unavailable_pending_card_reward(&state),
            Some(0)
        );

        session.next_step_index = 0;
        let same_floor_map = LiveState {
            sequence: 8,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=0".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {
                "floor": 5,
                "screen_type": "MAP",
                "screen_state": {"next_nodes": [{"x": 0, "symbol": "M"}]}
            }}),
        };
        assert_eq!(
            session.skip_unavailable_pending_card_reward(&same_floor_map),
            Some(0)
        );

        session.next_step_index = 0;
        let same_floor_rest = LiveState {
            sequence: 9,
            phase: LivePhase::Rest,
            legal_actions: vec![LegalAction {
                id: ActionId("rest".to_owned()),
                kind: LegalActionKind::RestSite,
                label: "rest".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"floor": 5, "screen_type": "REST"}}),
        };
        assert_eq!(
            session.skip_unavailable_pending_card_reward(&same_floor_rest),
            Some(0)
        );

        session.next_step_index = 0;
        let same_floor_shop_room = LiveState {
            sequence: 10,
            phase: LivePhase::Unknown,
            legal_actions: vec![LegalAction {
                id: ActionId("shop".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "shop".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"floor": 5, "screen_type": "SHOP_ROOM"}}),
        };
        assert_eq!(
            session.skip_unavailable_pending_card_reward(&same_floor_shop_room),
            Some(0)
        );

        session.report.steps.insert(
            0,
            SlayTheDataPreflightStep {
                floor: 6,
                ordinal: 15,
                intent: None,
                status: SlayTheDataPreflightStatus::Guided,
                code: "pending_room_resolution".to_owned(),
                message: "route symbol \"M\"".to_owned(),
                bridge_command: None,
            },
        );
        session.report.steps[1].floor = 6;
        session.next_step_index = 1;
        assert_eq!(
            session.rewind_future_card_reward_to_live_map(&same_floor_map),
            Some(0)
        );
        assert_eq!(session.next_step_index, 0);

        session.report.steps[1].code = "pending_room_resolution".to_owned();
        session.report.steps[1].message = "route symbol \"?\"".to_owned();
        session.next_step_index = 1;
        assert_eq!(
            session.rewind_future_unmatched_route_to_live_map(&same_floor_map),
            Some(0)
        );
    }

    #[test]
    fn completed_same_floor_route_is_skipped_after_event_returns_to_map() {
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![route_step(19, 0, "?"), route_step(20, 1, "M")],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Map,
            legal_actions: Vec::new(),
            raw: json!({"summary": {"floor": 19}}),
        };

        assert_eq!(session.skip_completed_route_on_live_map(&state), Some(0));
        assert_eq!(session.next_step_index, 1);
    }

    #[test]
    fn stale_non_shop_guidance_aligns_to_current_map_but_shop_work_does_not() {
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    SlayTheDataPreflightStep {
                        floor: 11,
                        ordinal: 30,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "combat_encounter_evidence".to_owned(),
                        message: "combat bookkeeping".to_owned(),
                        bridge_command: None,
                    },
                    route_step(11, 31, "R"),
                    SlayTheDataPreflightStep {
                        floor: 11,
                        ordinal: 32,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "guided_campfire".to_owned(),
                        message: "campfire key Some(\"REST\") target None".to_owned(),
                        bridge_command: None,
                    },
                    route_step(12, 33, "?"),
                ],
            },
            next_step_index: 0,
            blocked: Some(blocked("old", "old blocker")),
            last_message: None,
            auto_play_paused: false,
        };
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-5".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=5".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 5"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {
                "floor": 12,
                "screen_type": "MAP",
                "screen_state": {"next_nodes": [{"x": 5, "symbol": "?"}]}
            }}),
        };

        assert_eq!(
            session.align_past_completed_non_shop_guidance(&state),
            Some((1, 3, "pending_room_resolution".to_owned()))
        );
        assert_eq!(session.next_step_index, 3);
        assert!(session.blocked.is_none());

        session.report.steps[1].code = "guided_shop_purge".to_owned();
        session.next_step_index = 1;
        assert_eq!(session.align_past_completed_non_shop_guidance(&state), None);
        assert_eq!(session.next_step_index, 1);

        session.report.steps[1].floor = 12;
        session.report.steps[1].code = "guided_event_choice".to_owned();
        session.next_step_index = 1;
        assert_eq!(
            session.align_past_completed_non_shop_guidance(&state),
            Some((1, 3, "guided_event_choice".to_owned()))
        );
        assert_eq!(session.next_step_index, 3);
    }

    #[test]
    fn completed_campfire_advances_on_map_when_next_route_is_not_yet_bindable() {
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![guided_smith_step(), route_step(8, 17, "M")],
            },
            next_step_index: 0,
            blocked: Some(blocked(
                "guided_campfire",
                "missing recorded upgrade target",
            )),
            last_message: None,
            auto_play_paused: false,
        };
        session.report.steps[0].floor = 7;
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-2".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=2".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 2"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {
                "floor": 7,
                "screen_type": "MAP",
                "screen_state": {"next_nodes": [{"x": 2, "symbol": "?"}]}
            }}),
        };

        assert_eq!(
            session.align_past_completed_non_shop_guidance(&state),
            Some((0, 1, "guided_campfire".to_owned()))
        );
        assert_eq!(session.next_step_index, 1);
        assert!(session.blocked.is_none());
    }

    #[test]
    fn pending_card_reward_is_preserved_while_same_floor_event_can_enter_combat() {
        let mut session = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![SlayTheDataPreflightStep {
                    floor: 8,
                    ordinal: 24,
                    intent: None,
                    status: SlayTheDataPreflightStatus::Guided,
                    code: "pending_card_reward".to_owned(),
                    message: "pending Dead Adventurer combat reward".to_owned(),
                    bridge_command: None,
                }],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Event,
            legal_actions: Vec::new(),
            raw: json!({"summary": {"floor": 8, "screen_type": "EVENT"}}),
        };

        assert_eq!(session.skip_unavailable_pending_card_reward(&state), None);
        assert_eq!(session.next_step_index, 0);
    }

    #[test]
    fn guided_event_choice_flushes_reward_before_event() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("skip".to_owned()),
                kind: LegalActionKind::SkipReward,
                label: "skip".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "SKIP"}),
                disabled_reason: None,
            }],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_event_choice".to_owned(),
            message: "event Some(\"Golden Shrine\") choice Some(\"Pray\") is high-level guidance until event choice label mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "skip");
    }

    #[test]
    fn guided_campfire_flushes_reward_before_rest_site() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-gold".to_owned()),
                kind: LegalActionKind::ChooseReward,
                label: "gold".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "COMBAT_REWARD"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 16,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_campfire".to_owned(),
            message: "campfire key Some(\"SMITH\") target Some(\"Whirlwind\") is high-level guidance until rest/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-gold");
    }

    #[test]
    fn guided_campfire_binds_smith_rest_action() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Rest,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::RestSite,
                    label: "Rest".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::RestSite,
                    label: "Smith".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "REST"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 16,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_campfire".to_owned(),
            message: "campfire key Some(\"SMITH\") target Some(\"Whirlwind\") is high-level guidance until rest/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_campfire_proceeds_when_relics_disable_all_actions() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Rest,
            legal_actions: vec![LegalAction {
                id: ActionId("proceed".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Proceed".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "PROCEED"}),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "screen_type": "REST",
                    "relics": ["Fusion Hammer", "Coffee Dripper"]
                }
            }),
        };
        let step = guided_smith_step();

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "proceed");
    }

    #[test]
    fn guided_campfire_rests_below_half_hp() {
        let mut state = campfire_rest_state(39, 80);
        let step = guided_smith_step();

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-rest");

        state.raw["summary"]["current_hp"] = json!(40);
        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();
        assert_eq!(action.id.0, "choose-smith");
    }

    #[test]
    fn guided_campfire_does_not_rest_with_dream_catcher() {
        let mut state = campfire_rest_state(20, 80);
        state.raw["current_state"] = json!({
            "message": {"relics": [{"id": "DreamCatcher", "name": "Dream Catcher"}]}
        });
        let step = guided_smith_step();

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-smith");
    }

    #[test]
    fn guided_campfire_rests_when_fusion_hammer_disables_smith() {
        let mut state = campfire_rest_state(72, 74);
        state
            .legal_actions
            .retain(|action| action.label.eq_ignore_ascii_case("rest"));
        state.raw["current_state"] = json!({
            "message": {"game_state": {"relics": [
                {"id": "Fusion Hammer", "name": "Fusion Hammer"}
            ]}}
        });
        let step = guided_smith_step();

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-rest");
    }

    #[test]
    fn low_hp_campfire_rest_advances_recorded_smith_step() {
        let step = guided_smith_step();
        let mut attached = AttachedSlayTheDataRun {
            summary: test_summary(),
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![step],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };
        let rest = campfire_rest_state(20, 80)
            .legal_actions
            .into_iter()
            .find(|action| action.label.eq_ignore_ascii_case("rest"))
            .unwrap();

        attached.mark_sent_after_action(0, &rest);

        assert_eq!(attached.next_step_index, 1);
    }

    fn campfire_rest_state(current_hp: i64, max_hp: i64) -> LiveState {
        LiveState {
            sequence: 7,
            phase: LivePhase::Rest,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-rest".to_owned()),
                    kind: LegalActionKind::RestSite,
                    label: "Rest".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-smith".to_owned()),
                    kind: LegalActionKind::RestSite,
                    label: "Smith".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({
                "summary": {
                    "screen_type": "REST",
                    "current_hp": current_hp,
                    "max_hp": max_hp,
                    "relics": []
                }
            }),
        }
    }

    fn guided_smith_step() -> SlayTheDataPreflightStep {
        SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 16,
            intent: Some(SlayTheDataReplayStepKind::Campfire {
                key: Some("SMITH".to_owned()),
                target_card: Some(SlayTheDataCardName {
                    raw: "Whirlwind".to_owned(),
                    base: "Whirlwind".to_owned(),
                    upgraded: false,
                }),
            }),
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_campfire".to_owned(),
            message: "display-only campfire guidance".to_owned(),
            bridge_command: None,
        }
    }

    #[test]
    fn guided_divergence_is_typed_and_keeps_source_build_provenance() {
        let mut summary = test_summary();
        summary.build_version = Some("2020-07-30".to_owned());
        let attached = AttachedSlayTheDataRun {
            summary,
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                steps: vec![guided_smith_step()],
                diagnostics: Vec::new(),
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };

        let divergence = attached
            .guided_divergence(
                0,
                SlayTheDataGuidedDivergenceKind::CompletedGuidancePastLiveFloor,
                "live run legally moved past the recorded campfire",
            )
            .unwrap();

        assert_eq!(divergence.floor, 6);
        assert_eq!(
            divergence.source_build_version.as_deref(),
            Some("2020-07-30")
        );
        assert!(matches!(
            divergence.intent,
            SlayTheDataReplayStepKind::Campfire { .. }
        ));
        let value = serde_json::to_value(divergence).unwrap();
        assert_eq!(value["kind"], "completed_guidance_past_live_floor");
        assert!(value.get("fidelity").is_none());
    }

    #[test]
    fn guided_campfire_binds_target_card_on_smith_grid() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Strike".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "Whirlwind".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 16,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_campfire".to_owned(),
            message: "campfire key Some(\"SMITH\") target Some(\"Whirlwind\") is high-level guidance until rest/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn guided_campfire_matches_compact_hand_of_greed_target() {
        let state = LiveState {
            sequence: 8,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("hand-of-greed".to_owned()),
                kind: LegalActionKind::ChooseReward,
                label: "hand of greed".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 10"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "GRID"}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 20,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_campfire".to_owned(),
            message: "campfire key Some(\"SMITH\") target Some(\"HandOfGreed\") is high-level guidance until rest/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "hand-of-greed");
    }

    #[test]
    fn guided_campfire_confirms_selected_smith_grid_card() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({"transport": "communication_mod", "command": "CONFIRM"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "GRID", "screen_state": {"confirm_up": true}}}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 6,
            ordinal: 16,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_campfire".to_owned(),
            message: "campfire key Some(\"SMITH\") target Some(\"Flame Barrier\") is high-level guidance until rest/grid mapping is connected".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "confirm");
    }

    #[test]
    fn guided_shop_purchase_binds_matching_live_item_label() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Shop,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ShopBuy,
                    label: "Flex - 56 gold".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ShopBuy,
                    label: "Whirlwind - 112 gold".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 12,
            intent: Some(SlayTheDataReplayStepKind::ShopPurchase {
                item: "Whirlwind".to_owned(),
                base_item: "Whirlwind".to_owned(),
            }),
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message: "display text deliberately names Flex".to_owned(),
            bridge_command: None,
        };

        let action = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap();

        assert_eq!(action.id.0, "choose-1");
    }

    #[test]
    fn shop_purchase_matching_accepts_dataset_ids_and_upgrade_counts() {
        for (live_label, recorded_item) in [
            ("Hand Drill - 154 gold", "HandDrill"),
            ("Elixir - 74 gold", "ElixirPotion"),
            ("Panic Button - 102 gold", "PanicButton"),
            ("Blessing of the Forge - 52 gold", "BlessingOfTheForge"),
            ("Flex Potion - 48 gold", "SteroidPotion"),
            ("Trip+ - 91 gold", "Trip+1"),
            ("Metallicize - 75 gold", "Metallicize+1"),
        ] {
            assert!(
                shop_label_matches_purchase(live_label, recorded_item),
                "{recorded_item:?} should match {live_label:?}"
            );
        }
    }

    #[test]
    fn guided_shop_purchase_does_not_leave_when_requested_item_is_absent() {
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Shop,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("choose-0".to_owned()),
                    kind: LegalActionKind::ShopBuy,
                    label: "iron wave".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                LegalAction {
                    id: ActionId("leave".to_owned()),
                    kind: LegalActionKind::Confirm,
                    label: "Leave shop".to_owned(),
                    enabled: true,
                    command: json!({"transport": "communication_mod", "command": "LEAVE"}),
                    disabled_reason: None,
                },
            ],
            raw: json!({}),
        };
        let step = SlayTheDataPreflightStep {
            floor: 4,
            ordinal: 13,
            intent: None,
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purchase".to_owned(),
            message:
                "shop purchase \"PreservedInsect\" is high-level guidance until shop slot mapping is connected"
                    .to_owned(),
            bridge_command: None,
        };

        let error = bind_dynamic_guided_step_to_live_action(&state, &step).unwrap_err();

        assert_eq!(
            error,
            "guided shop purchase \"PreservedInsect\" has no enabled live shop label match"
        );
    }

    #[test]
    fn guided_shop_purge_opens_grid_selects_target_and_confirms() {
        let step = SlayTheDataPreflightStep {
            floor: 14,
            ordinal: 41,
            intent: Some(SlayTheDataReplayStepKind::ShopPurge {
                card: SlayTheDataCardName {
                    raw: "Defend_R".to_owned(),
                    base: "Defend_R".to_owned(),
                    upgraded: false,
                },
            }),
            status: SlayTheDataPreflightStatus::Guided,
            code: "guided_shop_purge".to_owned(),
            message: "display text deliberately names Strike".to_owned(),
            bridge_command: None,
        };
        let room = LiveState {
            sequence: 0,
            phase: LivePhase::Unknown,
            legal_actions: vec![LegalAction {
                id: ActionId("shop".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "shop".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "SHOP_ROOM"}}),
        };
        assert_eq!(
            bind_dynamic_guided_step_to_live_action(&room, &step)
                .unwrap()
                .id
                .0,
            "shop"
        );
        let shop = LiveState {
            sequence: 1,
            phase: LivePhase::Shop,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ShopBuy,
                label: "purge".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "SHOP_SCREEN"}}),
        };
        assert_eq!(
            bind_dynamic_guided_step_to_live_action(&shop, &step)
                .unwrap()
                .id
                .0,
            "choose-0"
        );

        let grid = LiveState {
            sequence: 2,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-3".to_owned()),
                kind: LegalActionKind::ChooseReward,
                label: "defend".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 3"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "GRID", "screen_state": {"for_purge": true, "confirm_up": false}}}),
        };
        assert_eq!(
            bind_dynamic_guided_step_to_live_action(&grid, &step)
                .unwrap()
                .id
                .0,
            "choose-3"
        );

        let confirm = LiveState {
            sequence: 3,
            phase: LivePhase::Reward,
            legal_actions: vec![LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({"command": "CONFIRM"}),
                disabled_reason: None,
            }],
            raw: json!({"summary": {"screen_type": "GRID", "screen_state": {"for_purge": true, "confirm_up": true}}}),
        };
        assert_eq!(
            bind_dynamic_guided_step_to_live_action(&confirm, &step)
                .unwrap()
                .id
                .0,
            "confirm"
        );
    }

    #[test]
    fn unavailable_shop_purchase_is_reported_without_advancing_to_later_purchase() {
        let mut session = AttachedSlayTheDataRun {
            summary: SlayTheDataRunSummary {
                id: 1,
                seed_played: None,
                build_version: None,
                ascension_level: Some(0),
                floor_reached: Some(4),
                victory: false,
                run_outcome: SlayTheDataRunOutcome::Loss,
                path_length: None,
                card_choice_count: None,
                event_choice_count: None,
                shop_purchase_count: Some(2),
                potion_usage_count: None,
                neow_bonus: None,
                neow_cost: None,
                guided_score: 0,
                materialized: true,
            },
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: sts_verify::SlayTheDataSource {
                    kind: sts_verify::SlayTheDataSourceKind::RawRun,
                    run_id: Some(1),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    SlayTheDataPreflightStep {
                        floor: 4,
                        ordinal: 12,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "guided_shop_purchase".to_owned(),
                        message: "shop purchase \"PreservedInsect\"".to_owned(),
                        bridge_command: None,
                    },
                    SlayTheDataPreflightStep {
                        floor: 4,
                        ordinal: 13,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "guided_shop_purchase".to_owned(),
                        message: "shop purchase \"FlameBarrier\"".to_owned(),
                        bridge_command: None,
                    },
                    SlayTheDataPreflightStep {
                        floor: 4,
                        ordinal: 14,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "guided_shop_purge".to_owned(),
                        message: "shop purge target \"Defend_R\"".to_owned(),
                        bridge_command: None,
                    },
                    SlayTheDataPreflightStep {
                        floor: 5,
                        ordinal: 15,
                        intent: None,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "pending_room_resolution".to_owned(),
                        message: "next room".to_owned(),
                        bridge_command: None,
                    },
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };
        let leave = LegalAction {
            id: ActionId("leave".to_owned()),
            kind: LegalActionKind::Confirm,
            label: "Leave shop".to_owned(),
            enabled: true,
            command: json!({"command": "LEAVE"}),
            disabled_reason: None,
        };
        let buy_flame_barrier = LegalAction {
            id: ActionId("choose-1".to_owned()),
            kind: LegalActionKind::ShopBuy,
            label: "Flame Barrier - 77 gold".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 1"}),
            disabled_reason: None,
        };
        let purge = LegalAction {
            id: ActionId("choose-0".to_owned()),
            kind: LegalActionKind::ShopBuy,
            label: "purge".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 0"}),
            disabled_reason: None,
        };
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Shop,
            legal_actions: vec![purge.clone(), buy_flame_barrier.clone(), leave.clone()],
            raw: json!({"summary": {"screen_type": "SHOP_SCREEN", "floor": 4}}),
        };

        session.next_step_index = 2;
        assert!(!session.rewind_to_unresolved_shop_purchase(4, &Default::default()));
        assert_eq!(session.next_step_index, 2);

        session.next_step_index = 0;
        assert!(session.rewind_to_unresolved_shop_purchase(4, &Default::default()));
        assert_eq!(session.next_step_index, 0);

        assert_eq!(
            session.unavailable_shop_purchase(&state),
            Some((0, "PreservedInsect".to_owned()))
        );
        assert_eq!(session.next_step_index, 0);
        assert!(session.ready_action(&state).is_err());

        assert_eq!(
            session.skip_current_shop_purchases(),
            vec![
                (0, "PreservedInsect".to_owned()),
                (1, "FlameBarrier".to_owned()),
            ]
        );
        assert_eq!(session.next_step_index, 2);
        let (index, action) = session.ready_action(&state).unwrap();
        assert_eq!(index, 2);
        assert_eq!(action.id.0, "choose-0");

        let missing_purge_target_state = LiveState {
            raw: json!({
                "current_state": {"message": {"game_state": {"deck": [
                    {"id": "Strike_R", "name": "Strike"}
                ]}}}
            }),
            ..state.clone()
        };
        assert_eq!(
            session.skip_unavailable_shop_purge(&missing_purge_target_state),
            Some((2, "Defend_R".to_owned()))
        );
        assert_eq!(session.next_step_index, 3);

        session.next_step_index = 2;
        assert_eq!(session.skip_completed_shop_purge(5), None);
        assert_eq!(session.skip_completed_shop_purge(4), Some(2));
        assert_eq!(session.next_step_index, 3);
    }

    #[test]
    fn load_or_materialize_run_extracts_raw_event_from_chunk() {
        let root = temp_dir("materialize-chunk-root");
        let chunks = root.join("chunks");
        fs::create_dir_all(&chunks).unwrap();
        let db = root.join("slaythedata.sqlite3");
        create_chunk_schema(&db);
        let chunk_path = chunks.join("000001.jsonl.zst");
        write_zstd_lines(
            &chunk_path,
            &[
                json!({"event":{"seed_played":"OTHER"}}).to_string(),
                json!({"event":{
                    "character_chosen":"IRONCLAD",
                    "ascension_level":0,
                    "seed_played":"CODEX04",
                    "build_version":"2022-12-18",
                    "floor_reached":1,
                    "victory":false,
                    "path_taken":[],
                    "path_per_floor":[],
                    "neow_bonus":"TEN_PERCENT_HP_BONUS",
                    "neow_cost":"NONE"
                }})
                .to_string(),
            ],
        );

        let (summary, raw) = SlayTheDataIndex::new(&db)
            .load_or_materialize_run(7)
            .unwrap();

        assert_eq!(summary.id, 7);
        assert!(summary.materialized);
        assert_eq!(summary.build_version.as_deref(), Some("2022-12-18"));
        assert!(raw.contains(r#""seed_played":"CODEX04""#));
        let conn = Connection::open(&db).unwrap();
        let stored = materialized_raw_json(&conn, 7).unwrap().unwrap();
        assert_eq!(stored, raw);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn default_local_uses_env_configured_database_path() {
        let _guard = env_lock().lock().unwrap();
        let db = temp_db("env-config");
        let previous = std::env::var_os(SLAYTHEDATA_DB_ENV);
        std::env::set_var(SLAYTHEDATA_DB_ENV, &db);

        assert_eq!(SlayTheDataIndex::default_local().db_path, db);

        if let Some(previous) = previous {
            std::env::set_var(SLAYTHEDATA_DB_ENV, previous);
        } else {
            std::env::remove_var(SLAYTHEDATA_DB_ENV);
        }
    }

    fn create_locator_schema(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            "#,
        )
        .unwrap();
    }

    fn create_chunk_schema(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE run_materialized_json (
                run_id INTEGER PRIMARY KEY,
                raw_event_json TEXT NOT NULL,
                materialized_at TEXT
            );
            CREATE TABLE chunk_files (
                chunk_id INTEGER PRIMARY KEY,
                chunk_path TEXT NOT NULL
            );
            CREATE TABLE chunk_runs (
                run_id INTEGER PRIMARY KEY,
                chunk_id INTEGER NOT NULL,
                line_number INTEGER NOT NULL
            );
            INSERT INTO runs VALUES (7, 'IRONCLAD', 0, 1, 0, 0, 0, 0, 'CODEX04', '2020-07-30', 0, 1, 0, 0, 0, 0, 'TEN_PERCENT_HP_BONUS', 'NONE');
            INSERT INTO chunk_files VALUES (1, 'chunks/000001.jsonl.zst');
            INSERT INTO chunk_runs VALUES (7, 1, 1);
            "#,
        )
        .unwrap();
    }

    fn write_zstd_lines(path: &Path, lines: &[String]) {
        let mut raw = Vec::new();
        for line in lines {
            writeln!(raw, "{line}").unwrap();
        }
        let compressed = ruzstd::encoding::compress_to_vec(
            raw.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        fs::write(path, compressed).unwrap();
    }

    fn temp_db(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-slaythedata-{name}-{nonce}.sqlite3"))
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-slaythedata-{name}-{nonce}"))
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn upgraded_slaythedata_card_name_matches_base_live_reward_label() {
        assert!(campfire_grid_label_matches_target(
            "barricade",
            "Barricade+1"
        ));
    }

    #[test]
    fn combat_card_selection_is_not_a_slaythedata_run_reward() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": { "screen_type": "CARD_REWARD" },
                "current_state": { "message": { "game_state": {
                    "screen_type": "CARD_REWARD",
                    "combat_state": { "player": {} }
                }}}
            }),
        };

        assert!(!is_card_reward_screen(&state));
    }
}
#[test]
fn legal_neow_leave_is_already_satisfied_on_the_map() {
    let step = SlayTheDataPreflightStep {
        floor: 0,
        ordinal: 2,
        intent: None,
        status: SlayTheDataPreflightStatus::Checked,
        code: "legal_neow_leave".to_owned(),
        message: "Neow leave is legal".to_owned(),
        bridge_command: None,
    };
    let state = LiveState {
        sequence: 1,
        phase: LivePhase::Map,
        legal_actions: Vec::new(),
        raw: json!({}),
    };

    assert!(step_already_satisfied_by_live_state(&step, &state));
}
