// ─── import packages ───
use anyhow::{anyhow, Result};
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, sync::{Arc, LazyLock}};

// ─── import crates ───
use crate::streaming::events::core::traits::EventParser;
use crate::streaming::events::protocols::{
    bonk::parser::BONK_PROGRAM_ID,
    pumpfun::parser::PUMPFUN_PROGRAM_ID,
    pumpswap::parser::PUMPSWAP_PROGRAM_ID,
    raydiumamm::parser::RAYDIUM_AMM_V4_PROGRAM_ID,
    raydiumclmm::parser::RAYDIUM_CLMM_PROGRAM_ID,
    raydiumcpmm::parser::RAYDIUM_CPMM_PROGRAM_ID,
    BonkEventParser,
    PumpFunEventParser,
    PumpSwapEventParser,
    RaydiumAmmV4EventParser,
    RaydiumClmmEventParser,
    RaydiumCpmmEventParser
};

// ─── static 'EVENT_PARSERS' ───
/// impl description
static EVENT_PARSERS: LazyLock<HashMap<Protocol, Arc<dyn EventParser>>> = LazyLock::new(|| {

    // ─── let 'parsers' ───
    let mut parsers: HashMap<Protocol, Arc<dyn EventParser>> = HashMap::with_capacity(6);
    parsers.insert(Protocol::PumpSwap, Arc::new(PumpSwapEventParser::new()));
    parsers.insert(Protocol::PumpFun, Arc::new(PumpFunEventParser::new()));
    parsers.insert(Protocol::Bonk, Arc::new(BonkEventParser::new()));
    parsers.insert(Protocol::RaydiumCpmm, Arc::new(RaydiumCpmmEventParser::new()));
    parsers.insert(Protocol::RaydiumClmm, Arc::new(RaydiumClmmEventParser::new()));
    parsers.insert(Protocol::RaydiumAmmV4, Arc::new(RaydiumAmmV4EventParser::new()));
    parsers
});

// ─── enum 'Protocol' ───
/// enum description
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Protocol {
    PumpSwap,
    PumpFun,
    Bonk,
    RaydiumCpmm,
    RaydiumClmm,
    RaydiumAmmV4,
}

// ─── impl 'Protocol' ───
/// impl description
impl Protocol {

    // ─── fn 'get_program_id' ───
    /// fn description
    pub fn get_program_id(&self) -> Vec<Pubkey> {

        // ─── match 'self' ───
        match self {
            Protocol::PumpSwap => vec![PUMPSWAP_PROGRAM_ID],
            Protocol::PumpFun => vec![PUMPFUN_PROGRAM_ID],
            Protocol::Bonk => vec![BONK_PROGRAM_ID],
            Protocol::RaydiumCpmm => vec![RAYDIUM_CPMM_PROGRAM_ID],
            Protocol::RaydiumClmm => vec![RAYDIUM_CLMM_PROGRAM_ID],
            Protocol::RaydiumAmmV4 => vec![RAYDIUM_AMM_V4_PROGRAM_ID],
        }
    }
}

// ─── impl 'Display for Protocol' ───
/// impl description
impl std::fmt::Display for Protocol {

    // ─── fn 'fmt' ───
    /// fn description
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        // ─── match 'self' ───
        match self {
            Protocol::PumpSwap => write!(f, "PumpSwap"),
            Protocol::PumpFun => write!(f, "PumpFun"),
            Protocol::Bonk => write!(f, "Bonk"),
            Protocol::RaydiumCpmm => write!(f, "RaydiumCpmm"),
            Protocol::RaydiumClmm => write!(f, "RaydiumClmm"),
            Protocol::RaydiumAmmV4 => write!(f, "RaydiumAmmV4"),
        }
    }
}

// ─── impl 'FromStr for Protocol' ───
/// impl description
impl std::str::FromStr for Protocol {

    // ─── type 'Err' ───
    type Err = anyhow::Error;

    // ─── fn 'from_str' ───
    /// fn description
    fn from_str(s: &str) -> Result<Self, Self::Err> {

        // ─── match 's.to_lowercase()' ───
        match s.to_lowercase().as_str() {
            "pumpswap" => Ok(Protocol::PumpSwap),
            "pumpfun" => Ok(Protocol::PumpFun),
            "bonk" => Ok(Protocol::Bonk),
            "raydiumcpmm" => Ok(Protocol::RaydiumCpmm),
            "raydiumclmm" => Ok(Protocol::RaydiumClmm),
            "raydiumammv4" => Ok(Protocol::RaydiumAmmV4),
            _ => Err(anyhow!("Unsupported protocol: {}", s)),
        }
    }
}

// ─── struct 'EventParserFactory' ───
/// struct description
pub struct EventParserFactory;

// ─── impl 'EventParserFactory' ───
/// impl description
impl EventParserFactory {

    // ─── fn 'create_parser' ───
    /// fn description
    pub fn create_parser(protocol: Protocol) -> Arc<dyn EventParser> {

        // ─── return 'EVENT_PARSERS' ───
        EVENT_PARSERS.get(&protocol).cloned().unwrap_or_else(|| {
            panic!("Parser for protocol {protocol} not found");
        })
    }

    // ─── fn 'create_all_parsers' ───
    /// fn description
    pub fn create_all_parsers() -> Vec<Arc<dyn EventParser>> {

        // ─── return 'Self' ───
        Self::supported_protocols()
            .into_iter()
            .map(Self::create_parser)
            .collect()
    }

    // ─── fn 'supported_protocols' ───
    /// fn description
    pub fn supported_protocols() -> Vec<Protocol> {

        // ─── return 'Vec' ───
        vec![Protocol::PumpSwap]
    }

    // ─── fn 'is_supported' ───
    /// fn description
    pub fn is_supported(protocol: &Protocol) -> bool {

        // ─── return 'bool' ───
        Self::supported_protocols().contains(protocol)
    }
}