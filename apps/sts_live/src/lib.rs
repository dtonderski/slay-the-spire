#![forbid(unsafe_code)]
#![doc = "Live trace collection backend primitives and operator surfaces."]

pub mod agent;
pub mod automation;
pub mod bridge;
pub mod cli;
pub mod cli_output;
pub mod combat_research;
pub mod communication;
pub mod error_payload;
pub mod fidelity;
mod fidelity_status;
pub mod http;
pub mod model;
mod operator_actions;
pub mod replay;
pub mod session;
mod session_blocking;
mod session_recovery;
mod session_response;
mod session_state;
pub mod slaythedata;
pub mod trace_writer;

#[cfg(test)]
mod automation_tests;
#[cfg(test)]
mod communication_tests;
#[cfg(test)]
mod fidelity_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod ui_contract_tests;

pub use bridge::{BridgeManager, FakeBridgeManager};
pub use communication::{CommunicationBridgeConfig, CommunicationModBridgeManager};
pub use fidelity::{FidelityChecker, TraceFidelityChecker};
pub use model::*;
pub use session::SessionStore;
pub use slaythedata::{SlayTheDataIndex, DEFAULT_SLAYTHEDATA_DB, SLAYTHEDATA_DB_ENV};
pub use trace_writer::{TraceRecovery, TraceWriter};
