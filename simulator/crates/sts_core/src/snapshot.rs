use crate::{SimError, SimResult};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot<T = PlaceholderState> {
    pub schema_version: u32,
    pub state: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceholderState {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotHash(u64);

impl Snapshot<PlaceholderState> {
    #[must_use]
    pub const fn placeholder() -> Self {
        Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: PlaceholderState {},
        }
    }
}

impl<T> Snapshot<T>
where
    T: Serialize,
{
    pub fn canonical_json(&self) -> SimResult<String> {
        serde_json::to_string(self)
            .map_err(|_| SimError::InvalidState("snapshot serialization failed"))
    }

    pub fn hash(&self) -> SimResult<SnapshotHash> {
        Ok(SnapshotHash(stable_hash64(
            self.canonical_json()?.as_bytes(),
        )))
    }
}

impl SnapshotHash {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SnapshotHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

fn stable_hash64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_snapshot_hashes_identically() {
        let first = Snapshot::placeholder();
        let second = Snapshot::placeholder();

        assert_eq!(
            first.hash().expect("first hashes"),
            second.hash().expect("second hashes")
        );
    }

    #[test]
    fn canonical_field_order_does_not_drift() {
        let snapshot = Snapshot::placeholder();

        assert_eq!(
            snapshot.canonical_json().expect("snapshot serializes"),
            r#"{"schema_version":1,"state":{}}"#
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_hash() {
        let snapshot = Snapshot::placeholder();
        let before = snapshot.hash().expect("snapshot hashes");
        let json = snapshot.canonical_json().expect("snapshot serializes");
        let restored: Snapshot = serde_json::from_str(&json).expect("snapshot deserializes");

        assert_eq!(restored, snapshot);
        assert_eq!(restored.hash().expect("restored hashes"), before);
    }
}
