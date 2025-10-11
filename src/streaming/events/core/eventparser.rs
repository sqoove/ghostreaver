// ─── import packages ───
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

// ─── imports crates ───
use crate::globals::statics::*;
use crate::streaming::events::common::filter::EventTypeFilter;
use crate::streaming::events::common::{EventMetadata, EventType, ProtocolType};
use crate::streaming::events::core::traits::UnifiedEvent;
use crate::streaming::events::Protocol;
use crate::streaming::events::protocols::block::blockmeta::BlockMetaEvent;
use crate::streaming::events::protocols::bonk::parser::BONK_PROGRAM_ID;
use crate::streaming::events::protocols::pumpfun::parser::PUMPFUN_PROGRAM_ID;
use crate::streaming::events::protocols::pumpswap::parser::PUMPSWAP_PROGRAM_ID;
use crate::streaming::events::protocols::raydiumamm::parser::RAYDIUM_AMM_V4_PROGRAM_ID;
use crate::streaming::events::protocols::raydiumclmm::parser::RAYDIUM_CLMM_PROGRAM_ID;
use crate::streaming::events::protocols::raydiumcpmm::parser::RAYDIUM_CPMM_PROGRAM_ID;
use crate::streaming::grpc::AccountPretty;
use crate::utils::helpers::HelperTools;

// ─── type 'AccountEventParserFn' ───
/// type description
pub type AccountEventParserFn = fn(account: &AccountPretty, metadata: EventMetadata) -> Option<Box<dyn UnifiedEvent>>;

// ─── struct 'AccountEventParseConfig' ───
/// struct description
#[derive(Debug, Clone)]
pub struct AccountEventParseConfig {
    pub program_id: Pubkey,
    pub protocol_type: ProtocolType,
    pub event_type: EventType,
    pub account_discriminator: &'static [u8],
    pub account_parser: AccountEventParserFn
}

// ─── struct 'AccountEventParser' ───
/// struct description
pub struct AccountEventParser;

// ─── impl 'AccountEventParser' ───
/// impl description
impl AccountEventParser {

    // ─── fn 'mapcfg' ───
    /// fn description
    fn mapcfg(program_id: Pubkey, protocol_type: ProtocolType, event_type: EventType, account_discriminator: &'static [u8],
        account_parser: AccountEventParserFn) -> AccountEventParseConfig {

        // ─── return 'mapcfgAccountEventParseConfig' ───
        AccountEventParseConfig {
            program_id,
            protocol_type,
            event_type,
            account_discriminator,
            account_parser,
        }
    }

