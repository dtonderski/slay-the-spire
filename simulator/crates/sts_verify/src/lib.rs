#![forbid(unsafe_code)]

mod diff;
mod sim_real;
mod trace;

pub use sim_real::{
    verify_communication_mod_trace_reader, SeedStartBoundary, SimRealError, SimRealReport,
};
