#![forbid(unsafe_code)]
#![doc = "Core library for the Slay the Spire simulator."]

pub mod error;
pub mod ids;
pub mod snapshot;

pub use error::{SimError, SimResult};
pub use ids::{ActionId, CardId, ContentId, MonsterId};
pub use snapshot::{PlaceholderState, Snapshot, SnapshotHash, SNAPSHOT_SCHEMA_VERSION};
