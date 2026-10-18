//! PC ForeignInfluenceAction / CardLibrary.getAnyColorCard(ATTACK, rarity).
//! Complete unlocked attack pools, sorted by native cardID (not translated name).
//! HEALING excludes Feed, Reaper and Lesson Learned. Unsupported effects remain
//! eligible: filtering them would change the native selection distribution.
use crate::{content::shop_pool::shop_card_content_id, rng::StsRng, ContentId};
const COMMON: &[&str] = &[
    "ANGER",
    "BALL_LIGHTNING",
    "BANE",
    "BARRAGE",
    "BEAM_CELL",
    "BODY_SLAM",
    "BOWLING_BASH",
    "CLASH",
    "CLEAVE",
    "CLOTHESLINE",
    "COLD_SNAP",
    "COMPILE_DRIVER",
    "CONSECRATE",
    "CRUSH_JOINTS",
    "CUT_THROUGH_FATE",
    "DAGGER_SPRAY",
    "DAGGER_THROW",
    "EMPTY_FIST",
    "FLURRY_OF_BLOWS",
    "FLYING_KNEE",
    "FLYING_SLEEVES",
    "FOLLOW_UP",
    "CLAW",
    "GO_FOR_THE_EYES",
    "HEADBUTT",
    "HEAVY_BLADE",
    "IRON_WAVE",
    "JUST_LUCKY",
    "PERFECTED_STRIKE",
    "POISONED_STAB",
    "POMMEL_STRIKE",
    "QUICK_SLASH",
    "REBOUND",
    "SASH_WHIP",
    "SLICE",
    "STREAMLINE",
    "SUCKER_PUNCH",
    "SWEEPING_BEAM",
    "SWORD_BOOMERANG",
    "THUNDERCLAP",
    "TWIN_STRIKE",
    "SNEAKY_STRIKE",
    "WILD_STRIKE",
];
const UNCOMMON: &[&str] = &[
    "ALL_OUT_ATTACK",
    "BACKSTAB",
    "BLIZZARD",
    "BLOOD_FOR_BLOOD",
    "CARNAGE",
    "CARVE_REALITY",
    "CHOKE",
    "CONCLUDE",
    "DASH",
    "DOOM_AND_GLOOM",
    "DRAMATIC_ENTRANCE",
    "DROPKICK",
    "ENDLESS_AGONY",
    "EVISCERATE",
    "FTL",
    "FEAR_NO_EVIL",
    "FINISHER",
    "FLASH_OF_STEEL",
    "FLECHETTES",
    "HEEL_HOOK",
    "HEMOKINESIS",
    "BULLSEYE",
    "MASTERFUL_STAB",
    "MELTER",
    "MIND_BLAST",
    "PREDATOR",
    "PUMMEL",
    "RAMPAGE",
    "REACH_HEAVEN",
    "RECKLESS_CHARGE",
    "RIDDLE_WITH_HOLES",
    "RIP_AND_TEAR",
    "SANDS_OF_TIME",
    "SCRAPE",
    "SEARING_BLOW",
    "SEVER_SOUL",
    "SIGNATURE_MOVE",
    "SKEWER",
    "SUNDER",
    "SWIFT_STRIKE",
    "TALK_TO_THE_HAND",
    "TANTRUM",
    "UPPERCUT",
    "WALLOP",
    "WEAVE",
    "WHEEL_KICK",
    "WHIRLWIND",
    "WINDMILL_STRIKE",
];
const RARE: &[&str] = &[
    "ALL_FOR_ONE",
    "BLUDGEON",
    "BRILLIANCE",
    "CORE_SURGE",
    "DIE_DIE_DIE",
    "FIEND_FIRE",
    "GLASS_KNIFE",
    "GRAND_FINALE",
    "HAND_OF_GREED",
    "HYPERBEAM",
    "IMMOLATE",
    "METEOR_STRIKE",
    "RAGNAROK",
    "THUNDER_STRIKE",
    "UNLOAD",
];
pub(crate) fn choices(card_random: &mut StsRng, card: &mut StsRng) -> Vec<ContentId> {
    let mut result = Vec::with_capacity(3);
    while result.len() < 3 {
        let roll = card_random.random_int(99);
        let pool = if roll < 55 {
            COMMON
        } else if roll < 85 {
            UNCOMMON
        } else {
            RARE
        };
        // CardLibrary shuffles with cardRandomRng before getRandomCard sorts
        // the candidates again. The permutation is dead; its RNG call is not.
        card_random.random_long();
        let id = shop_card_content_id(pool[card.random_int((pool.len() - 1) as i32) as usize]);
        if !result.contains(&id) {
            result.push(id);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_native_pools_include_delegated_constructor_and_exclude_healing() {
        assert_eq!((COMMON.len(), UNCOMMON.len(), RARE.len()), (43, 48, 15));
        let all: std::collections::BTreeSet<_> =
            COMMON.iter().chain(UNCOMMON).chain(RARE).copied().collect();
        assert_eq!(all.len(), 106);
        for key in ["FEED", "REAPER", "LESSON_LEARNED"] {
            assert!(!all.contains(key));
        }
        for key in [
            "ANGER",
            "CLAW",
            "SKEWER",
            "SEARING_BLOW",
            "CONCLUDE",
            "HAND_OF_GREED",
        ] {
            assert!(all.contains(key));
        }
        let index = UNCOMMON.iter().position(|k| *k == "SEARING_BLOW").unwrap();
        assert_eq!(
            &UNCOMMON[index - 1..=index + 1],
            &["SCRAPE", "SEARING_BLOW", "SEVER_SOUL"]
        );
        for key in all {
            assert_eq!(
                crate::run::reward::any_color_reward_card_key(shop_card_content_id(key)),
                Some(key)
            );
        }
    }
    #[test]
    fn each_attempt_owns_two_card_random_draws_and_one_card_draw_including_rejections() {
        let mut rejected = false;
        for seed in 0..128 {
            let mut random = StsRng::new(seed);
            let mut card = StsRng::new(seed + 200);
            let a = choices(&mut random, &mut card);
            assert_eq!(a.len(), 3);
            assert_eq!(a.iter().collect::<std::collections::BTreeSet<_>>().len(), 3);
            assert_eq!(random.counter(), 2 * card.counter());
            assert!(card.counter() >= 3);
            rejected |= card.counter() > 3;
        }
        assert!(
            rejected,
            "controls must exercise duplicate rejection, not just three picks"
        );
    }
}
