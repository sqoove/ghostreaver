// ─── mod 'types' ───
/// mod description
pub mod types;

// ─── mod 'filter' ───
/// mod description
pub mod filter;

// ─── use 'types' ───
/// use description
pub use types::*;

// ─── macro_rules 'impl_unified_event' ───
/// macro_rules description
#[macro_export]
macro_rules! impl_unified_event {
    ($struct_name:ident, $($field:ident),*) => {
        
        // ─── impl 'UnifiedEvent' ───
        /// impl description
        impl $crate::streaming::events::core::traits::UnifiedEvent for $struct_name {
            
            // ─── fn 'id' ───
            /// fn description
            fn id(&self) -> &str {
                
                // ─── return 'str' ───
                &self.metadata.id
            }

            // ─── fn 'event_type' ───
            /// fn description
            fn event_type(&self) -> $crate::streaming::events::common::types::EventType {
                
                // ─── return 'crate' ───
                self.metadata.event_type.clone()
            }

            // ─── fn 'signature' ───
            /// fn description
            fn signature(&self) -> &str {
                
                // ─── return 'str' ───
                &self.metadata.signature
            }

            // ─── fn 'slot' ───
            /// fn description
            fn slot(&self) -> u64 {
                
                // ─── return 'u64' ───
                self.metadata.slot
            }

            // ─── fn 'program_received_time_ms' ───
            /// fn description
            fn program_received_time_ms(&self) -> i64 {
                
                // ─── return 'i64' ───
                self.metadata.program_received_time_ms
            }

            // ─── fn 'program_handle_time_consuming_ms' ───
            /// fn description
            fn program_handle_time_consuming_ms(&self) -> i64 {
                
                // ─── return 'i64' ───
                self.metadata.program_handle_time_consuming_ms
            }

            // ─── fn 'set_program_handle_time_consuming_ms' ───
            /// fn description
            fn set_program_handle_time_consuming_ms(&mut self, program_handle_time_consuming_ms: i64) {
                
                // ─── return 'i64' ───
                self.metadata.program_handle_time_consuming_ms = program_handle_time_consuming_ms;
            }

            // ─── fn 'as_any' ───
            /// fn description
            fn as_any(&self) -> &dyn std::any::Any {
                
                // ─── return 'dyn' ───
                self
            }

            // ─── fn 'as_any_mut' ───
            /// fn description
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                
                // ─── return 'dyn' ───
                self
            }

            // ─── fn 'clone_boxed' ───
            /// fn description
            fn clone_boxed(&self) -> Box<dyn $crate::streaming::events::core::traits::UnifiedEvent> {
                
                // ─── return 'Box' ───
                Box::new(self.clone())
            }

            // ─── fn 'merge' ───
            /// fn description
            fn merge(&mut self, other: Box<dyn $crate::streaming::events::core::traits::UnifiedEvent>) {
                
                // ─── compare 'other.as_any()' ───
                if let Some(_e) = other.as_any().downcast_ref::<$struct_name>() {
                    
                    // ─── return 'Box' ───
                    $(self.$field = _e.$field.clone();)*
                }
            }

            // ─── fn 'set_transfer_datas' ───
            /// fn description
            fn set_transfer_datas(&mut self, transfer_datas: Vec<$crate::streaming::events::common::types::TransferData>, swap_data: Option<$crate::streaming::events::common::types::SwapData>) {
                
                // ─── return 'String' ───
                self.metadata.set_transfer_datas(transfer_datas, swap_data);
            }

            // ─── fn 'index' ───
            /// fn description
            fn index(&self) -> String {

                // ─── return 'String' ───
                self.metadata.index.clone()
            }
        }
    };
}