//! CommunicationMod trace import helpers.

use crate::{
    normalize::{normalize_communication_mod_message, CanonicalRunObservation},
    trace::{import_communication_mod_trace, TraceLine},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedTraceStep {
    pub step: u32,
    pub observation: CanonicalRunObservation,
}

/// Extract normalized observations from each state line in a trace.
pub fn observations_from_trace(content: &str) -> Result<Vec<ImportedTraceStep>, serde_json::Error> {
    let trace = import_communication_mod_trace(content)?;
    let mut observations = Vec::new();
    for line in trace.lines {
        if let TraceLine::State(state) = line {
            if let Some(observation) = normalize_communication_mod_message(&state.message) {
                observations.push(ImportedTraceStep {
                    step: state.step,
                    observation,
                });
            }
        }
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_corpus_file;

    #[test]
    fn communication_mod_corpus_parses_states_if_present() {
        let Some(content) =
            load_corpus_file("communication_mod/trace-2026-06-18T00-53-06-235Z.jsonl")
        else {
            return;
        };

        let observations = observations_from_trace(&content).expect("import trace");
        assert!(!observations.is_empty());
        assert!(observations.iter().any(|step| step.observation.in_combat));
    }
}
