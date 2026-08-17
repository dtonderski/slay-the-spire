use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{SimError, SimResult};

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $prefix, self.0)
            }
        }
    };
}

id_type!(CardId, "card");
id_type!(MonsterId, "monster");
id_type!(ActionId, "action");
id_type!(ContentId, "content");
id_type!(MapNodeId, "map_node");

/// Card-instance IDs share the positive signed-long domain used by external
/// state and reserve the upper unsigned range for allocation headroom.
pub(crate) const MAX_SUPPORTED_CARD_INSTANCE_ID: u64 = i64::MAX as u64;

/// Offset used to remint a Headbutt skipped-retrieval alias so two listings of
/// the same Java `AbstractCard` can coexist under unique instance IDs.
///
/// Generated combat-only cards (Wounds, Dazed) can also occupy this high band.
/// Pairing therefore requires matching `content_id`, not the offset alone.
pub const HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET: u64 = 4_000_000_000_000_000_000;

/// Returns the Headbutt alias pairing for `id`, if the offset arithmetic fits.
#[must_use]
pub fn headbutt_alias_sibling_id(id: CardId) -> Option<CardId> {
    let raw = id.get();
    if raw >= HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET {
        Some(CardId::new(
            raw - HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET,
        ))
    } else {
        raw.checked_add(HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET)
            .map(CardId::new)
    }
}

#[must_use]
pub(crate) const fn card_instance_id_is_supported(id: CardId) -> bool {
    id.get() > 0 && id.get() <= MAX_SUPPORTED_CARD_INSTANCE_ID
}

/// Returns the first ID in a contiguous card-instance allocation after
/// `max_id`, after proving that the complete allocation fits the externally
/// representable positive signed-long domain.
pub(crate) fn reserve_card_instance_id_range(max_id: u64, count: usize) -> SimResult<u64> {
    if count == 0 {
        return Err(SimError::InvalidState(
            "card instance allocation reserved no IDs",
        ));
    }
    if max_id > MAX_SUPPORTED_CARD_INSTANCE_ID {
        return Err(SimError::InvalidState(
            "existing card instance ID exceeds the supported domain",
        ));
    }
    let count = u64::try_from(count).map_err(|_| {
        SimError::InvalidState("card instance allocation count exceeds the supported domain")
    })?;
    let last_id = max_id.checked_add(count).ok_or(SimError::InvalidState(
        "card instance ID allocation overflows u64",
    ))?;
    if last_id > MAX_SUPPORTED_CARD_INSTANCE_ID {
        return Err(SimError::InvalidState(
            "card instance ID allocation exceeds the supported domain",
        ));
    }
    max_id.checked_add(1).ok_or(SimError::InvalidState(
        "card instance ID allocation overflows u64",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_json() {
        let card = CardId::new(42);

        let serialized = serde_json::to_string(&card).expect("card id serializes");
        let deserialized: CardId = serde_json::from_str(&serialized).expect("card id deserializes");

        assert_eq!(deserialized, card);
        assert_eq!(deserialized.get(), 42);
    }

    #[test]
    fn ids_are_distinct_and_printable() {
        assert_eq!(CardId::new(1).to_string(), "card:1");
        assert_eq!(MonsterId::new(2).to_string(), "monster:2");
        assert_eq!(ActionId::new(3).to_string(), "action:3");
        assert_eq!(ContentId::new(4).to_string(), "content:4");
    }

    #[test]
    fn card_instance_ids_use_the_positive_signed_long_domain() {
        assert!(!card_instance_id_is_supported(CardId::new(0)));
        assert!(card_instance_id_is_supported(CardId::new(i64::MAX as u64)));
        assert!(!card_instance_id_is_supported(CardId::new(
            i64::MAX as u64 + 1
        )));
    }

    #[test]
    fn headbutt_alias_sibling_pairs_across_the_remint_offset() {
        let original = CardId::new(5);
        let alias = CardId::new(5 + HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET);
        assert_eq!(headbutt_alias_sibling_id(original), Some(alias));
        assert_eq!(headbutt_alias_sibling_id(alias), Some(original));
    }

    #[test]
    fn card_instance_range_reservation_checks_the_complete_range() {
        assert_eq!(reserve_card_instance_id_range(0, 2), Ok(1));
        assert_eq!(
            reserve_card_instance_id_range(MAX_SUPPORTED_CARD_INSTANCE_ID - 1, 2),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
        assert_eq!(
            reserve_card_instance_id_range(MAX_SUPPORTED_CARD_INSTANCE_ID, 1),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
    }
}
