// ─── import packages ───
use std::collections::HashMap;
use solana_program::pubkey::Pubkey;

// ─── import crates ───
use crate::globals::statics::*;
use crate::streaming::events::protocols::bonk::parser::BONK_PROGRAM_ID;
use crate::streaming::events::protocols::pumpfun::parser::PUMPFUN_PROGRAM_ID;
use crate::streaming::events::protocols::pumpswap::parser::PUMPSWAP_PROGRAM_ID;
use crate::streaming::events::protocols::raydiumamm::parser::RAYDIUM_AMM_V4_PROGRAM_ID;
use crate::streaming::events::protocols::raydiumclmm::parser::RAYDIUM_CLMM_PROGRAM_ID;
use crate::streaming::events::protocols::raydiumcpmm::parser::RAYDIUM_CPMM_PROGRAM_ID;

// ─── struct 'HelperTools' ───
/// struct description
pub struct HelperTools;

// ─── impl 'HelperTools' ───
/// impl description
impl HelperTools {

    // ─── fn 'leakstring' ───
    /// fn decription
    fn leakstring(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }
    
    // ─── fn 'ensureprogramid' ───
    /// fn decription
    pub fn ensureprogramid() -> &'static HashMap<Pubkey, &'static str> {
        PROGRAMIDSTRINGS.get_or_init(|| {

            // ─── define 'm' ───
            let mut m = HashMap::new();
            m.insert(PUMPSWAP_PROGRAM_ID, HelperTools::leakstring(PUMPSWAP_PROGRAM_ID.to_string()));
            m.insert(PUMPFUN_PROGRAM_ID, HelperTools::leakstring(PUMPFUN_PROGRAM_ID.to_string()));
            m.insert(BONK_PROGRAM_ID, HelperTools::leakstring(BONK_PROGRAM_ID.to_string()));
            m.insert(RAYDIUM_CPMM_PROGRAM_ID, HelperTools::leakstring(RAYDIUM_CPMM_PROGRAM_ID.to_string()));
            m.insert(RAYDIUM_CLMM_PROGRAM_ID, HelperTools::leakstring(RAYDIUM_CLMM_PROGRAM_ID.to_string()));
            m.insert(RAYDIUM_AMM_V4_PROGRAM_ID, HelperTools::leakstring(RAYDIUM_AMM_V4_PROGRAM_ID.to_string()));

            // ─── return 'm' ───
            m
        })
    }
}