// ─── imports packages ───
use anyhow::{Result, anyhow};
use env_logger::{Builder, Env};
use log::{error, info};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// ─── import crates ───
use ghostreaver::core::client::RPCClient;
use ghostreaver::globals::constants::*;
use ghostreaver::{
    eventsmatch,
    streaming::{
        events::{
            common::{filter::EventTypeFilter, EventType},
            protocols::{
                bonk::{parser::BONK_PROGRAM_ID,
                    BonkPoolCreateEvent,
                    BonkTradeEvent
                },
                pumpfun::{parser::PUMPFUN_PROGRAM_ID,
                    PumpFunCreateTokenEvent,
                    PumpFunTradeEvent
                },
                pumpswap::{parser::PUMPSWAP_PROGRAM_ID,
                    PumpSwapBuyEvent,
                    PumpSwapCreatePoolEvent,
                    PumpSwapSellEvent,
                    PumpSwapWithdrawEvent
                },
                raydiumamm::{parser::RAYDIUM_AMM_V4_PROGRAM_ID,
                    RaydiumAmmV4AmmInfoAccountEvent,
                    RaydiumAmmV4Initialize2Event
                },
                raydiumclmm::{
                    parser::RAYDIUM_CLMM_PROGRAM_ID,
                    RaydiumClmmCreatePoolEvent,
                    RaydiumClmmPoolStateAccountEvent
                },
                raydiumcpmm::{
                    parser::RAYDIUM_CPMM_PROGRAM_ID,
                    RaydiumCpmmInitializeEvent,
                    RaydiumCpmmSwapEvent
                },
            },
            Protocol, UnifiedEvent,
        },
        grpc::ClientConfig,
        yellowstone::{AccountFilter, TransactionFilter},
        YellowstoneGrpc
    },
};
use ghostreaver::utils::loader::{ServerConfig, TradeConfig, WalletConfig};
use ghostreaver::utils::storage::Storage;
use ghostreaver::trading::monitor::TradeMonitor;

// ─── struct 'GhostReaver' ───
/// struct description
struct GhostReaver {
    grpc: YellowstoneGrpc,
    storage: Arc<Storage>
}

// ─── impl 'GhostReaver' ───
/// impl description
impl GhostReaver {

    // ─── fn 'new' ───
    /// fn description
    async fn new(endpoint: &str) -> Result<Self> {

        // ─── define 'confserv' ───
        let confserv = ServerConfig::loadconfig(endpoint).map_err(|e| anyhow!("loading server config: {e}"))?;

        // ─── define 'confbot' ───
        let confbot = TradeConfig::loadconfig(PATHCONFIGBOT).map_err(|e| anyhow!("loading bot config: {e}"))?;

        // ─── define 'bot' ───
        let bot = Arc::new(confbot);

        // ─── define 'wallet' ───
        let wallet = Arc::new(WalletConfig::loadconfig(PATHCONFIGWALLET).map_err(|e| anyhow!("loading wallet config: {e}"))?);
        info!("[Wallet] Public key: {}", wallet.wallet.publicaddr);

        // ─── define 'rpcclient' ───
        let rpcclient = RPCClient::new(&confserv.endpoint.rpc).map_err(|e| anyhow!("creating RPC client: {e}"))?;

        // ─── compare 'rpcclient' ───
        if !rpcclient.getstatus().await {
            error!("RPC endpoint unreachable —> Startup aborted");
            std::process::exit(0);
        }

        // ─── compare 'rpcclient.gethealth()' ───
        if let Err(e) = rpcclient.gethealth().await {
            return Err(anyhow!("RPC health check failed: {e}"));
        }

        // ─── define 'rpc' ───
        let rpc = Arc::new(rpcclient);

        // ─── define 'confgrpc' ───
        let mut confgrpc = ClientConfig::high_performance();
        confgrpc.enable_metrics = false;

        // ─── define 'grpc' ───
        let grpc = YellowstoneGrpc::new_with_config(confserv.endpoint.geyser.clone(), 
            Some(confserv.endpoint.xtoken.clone()), confgrpc).map_err(|e| anyhow!("creating Yellowstone gRPC client: {e}"))?;

        // ─── define 'storage' ───
        let storage = Arc::new(Storage::new().await.map_err(|e| anyhow!("creating Storage: {e}"))?);

        // ─── define 'initbalance' ───
        let initbalance: f64 = if bot.bot.main.sandbox {
            info!("[Balance] Using sandbox balance: {}", bot.bot.main.balance);
            bot.bot.main.balance
        } else {

            // ─── match 'rpc.getwalletbalance()' ───
            match rpc.getwalletbalance(&wallet.wallet.publicaddr).await {
                Ok(lamports) => lamports as f64 / 1_000_000_000.0,
                Err(e) => {
                    error!("Failed to fetch wallet balance: {e}");
                    0.0
                }
            }
        };

        // ─── compare 'storage.walletupdate()' ───
        if let Err(e) = storage.walletupdate(initbalance).await {
            error!("Failed to update wallet: {e}");
        }

        // ─── callback 'storage.walletupdate()' ───
        GhostReaver::spawnexit(Arc::clone(&storage), bot.bot.main.maxtrades as u64);

        // ─── return 'Result' ───
        Ok(GhostReaver { grpc, storage })
    }

