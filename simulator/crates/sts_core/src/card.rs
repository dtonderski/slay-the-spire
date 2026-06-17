use crate::ids::{CardId, ContentId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardInstance {
    pub id: CardId,
    pub content_id: ContentId,
}

impl CardInstance {
    #[must_use]
    pub const fn new(id: CardId, content_id: ContentId) -> Self {
        Self { id, content_id }
    }
}
