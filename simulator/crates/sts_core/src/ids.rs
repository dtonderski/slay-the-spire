use serde::{Deserialize, Serialize};
use std::fmt;

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
}