    // ─── fn 'configs' ───
    /// fn description
    pub fn configs(protocols: Vec<Protocol>, event_type_filter: Option<EventTypeFilter>) -> Vec<AccountEventParseConfig> {

        // ─── define 'protocols_map' ───
        let protocols_map = PROTOCOLCACHECONFIG.get_or_init(|| {

            // ─── define 'map' ───
            let mut map: HashMap<Protocol, Vec<AccountEventParseConfig>> = HashMap::new();

            // ─── map 'Bonk' ───
            map.insert(Protocol::Bonk,
                vec![
                    Self::mapcfg(
                        BONK_PROGRAM_ID,
                        ProtocolType::Bonk,
                        EventType::AccountBonkPoolState,
                        crate::streaming::events::protocols::bonk::discriminators::POOL_STATE_ACCOUNT,
                        crate::streaming::events::protocols::bonk::types::pool_state_parser,
                    ),
                    Self::mapcfg(
                        BONK_PROGRAM_ID,
                        ProtocolType::Bonk,
                        EventType::AccountBonkGlobalConfig,
                        crate::streaming::events::protocols::bonk::discriminators::GLOBAL_CONFIG_ACCOUNT,
                        crate::streaming::events::protocols::bonk::types::global_config_parser,
                    ),
                    Self::mapcfg(
                        BONK_PROGRAM_ID,
                        ProtocolType::Bonk,
                        EventType::AccountBonkPlatformConfig,
                        crate::streaming::events::protocols::bonk::discriminators::PLATFORM_CONFIG_ACCOUNT,
                        crate::streaming::events::protocols::bonk::types::platform_config_parser,
                    ),
                ],
            );

            // ─── map 'PumpFun' ───
            map.insert(Protocol::PumpFun,
                vec![
                    Self::mapcfg(
                        PUMPFUN_PROGRAM_ID,
                        ProtocolType::PumpFun,
                        EventType::AccountPumpFunBondingCurve,
                        crate::streaming::events::protocols::pumpfun::discriminators::BONDING_CURVE_ACCOUNT,
                        crate::streaming::events::protocols::pumpfun::types::bonding_curve_parser,
                    ),
                    Self::mapcfg(
                        PUMPFUN_PROGRAM_ID,
                        ProtocolType::PumpFun,
                        EventType::AccountPumpFunGlobal,
                        crate::streaming::events::protocols::pumpfun::discriminators::GLOBAL_ACCOUNT,
                        crate::streaming::events::protocols::pumpfun::types::global_parser,
                    ),
                ],
            );

            // ─── map 'PumpSwap' ───
            map.insert(Protocol::PumpSwap,
                vec![
                    Self::mapcfg(
                        PUMPSWAP_PROGRAM_ID,
                        ProtocolType::PumpSwap,
                        EventType::AccountPumpSwapGlobalConfig,
                        crate::streaming::events::protocols::pumpswap::discriminators::GLOBAL_CONFIG_ACCOUNT,
                        crate::streaming::events::protocols::pumpswap::types::global_config_parser,
                    ),
                    Self::mapcfg(
                        PUMPSWAP_PROGRAM_ID,
                        ProtocolType::PumpSwap,
                        EventType::AccountPumpSwapPool,
                        crate::streaming::events::protocols::pumpswap::discriminators::POOL_ACCOUNT,
                        crate::streaming::events::protocols::pumpswap::types::pool_parser,
                    ),
                ],
            );

            // ─── map 'RaydiumAmm' ───
            map.insert(Protocol::RaydiumAmmV4,
                vec![Self::mapcfg(
                    RAYDIUM_AMM_V4_PROGRAM_ID,
                    ProtocolType::RaydiumAmmV4,
                    EventType::AccountRaydiumAmmV4AmmInfo,
                    crate::streaming::events::protocols::raydiumamm::discriminators::AMM_INFO,
                    crate::streaming::events::protocols::raydiumamm::types::amm_info_parser,
                )],
            );

            // ─── map 'RaydiumClmm' ───
            map.insert(Protocol::RaydiumClmm,
                vec![
                    Self::mapcfg(
                        RAYDIUM_CLMM_PROGRAM_ID,
                        ProtocolType::RaydiumClmm,
                        EventType::AccountRaydiumClmmAmmConfig,
                        crate::streaming::events::protocols::raydiumclmm::discriminators::AMM_CONFIG,
                        crate::streaming::events::protocols::raydiumclmm::types::amm_config_parser,
                    ),
                    Self::mapcfg(
                        RAYDIUM_CLMM_PROGRAM_ID,
                        ProtocolType::RaydiumClmm,
                        EventType::AccountRaydiumClmmPoolState,
                        crate::streaming::events::protocols::raydiumclmm::discriminators::POOL_STATE,
                        crate::streaming::events::protocols::raydiumclmm::types::pool_state_parser,
                    ),
                    Self::mapcfg(
                        RAYDIUM_CLMM_PROGRAM_ID,
                        ProtocolType::RaydiumClmm,
                        EventType::AccountRaydiumClmmTickArrayState,
                        crate::streaming::events::protocols::raydiumclmm::discriminators::TICK_ARRAY_STATE,
                        crate::streaming::events::protocols::raydiumclmm::types::tick_array_state_parser,
                    ),
                ],
            );

            // ─── map 'RaydiumCpmm' ───
            map.insert(Protocol::RaydiumCpmm,
                vec![
                    Self::mapcfg(
                        RAYDIUM_CPMM_PROGRAM_ID,
                        ProtocolType::RaydiumCpmm,
                        EventType::AccountRaydiumCpmmAmmConfig,
                        crate::streaming::events::protocols::raydiumcpmm::discriminators::AMM_CONFIG,
                        crate::streaming::events::protocols::raydiumcpmm::types::amm_config_parser,
                    ),
                    Self::mapcfg(
                        RAYDIUM_CPMM_PROGRAM_ID,
                        ProtocolType::RaydiumCpmm,
                        EventType::AccountRaydiumCpmmPoolState,
                        crate::streaming::events::protocols::raydiumcpmm::discriminators::POOL_STATE,
                        crate::streaming::events::protocols::raydiumcpmm::types::pool_state_parser,
                    ),
                ],
            );

            // ─── return 'map' ───
            map
        });

        // ─── define 'out' ───
        let mut out = Vec::with_capacity(16);

        // ─── proceed 'for' ───
        for protocol in protocols {

            // ─── compare 'protocols_map' ───
            if let Some(protocol_configs) = protocols_map.get(&protocol) {

                // ─── compare 'event_type_filter' ───
                if let Some(filter) = &event_type_filter {
                    out.extend(protocol_configs.iter().cloned().filter(|mapcfg| filter.include.contains(&mapcfg.event_type)));
                } else {
                    out.extend(protocol_configs.iter().cloned());
                }
            }
        }

        // ─── return 'out' ───
        out
    }

