// ─── import crates ───
use crate::streaming::events::common::{types::EventType, ACCOUNT_EVENT_TYPES, BLOCK_EVENT_TYPES};

// ─── struct 'EventTypeFilter' ───
/// struct description
#[derive(Debug, Clone)]
pub struct EventTypeFilter {
    pub include: Vec<EventType>
}

// ─── impl 'EventTypeFilter' ───
/// impl description
impl EventTypeFilter {

    // ─── fn 'include_transaction_event' ───
    /// fn description
    pub fn include_transaction_event(&self) -> bool {

        // ─── return 'self.include.iter()' ───
        self.include.iter().any(|event| !ACCOUNT_EVENT_TYPES.contains(event) && !BLOCK_EVENT_TYPES.contains(event))
    }

    // ─── fn 'include_account_event' ───
    /// fn description
    pub fn include_account_event(&self) -> bool {

        // ─── return 'self.include.iter()' ───
        self.include.iter().any(|event| ACCOUNT_EVENT_TYPES.contains(event))
    }

    // ─── fn 'include_block_event' ───
    /// fn description
    pub fn include_block_event(&self) -> bool {

        // ─── return 'self.include.iter()' ───
        self.include.iter().any(|event| BLOCK_EVENT_TYPES.contains(event))
    }
}