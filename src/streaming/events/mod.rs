// ─── mod 'common' ───
/// mod description
pub mod common;

// ─── mod 'core' ───
/// mod description
pub mod core;

// ─── mod 'factory' ───
/// mod description
pub mod factory;

// ─── mod 'protocols' ───
/// mod description
pub mod protocols;

// ─── use 'core::traits' ───
/// use description
pub use core::traits::{EventParser, UnifiedEvent};

// ─── use 'factory' ───
/// use description
pub use factory::{EventParserFactory, Protocol};

// ─── macro_rules 'eventsmatch' ───
/// macro_rules description
#[macro_export]
macro_rules! eventsmatch {
    ($event:expr, {$($event_type:ty => $handler:expr),* $(,)?}) => {
        $(if let Some(typed_event) = $event.as_any().downcast_ref::<$event_type>() {$handler(typed_event.clone());} else)*
        {
            // No action defined
        }
    };
}

// ─── use 'eventsmatch' ───
/// use description
pub use eventsmatch;