    // ─── fn 'parse_account_event' ───
    /// fn description
    pub fn parse_account_event(protocols: Vec<Protocol>, account: AccountPretty, program_received_time_ms: i64,
        event_type_filter: Option<EventTypeFilter>) -> Option<Box<dyn UnifiedEvent>> {

        // ─── define 'id_strings' ───
        let id_strings = HelperTools::ensureprogramid();

        // ─── define 'configs' ───
        let configs = Self::configs(protocols, event_type_filter);

        // ─── proceed 'for' ───
        for config in configs {

            // ─── compare 'id_strings.get()' ───
            if let Some(&program_str) = id_strings.get(&config.program_id) {

                // ─── compare 'program_str' ───
                if account.owner.as_str() != program_str {

                    // ─── return 'continue' ───
                    continue;
                }
            } else if account.owner != config.program_id.to_string() {

                // ─── return 'continue' ───
                continue;
            }

            // ─── define 'disc_len' ───
            let disc_len = config.account_discriminator.len();

            // ─── compare 'account.data.len()' ───
            if account.data.len() < disc_len {

                // ─── return 'continue' ───
                continue;
            }

            // ─── compare 'config.account_discriminator' ───
            if &account.data[..disc_len] != config.account_discriminator {

                // ─── return 'continue' ───
                continue;
            }

            // ─── define 'event' ───
            let event = (config.account_parser)(&account, EventMetadata {
                    slot: account.slot,
                    signature: account.signature.clone(),
                    protocol: config.protocol_type,
                    event_type: config.event_type,
                    program_id: config.program_id,
                    program_received_time_ms,
                    ..Default::default()
                },
            );

            // ─── compare 'event' ───
            if let Some(mut event) = event {

                // ─── callback 'set_program_handle_time_consuming_ms()' ───
                event.set_program_handle_time_consuming_ms(chrono::Utc::now().timestamp_millis() - program_received_time_ms);

                // ─── return 'Option' ───
                return Some(event);
            }
        }

        // ─── return 'Option' ───
        None
    }
}

// ─── struct 'CommonEventParser' ───
/// struct description
pub struct CommonEventParser;

// ─── impl 'CommonEventParser' ───
/// impl description
impl CommonEventParser {

    // ─── fn 'generate_block_meta_event' ───
    /// fn description
    pub fn generate_block_meta_event(slot: u64, block_hash: &str, block_time_ms: i64) -> Box<dyn UnifiedEvent> {

        // ─── define 'block_meta_event' ───
        let block_meta_event = BlockMetaEvent::new(slot, block_hash.to_string(), block_time_ms);
        Box::new(block_meta_event)
    }
}