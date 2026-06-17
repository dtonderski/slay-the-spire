use crate::ids::{ActionId, CardId, ContentId, MonsterId};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    InvalidAction(ActionId),
    IllegalAction(&'static str),
    UnknownCard(CardId),
    UnknownMonster(MonsterId),
    UnknownContent(ContentId),
    InvalidState(&'static str),
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAction(id) => write!(f, "invalid action: {id}"),
            Self::IllegalAction(message) => write!(f, "illegal action: {message}"),
            Self::UnknownCard(id) => write!(f, "unknown card: {id}"),
            Self::UnknownMonster(id) => write!(f, "unknown monster: {id}"),
            Self::UnknownContent(id) => write!(f, "unknown content: {id}"),
            Self::InvalidState(message) => write!(f, "invalid state: {message}"),
        }
    }
}

impl std::error::Error for SimError {}

pub type SimResult<T> = Result<T, SimError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_printable_and_comparable() {
        let first = SimError::UnknownCard(CardId::new(7));
        let second = SimError::UnknownCard(CardId::new(7));

        assert_eq!(first, second);
        assert_eq!(first.to_string(), "unknown card: card:7");
    }
}