    // ─── fn 'spawnexit' ───
    /// fn description
    fn spawnexit(storage: Arc<Storage>, maxtrades: u64) {

        // ─── compare 'maxtrades' ───
        if maxtrades == 0 {
            return;
        }

        // ─── proceed 'tokio' ───
        tokio::spawn(async move {

            // ─── proceed 'loop' ───
            loop {

                // ─── define 'open' ───
                let open = storage.tradescount().await.unwrap_or(1);

                // ─── define 'done' ───
                let done = storage.tradestotal().await.unwrap_or(0);

                // ─── compare 'open' ───
                if open == 0 && done >= maxtrades as i64 {
                    info!("[Shutdown] All {} trades completed (maxtrades={}) — exporting CSVs…", done, maxtrades);

                    // ─── match 'storage.exporttables()' ───
                    match storage.exporttables().await {
                        Ok(_) => info!("[Export] CSVs written under ./database"),
                        Err(e) => error!("[Export] Failed to export tables to CSV: {}", e),
                    }

                    info!("[Shutdown] Exiting now.");
                    std::process::exit(0);
                }

                // ─── callback 'sleep()' ───
                sleep(Duration::from_secs(2)).await;
            }
        });
    }

    // ─── fn 'removedatabase' ───
    /// fn description
    async fn removedatabase() -> Result<()> {

        // ─── define 'storage' ───
        let storage = Storage::new().await.map_err(|e| anyhow!("creating Storage for drop: {e}"))?;

        // ─── match 'storage.droptables()' ───
        match storage.droptables().await {
            Ok(_) => {
                info!("[startup] all database tables dropped");
                Ok(())
            }
            Err(e) => Err(anyhow!("[startup] failed dropping tables: {e}")),
        }
    }

