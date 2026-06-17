use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatorRng {
    state: u64,
    log: Vec<RngDraw>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RngDraw {
    pub stream: RngStream,
    pub call_site: String,
    pub bound: usize,
    pub value: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RngStream {
    Shuffle,
}

impl SimulatorRng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed,
            log: Vec::new(),
        }
    }

    #[must_use]
    pub fn log(&self) -> &[RngDraw] {
        &self.log
    }

    pub fn next_usize(
        &mut self,
        stream: RngStream,
        call_site: &'static str,
        bound: usize,
    ) -> usize {
        assert!(bound > 0, "rng bound must be greater than zero");
        let value = (self.next_u64() as usize) % bound;
        self.log.push(RngDraw {
            stream,
            call_site: call_site.to_owned(),
            bound,
            value,
        });
        value
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_rng_is_deterministic_and_logged() {
        let mut first = SimulatorRng::new(7);
        let mut second = SimulatorRng::new(7);

        assert_eq!(
            first.next_usize(RngStream::Shuffle, "test", 10),
            second.next_usize(RngStream::Shuffle, "test", 10)
        );
        assert_eq!(first.log().len(), 1);
    }
}