    // ─── fn 'eventcallback' ───
    /// fn description─
    fn eventcallback(storage: Arc<Storage>) -> impl Fn(Box<dyn UnifiedEvent>) + Send + Sync + 'static {
        move |event: Box<dyn UnifiedEvent>| {

            // ─── define 'storage' ───
            let storage = Arc::clone(&storage);
            eventsmatch!(event, {
                BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokeninsertbonk()' ───
                        if let Err(err) = Storage::tokeninsertbonk(&dbstore, &e).await {
                            error!("storage write failed: {err}");
                        }
                    });
                },
                PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokeninsertpumpfun()' ───
                        if let Err(err) = Storage::tokeninsertpumpfun(&dbstore, &e).await {
                            error!("storage write failed: {err}");
                        }
                    });
                },
                PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokeninsertpumpswap()' ───
                        if let Err(err) = Storage::tokeninsertpumpswap(&dbstore, &e).await {
                            error!("storage write failed: {err}");
                        }
                    });
                },
                RaydiumAmmV4Initialize2Event => |e: RaydiumAmmV4Initialize2Event| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokeninsertraydiumamm()' ───
                        if let Err(err) = Storage::tokeninsertraydiumamm(&dbstore, &e).await {
                            error!("storage write failed: {err}");
                        }
                    });
                },
                RaydiumClmmCreatePoolEvent => |e: RaydiumClmmCreatePoolEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokeninsertraydiumclmm()' ───
                        if let Err(err) = Storage::tokeninsertraydiumclmm(&dbstore, &e).await {
                            error!("storage write failed: {err}");
                        }
                    });
                },
                RaydiumCpmmInitializeEvent => |e: RaydiumCpmmInitializeEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokeninsertraydiumcpmm()' ───
                        if let Err(err) = Storage::tokeninsertraydiumcpmm(&dbstore, &e).await {
                            error!("storage write failed: {err}");
                        }
                    });
                },
                BonkTradeEvent => |e: BonkTradeEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdatebonk()' ───
                        if let Err(err) = Storage::tokenupdatebonk(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                PumpFunTradeEvent => |e: PumpFunTradeEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdatepumpfun()' ───
                        if let Err(err) = Storage::tokenupdatepumpfun(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdatepumpswapbuy()' ───
                        if let Err(err) = Storage::tokenupdatepumpswapbuy(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                PumpSwapSellEvent => |e: PumpSwapSellEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdatepumpswapsell()' ───
                        if let Err(err) = Storage::tokenupdatepumpswapsell(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                RaydiumAmmV4AmmInfoAccountEvent => |e: RaydiumAmmV4AmmInfoAccountEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdateraydiumamm()' ───
                        if let Err(err) = Storage::tokenupdateraydiumamm(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                RaydiumClmmPoolStateAccountEvent => |e: RaydiumClmmPoolStateAccountEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdateraydiumclmm()' ───
                        if let Err(err) = Storage::tokenupdateraydiumclmm(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {

                    // ─── define 'dbstore' ───
                    let dbstore = Arc::clone(&storage);

                    // ─── proceed 'tokio' ───
                    tokio::spawn(async move {

                        // ─── compare 'Storage::tokenupdateraydiumcpmm()' ───
                        if let Err(err) = Storage::tokenupdateraydiumcpmm(&dbstore, &e).await {
                            error!("update write failed: {err}");
                        }
                    });
                },
                PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                    TradeMonitor::signalclose(e.quote_mint, "Withdraw");
                }
            });
        }
    }

    // ─── fn 'subgrpc' ───
    async fn subgrpc(self) -> Result<()> {

        // ─── define 'protocols' ───
        let protocols = vec![
            Protocol::PumpFun,
            Protocol::PumpSwap,
            Protocol::Bonk,
            Protocol::RaydiumCpmm,
            Protocol::RaydiumClmm,
            Protocol::RaydiumAmmV4,
        ];

        // ─── define 'account_include' ───
        let account_include = vec![
            PUMPFUN_PROGRAM_ID.to_string(),
            PUMPSWAP_PROGRAM_ID.to_string(),
            BONK_PROGRAM_ID.to_string(),
            RAYDIUM_CPMM_PROGRAM_ID.to_string(),
            RAYDIUM_CLMM_PROGRAM_ID.to_string(),
            RAYDIUM_AMM_V4_PROGRAM_ID.to_string(),
        ];

        // ─── define 'transaction_filter' ───
        let transaction_filter = TransactionFilter {
            account_include: account_include.clone(),
            account_exclude: vec![],
            account_required: vec![],
        };

        // ─── define 'account_filter' ───
        let account_filter = AccountFilter {
            account: vec![],
            owner: account_include.clone(),
        };

        // ─── define 'event_type_filter' ───
        let event_type_filter = Some(EventTypeFilter {
            include: vec![
                // Bonk
                EventType::BonkInitialize,
                EventType::BonkInitializeV2,
                EventType::BonkBuyExactIn,
                EventType::BonkBuyExactOut,
                EventType::BonkSellExactIn,
                EventType::BonkSellExactOut,
                // PumpFun
                EventType::PumpFunCreateToken,
                EventType::PumpFunBuy,
                EventType::PumpFunSell,
                // PumpSwap
                EventType::PumpSwapCreatePool,
                EventType::PumpSwapBuy,
                EventType::PumpSwapSell,
                EventType::PumpSwapWithdraw,
                // Raydium AMM
                EventType::RaydiumAmmV4Initialize2,
                EventType::RaydiumAmmV4SwapBaseIn,
                EventType::RaydiumAmmV4SwapBaseOut,
                EventType::RaydiumAmmV4Withdraw,
                // Raydium CLMM
                EventType::RaydiumClmmCreatePool,
                EventType::RaydiumClmmSwap,
                EventType::RaydiumClmmSwapV2,
                EventType::RaydiumClmmDecreaseLiquidityV2,
                EventType::RaydiumClmmClosePosition,
                // Raydium CPMM
                EventType::RaydiumCpmmInitialize,
                EventType::RaydiumCpmmSwapBaseInput,
                EventType::RaydiumCpmmSwapBaseOutput,
                EventType::RaydiumCpmmWithdraw,
            ],
        });

        info!("Starting to listen for events, press Ctrl+C to stop...");

        // ─── define 'storage' ───
        let storage = Arc::clone(&self.storage);

        // ─── define 'callback' ───
        let callback = Self::eventcallback(storage);

        // ─── callback 'self.grpc.subscribe_events_immediate()' ───
        self.grpc.subscribe_events_immediate(protocols, None, transaction_filter, account_filter, event_type_filter, None, callback)
            .await
            .map_err(|e| anyhow!("subscribing to Yellowstone stream: {e}"))?;

        info!("Waiting for Ctrl+C to stop...");
        tokio::signal::ctrl_c().await.map_err(|e| anyhow!("awaiting Ctrl+C: {e}"))?;
        Ok(())
    }

    // ─── fn 'run' ───
    pub async fn run(self) -> Result<()> {

        // ─── proceed 'self.subgrpc()' ───
        if let Err(e) = self.subgrpc().await {

            // ─── proceed 'e.to_string().contains()' ───
            if e.to_string().contains("Unauthenticated") {
                error!("Unauthenticated error —> Startup aborted");
                std::process::exit(1);
            } else {
                error!("Error occurred: {}", e);
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }
}

// ─── fn 'main' ───
#[tokio::main]
async fn main() -> Result<()> {

    // ─── callback 'Builder::from_env()' ───
    Builder::from_env(Env::default().default_filter_or("info,sqlx=warn")).format_timestamp_millis().init();

    // ─── callback 'GhostReaver::removedatabase()' ───
    GhostReaver::removedatabase().await?;

    // ─── callback 'GhostReaver::new()' ───
    GhostReaver::new(PATHCONFIGENDPOINT).await?.run().await
}