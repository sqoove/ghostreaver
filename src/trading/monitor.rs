// ─── imports packages ───
use anyhow::{anyhow, Result as AnyResult};
use chrono::Utc;
use log::{error, info};
use once_cell::sync::OnceCell;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock, Semaphore};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

// ─── import crates ───
use crate::globals::constants::*;
use crate::globals::pubkeys::*;
use crate::globals::statics::*;
use crate::schema::trade::TradeInfo;
use crate::trading::jupiter::SwapClient;
use crate::utils::loader::{ServerConfig, TradeConfig, WalletConfig};
use crate::utils::storage::Storage;

// ─── struct 'MintOpenGuard' ───
/// struct description

struct MintOpenGuard {
    storage: Arc<Storage>,
    mint: Pubkey,
    holder: Uuid,
    held: bool,
}

// ─── struct 'MintOpenGuard' ───
/// struct description
impl MintOpenGuard {

    // ─── fn 'acquire' ───
    /// fn description
    async fn acquire(storage: Arc<Storage>, mint: Pubkey, holder: Uuid) -> anyhow::Result<Option<Self>> {

        // ─── define 'got' ───
        let got = storage.mintlock(&mint, &holder).await?;

        // ─── compare 'got' ───
        if got {
            Ok(Some(Self { storage, mint, holder, held: true }))
        } else {
            Ok(None)
        }
    }
}

// ─── struct 'Drop for MintOpenGuard' ───
/// struct description
impl Drop for MintOpenGuard {

    // ─── fn 'drop' ───
    /// fn description
    fn drop(&mut self) {

        // ─── compare 'self.held' ───
        if self.held {

            // ─── define 'storage' ───
            let storage = Arc::clone(&self.storage);

            // ─── define 'mint' ───
            let mint = self.mint;

            // ─── define 'holder' ───
            let holder = self.holder;

            // ─── process 'tokio' ───
            tokio::spawn(async move {

                // ─── callback 'storage.mintunlock()' ───
                let _ = storage.mintunlock(&mint, &holder).await;
            });
        }
    }
}

// ─── struct 'CloseCmd' ───
/// struct description
#[derive(Clone, Debug)]
pub struct CloseCmd {
    pub reason: &'static str,
    pub uuid: Option<String>,
    pub mint: Option<Pubkey>
}

// ─── struct 'TradeMonitor' ───
/// struct description
pub struct TradeMonitor {
    pub initbalance: f64,
    pub storage: Arc<Storage>,
    closelocks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    openlocks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    readbudget: Arc<Semaphore>,
    rpcurl: OnceCell<String>,
    tradelock: Mutex<()>
}

// ─── impl 'TradeMonitor' ───
/// impl description
impl TradeMonitor {

    // ─── fn 'new' ───
    /// fn description
    pub fn new(storage: Arc<Storage>, initbalance: f64) -> Self {

        // ─── return 'Self' ───
        Self {
            initbalance,
            storage,
            closelocks: Arc::new(RwLock::new(HashMap::new())),
            openlocks: Arc::new(RwLock::new(HashMap::new())),
            readbudget: Arc::new(Semaphore::new(8)),
            rpcurl: OnceCell::new(),
            tradelock: Mutex::new(())
        }
    }

    // ─── fn 'closebroadcast' ───
    /// fn description
    fn closebroadcast() -> &'static broadcast::Sender<CloseCmd> {

        // ─── callback 'MONITORBUS.get_or_init()' ───
        MONITORBUS.get_or_init(|| {

            // ─── define 'tx' ───
            let (tx, _) = broadcast::channel::<CloseCmd>(64);

            // ─── return 'tx' ───
            tx
        })
    }

    // ─── fn 'subscribeclose' ───
    /// fn description
    fn subscribeclose() -> broadcast::Receiver<CloseCmd> {

        // ─── return 'Self' ───
        Self::closebroadcast().subscribe()
    }

    // ─── fn 'signalclose' ───
    /// fn description
    pub fn signalclose(mint: Pubkey, reason: &'static str) {

        // ─── callback 'Self::closebroadcast()' ───
        let _ = Self::closebroadcast().send(CloseCmd {
            reason,
            uuid: None,
            mint: Some(mint)
        });
    }

    // ─── fn 'endpointrpc' ───
    /// fn description
    fn endpointrpc(&self) -> &str {

        // ─── callback 'self.rpcurl' ───
        self.rpcurl.get_or_init(|| {

            // ─── define 'cfg' ───
            let cfg = ServerConfig::loadconfig(PATHCONFIGENDPOINT).expect("Missing/invalid server config at PATHCONFIGENDPOINT");

            // ─── return 'cfg.endpoint.rpc' ───
            cfg.endpoint.rpc
        })
    }

    // ─── fn 'proglabel' ───
    /// fn description
    fn proglabel(&self, key: &Pubkey) -> &'static str {

        // ─── callback 'PROGRAMLABELS.get()' ───
        PROGRAMLABELS.get(key).copied().unwrap_or("Unknown")
    }

    // ─── fn 'progthresholds' ───
    /// fn description
    pub fn progthresholds(program: &Pubkey, cfg: &TradeConfig) -> (Option<u64>, Option<u64>) {

        // ─── define 'rule' ───
        let rule = &cfg.bot.rules;

        // ─── compare 'program' ───
        if *program == bonk_pubkeys::PROGRAM {
            (
                rule.bonkmaxtokens,
                rule.bonkexit
            )
        } else if *program == pumpfun_pubkeys::PROGRAM {
            (
                rule.pumpfunmaxtokens,
                rule.pumpfunexit
            )
        } else if *program == pumpswap_pubkeys::PROGRAM {
            (
                rule.pumpswapmaxtokens,
                rule.pumpswapexit
            )
        } else if *program == raydiumamm_pubkeys::PROGRAM {
            (
                rule.raydiumammmaxtokens,
                rule.raydiumammexit
            )
        } else if *program == raydiumclmm_pubkeys::PROGRAM {
            (
                rule.raydiumclmmmaxtokens,
                rule.raydiumclmmexit
            )
        } else if *program == raydiumcpmm_pubkeys::PROGRAM {
            (
                rule.raydiumcpmmmaxtokens,
                rule.raydiumcpmmexit
            )
        } else {
            (
                Some(0), Some(0)
            )
        }
    }

    // ─── fn 'fetchprice' ───
    /// fn description
    async fn fetchprice(storage: &Storage, mintaddr: &Pubkey) -> Option<f64> {

        // ─── match 'storage' ───
        match storage.tokenprice(mintaddr).await {
            Ok(Some(p)) => Some(p),
            _ => None,
        }
    }

    // ─── fn 'guardlock' ───
    /// fn description
    async fn guardlock(locks: &Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>, uuid: &str) -> Arc<Mutex<()>> {

        // ─── comppare 'locks.read()' ───
        if let Some(found) = locks.read().await.get(uuid).cloned() {

            // ─── return 'Mutex' ───
            return found;
        }

        // ─── define 'w' ───
        let mut w = locks.write().await;

        // ─── define 'entry' ───
        let entry = w.entry(uuid.to_string()).or_insert_with(|| Arc::new(Mutex::new(())));

        // ─── return 'Mutex' ───
        entry.clone()
    }

    // ─── fn 'swapbuy' ───
    /// fn description
    async fn swapbuy(rpc: &str, mintaddr: &Pubkey, amount: f64, decimals: u8, wallet: &WalletConfig, sandbox: bool,
        priorityfees: u64, _maxslippage: u16, maxtokens: Option<u64>) -> AnyResult<(f64, f64, String)> {

        // ─── define 'client' ───
        let client = SwapClient::new().map_err(|e| anyhow!(e.to_string()))?;

        // ─── define 'res' ───
        let res = client.swapbase(amount, &mintaddr.to_string(), decimals, maxtokens, sandbox,
            &wallet.wallet.publicaddr, &wallet.wallet.privatekey, priorityfees, rpc)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;

        // ─── return 'AnyResult' ───
        Ok(res)
    }

    // ─── fn 'swapsell' ───
    /// fn description
    async fn swapsell(rpc: &str, mintaddr: &Pubkey, units: f64, decimals: u8, wallet: &WalletConfig, sandbox: bool,
        priorityfees: u64, _maxslippage: u16) -> AnyResult<(f64, f64, String)> {

        // ─── define 'client' ───
        let client = SwapClient::new().map_err(|e| anyhow!(e.to_string()))?;

        // ─── define 'res' ───
        let res = client.swapquote(units, &mintaddr.to_string(), decimals, sandbox,
            &wallet.wallet.publicaddr, &wallet.wallet.privatekey, priorityfees, rpc)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;

        // ─── return 'AnyResult' ───
        Ok(res)
    }

    // ─── fn 'swapbuy' ───
    /// fn description
    async fn closenow(rpc: &str, storage: &Storage, mintaddr: Pubkey, tradeuuid: &str, exitreason: &'static str,
        totalunits: f64, config: &TradeConfig, decimals: u8, wallet: Arc<WalletConfig>) {

        // ─── define 'bot' ───
        let bot = &config.bot;

        // ─── callback 'Self::handlerclose()' ───
        Self::handlerclose(rpc, storage, mintaddr, tradeuuid, exitreason, totalunits, bot.main.debug, bot.orders.sellslippage,
            bot.priority.outputfees, decimals, bot.orders.attempts, bot.main.sandbox, wallet).await;
    }

    // ─── fn 'exitbundler' ───
    /// fn description
    fn exitbundler(trailingstatus: bool, trailinghigh: f64, loadoutput: f64, totalpnl: f64, elapsed: i64,
        timexit: i64, bot: &crate::utils::loader::BotConfig) -> Option<&'static str> {

        // ─── compare 'trailingstatus' ───
        if trailingstatus {

            // ─── define 'trailinglimit' ───
            let trailinglimit = trailinghigh * (1.0 - bot.orders.trailingstop);

            // ─── compare 'loadoutput' ───
            if loadoutput <= trailinglimit {

                // ─── return 'Option' ───
                return Some("Trailing");
            }
        }

        // ─── compare 'bot.orders.takeprofit' ───
        if totalpnl >= bot.orders.takeprofit {

            // ─── return 'Option' ───
            return Some("Takeprofit");
        }

        // ─── compare 'bot.orders.stoploss' ───
        if totalpnl <= -bot.orders.stoploss {

            // ─── return 'Option' ───
            return Some("Stoploss");
        }

        // ─── compare 'timexit' ───
        if !trailingstatus && elapsed >= timexit {

            // ─── return 'Option' ───
            return Some("Holdtime");
        }

        // ─── return 'Option' ───
        None
    }

    // ─── fn 'tradelogger' ───
    /// fn description
    fn tradelogger(trailingstatus: bool, trailinghigh: f64, tradequote: f64, perf: f64, elapsed: i64, timerstart: i64,
        trailingstop: f64, takeprofit: f64, stoploss: f64, timexit: i64, timeclose: i64, tradeuuid: &str) {

        // ─── define 'reasons' ───
        let mut reasons = Vec::new();

        // ─── compare 'trailingstatus' ───
        if trailingstatus {

            // ─── define 'trailinglimit' ───
            let trailinglimit = trailinghigh * (1.0 - trailingstop);
            reasons.push(format!("Trailing-stop: High {:.12} | Limit {:.12} | Current {:.12} | perf {:.12}", trailinghigh, trailinglimit, tradequote, perf));
        }

        reasons.push(if perf >= takeprofit {
            "Take-profit: Above".to_string()
        } else {
            "Take-profit: Below".to_string()
        });

        reasons.push(if perf <= -stoploss {
            "Stop-loss: Below".to_string()
        } else {
            "Stop-loss: Above".to_string()
        });

        // ─── define 'elapsed' ───
        if elapsed < timexit {
            reasons.push(format!("Holdtime: {}s remaining", (timexit - elapsed) / 1000));
        } else {
            reasons.push("Holdtime passed".to_string());
        }

        // ─── define 'timeout' ───
        let timeout = (timeclose - (Utc::now().timestamp_millis() - timerstart)) / 1000;
        reasons.push(format!("Timeout: {}s remaining", timeout));

        // ─── Console log ───
        info!("[Trade UUID: {}]", tradeuuid);

        // ─── proceed 'for' ───
        for reason in reasons {
            info!("{}", reason);
        }

        info!("{}", "-".repeat(140));
    }

    // ─── fn 'openflight' ───
    /// fn description
    fn openflight() -> &'static Arc<Semaphore> {

        // ─── callback 'OPENFLIGHT.get_or_init()' ───
        OPENFLIGHT.get_or_init(|| Arc::new(Semaphore::new(1)))
    }

    // ─── fn 'handlerorder' ───
    /// fn description
    pub async fn handlerorder(&self, mintaddr: Pubkey, programaddr: Pubkey, slot: i64, localtime: i64, decimals: u8,
        tokenprice: f64, bot: Arc<TradeConfig>, wallet: Arc<WalletConfig>) -> Option<(f64, f64, String, f64)> {

        // ─── define 'cfg' ───
        let cfg = &bot.bot;

        // ─── define 'storage' ───
        let storage = Arc::clone(&self.storage);

        // ─── define 'holder' ───
        let holder = Uuid::new_v4();

        // ─── define 'storarc' ───
        let storarc = Arc::clone(&self.storage);

        // ─── define '_mint_guard' ───
        let Some(_) = MintOpenGuard::acquire(storarc, mintaddr, holder).await.map_err(|error| {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug {
                error!("Lock acquire error for mint {}: {}", mintaddr, error);
                info!("{}", "-".repeat(140));
            }
            error
        }).ok().flatten()
        else {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug {
                error!("Mint {} is already being opened elsewhere -> Aborting trade creation", mintaddr);
                info!("{}", "-".repeat(140));
            }

            return None;
        };

        // ─── define 'openflight' ───
        let openflight = match Self::openflight().clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => return None,
        };

        // ─── define 'tradelock' ───
        let tradelock = self.tradelock.lock().await;

        // ─── define 'mintkey' ───
        let mintkey = mintaddr.to_string();

        // ─── define 'mintlock' ───
        let mintlock = Self::guardlock(&self.openlocks, &mintkey).await;

        // ─── define 'mintguard' ───
        let mintguard = mintlock.lock().await;

        // ─── define 'latency' ───
        let latency = Utc::now().timestamp_millis() - localtime;

        // ─── compare 'cfg.rules.maxtokenage' ───
        if let Some(maxage) = cfg.rules.maxtokenage {

            // ─── compare 'maxage' ───
            if latency > maxage as i64 {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug == true {

                    // ─── Console log ───
                    error!("Mint {} older than {} ms (Latency: {}) -> Aborting trade creation", mintaddr, maxage, latency);
                    info!("{}", "-".repeat(140));
                }

                // ─── callback 'drop()' ───
                drop(mintguard);
                drop(tradelock);
                drop(openflight);
                return None;
            }
        }

        // ─── compare 'programaddr' ───
        if cfg.monitoring.programs != "all" && cfg.monitoring.programs != programaddr.to_string() {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Program {} is not allowed for mint {} -> Aborting trade creation", programaddr, mintaddr);
                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        }

        // ─── compare 'cfg.main.opentrades' ───
        if cfg.main.opentrades > 0 {

            // ─── compare 'self.storage.tradescount()' ───
            if let Ok(open) = self.storage.tradescount().await {

                // ─── compare 'cfg.main.opentrades' ───
                if open >= cfg.main.opentrades as i64 {

                    // ─── compare 'cfg.main.debug' ───
                    if cfg.main.debug == true {

                        // ─── Console log ───
                        error!("Open trades limit reached -> Aborting trade creation");
                        info!("{}", "-".repeat(140));
                    }

                    // ─── callback 'drop()' ───
                    drop(mintguard);
                    drop(tradelock);
                    drop(openflight);
                    return None;
                }
            }
        }

        // ─── compare 'cfg.main.maxtrades' ───
        if cfg.main.maxtrades > 0 {

            // ─── compare 'self.storage.tradestotal()' ───
            if let Ok(open) = self.storage.tradestotal().await {

                // ─── compare 'cfg.main.maxtrades' ───
                if open >= cfg.main.maxtrades as i64 {

                    // ─── compare 'cfg.main.debug' ───
                    if cfg.main.debug == true {

                        // ─── Console log ───
                        error!("Maximum trades limit reached -> Aborting trade creation");
                        info!("{}", "-".repeat(140));
                    }

                    // ─── callback 'drop()' ───
                    drop(mintguard);
                    drop(tradelock);
                    drop(openflight);
                    return None;
                }
            }
        }

        // ─── compare 'self.storage.tradeexists()' ───
        if let Ok(true) = self.storage.tradeexists(&mintaddr).await {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Token {} already traded -> Aborting trade creation", mintaddr);
                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        }

        // ─── define 'loadbalance' ───
        let loadbalance = self.storage.walletbalance().await.ok().flatten();

        // ─── define 'loadbalance' ───
        let Some(balance) = loadbalance else {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Not enough fund to trade token {} -> Aborting trade creation", mintaddr);
                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        };

        // ─── compare 'self.storage.tokentxs()' ───
        let transactions: i64 = match self.storage.tokentxs(&mintaddr).await {
            Ok(Some(p)) => p,
            Ok(None) => {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug {
                    error!("Unable to retrieve transactions for mint {} -> defaulting to 0", mintaddr);
                    info!("{}", "-".repeat(140));
                }
                0i64
            }
            Err(e) => {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug {
                    error!("Error while retrieving transactions for mint {} -> defaulting to 0: {}", mintaddr, e);
                    info!("{}", "-".repeat(140));
                }
                0i64
            }
        };

        // ─── compare 'self.storage.tokensupply()' ───
        let supply: i64 = match self.storage.tokensupply(&mintaddr).await {
            Ok(Some(p)) => p,
            Ok(None) => {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug {
                    error!("Unable to retrieve supply for mint {} -> defaulting to 0", mintaddr);
                    info!("{}", "-".repeat(140));
                }
                0i64
            }
            Err(e) => {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug {
                    error!("Error while retrieving supply for mint {} -> defaulting to 0: {}", mintaddr, e);
                    info!("{}", "-".repeat(140));
                }
                0i64
            }
        };

        // ─── define 'self.storage.tokenpools()' ───
        let (poolbase, poolquote): (f64, f64) = match self.storage.tokenpools(&mintaddr).await {
            Ok(Some((optbase, optquote))) => {

                // ─── define 'base' ───
                let base = optbase.unwrap_or(0.0);

                // ─── define 'quote' ───
                let quote = optquote.unwrap_or(0.0);

                // ─── compare 'cfg.main.debug' ───
                if (optbase.is_none() || optquote.is_none()) && cfg.main.debug {
                    error!("Pool values missing for mint {} -> base={:?}, quote={:?} -> defaulting missing legs to 0.0", mintaddr, optbase, optquote);
                    info!("{}", "-".repeat(140));
                }
                (base, quote)
            }
            Ok(None) => {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug {
                    error!("Unable to retrieve pools for mint {} -> defaulting to (0.0, 0.0)", mintaddr);
                    info!("{}", "-".repeat(140));
                }
                (0.0, 0.0)
            }
            Err(e) => {

                // ─── compare 'cfg.main.debug' ───
                if cfg.main.debug {
                    error!("Error while retrieving pools for mint {} -> defaulting to (0.0, 0.0): {}", mintaddr, e);
                    info!("{}", "-".repeat(140));
                }
                (0.0, 0.0)
            }
        };

        // ─── define 'poolbase' ───
        let poolbase = (tokenprice * poolquote) + poolbase;

        // ─── callback 'Self::progthresholds(&programaddr' ───
        let (maxtokens, maxhold) = Self::progthresholds(&programaddr, bot.as_ref());

        // ─── define 'label' ───
        let label = self.proglabel(&programaddr);

        // ─── define 'rejectcause' ───
        let rejectcause: Vec<String> = Vec::new();

        // ─── compare '!rejectcause.is_empty()' ───
        if !rejectcause.is_empty() {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("[Filter Rejected] Mint: {} | Program: {}", mintaddr, label);

                // ─── proceed 'for' ───
                for r in &rejectcause {
                    error!("{}", r);
                }

                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        }

        // ─── define 'amount' ───
        let amount = cfg.orders.amount;

        // ─── define 'cost' ───
        let cost = amount + (cfg.priority.inputfees / 1_000_000_000.0);

        // ─── compare 'cost' ───
        if balance < cost {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Wallet balance too low to trade {mintaddr} -> Aborting trade creation");
                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        }

        // ─── define 'rpc' ───
        let rpc = self.endpointrpc().to_string();

        // ─── define '(tradeunits, tradefees, tradehash)' ───
        let (tradeunits, tradefees, tradehash) = {

            // ─── define 'attempts' ───
            let mut attempts = 0;

            // ─── proceed 'loop' ───
            loop {

                // ─── increment 'attempts' ───
                attempts += 1;

                // ─── match 'Self::swapbuy()' ───
                match Self::swapbuy(&rpc, &mintaddr, cfg.orders.amount, decimals, &wallet, cfg.main.sandbox, cfg.priority.inputfees as u64,
                    (cfg.orders.buyslippage * 100.0).round() as u16, maxtokens).await {
                    Ok((quotetotal, quotefees, signature)) => {
                        break (quotetotal, quotefees, signature);
                    }
                    Err(e) => {

                        // ─── define 'emsg' ───
                        let emsg = e.to_string();

                        // ─── compare 'emsg.starts_with()' ───
                        if emsg.starts_with("OutOfRangeTokens:") {

                            // ─── compare 'cfg.main.debug' ───
                            if cfg.main.debug {

                                // ─── Console log ───
                                error!("[Rejected] {} for mint {}", emsg, mintaddr);
                                info!("{}", "-".repeat(140));
                            }

                            // ─── callback 'drop()' ───
                            drop(mintguard);
                            drop(tradelock);
                            drop(openflight);
                            return None;
                        }

                        // ─── generic failure ───
                        if cfg.main.debug {

                            // ─── Console log ───
                            error!("[Buy Failed - Attempt {}/{}] Mint {}: {}", attempts, cfg.orders.attempts, mintaddr, emsg);
                            info!("{}", "-".repeat(140));
                        }

                        // ─── compare 'attempts' ───
                        if attempts >= cfg.orders.attempts {
                            // ─── callback 'drop()' ───
                            drop(mintguard);
                            drop(tradelock);
                            drop(openflight);
                            return None;
                        }

                        // ─── callback 'sleep' ───
                        sleep(Duration::from_millis(2000 + rand::random::<u16>() as u64)).await;
                    }
                }
            }
        };

        // ─── define 'tradecost' ───
        let tradecost = cfg.orders.amount + tradefees;

        // ─── compare 'storage.walletbalance()' ───
        if let Ok(Some(oldbalance)) = storage.walletbalance().await {

            // ─── define 'newbalance' ───
            let newbalance = oldbalance - tradecost;

            // ─── callback 'storage.walletupdate()' ───
            let _ = storage.walletupdate(newbalance).await;
        } else {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Failed to get wallet balance -> Aborting trade creation");
                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        }

        // ─── define 'tradeuuid' ───
        let tradeuuid = Uuid::new_v4().to_string();

        // ─── define 'tradeinfo' ───
        let tradeinfo = TradeInfo::init(tradeuuid.clone(), mintaddr, tradecost, tradeunits, tradehash.clone(), programaddr, slot);

        // ─── compare 'storage.tradecreate()' ───
        if let Err(e) = storage.tradecreate(&tradeinfo).await {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Function 'tradecreate' failed for {}: {}", tradeuuid, e);
                info!("{}", "-".repeat(140));
            }

            // ─── callback 'drop()' ───
            drop(mintguard);
            drop(tradelock);
            drop(openflight);
            return None;
        }

        // ─── define 'tradeopen' ───
        let tradeopen = tradecost / tradeunits;

        // ─── callback 'storage.marketopen()' ───
        let _ = storage.marketopen(&tradeuuid, tradeopen).await;

        // ─── compare 'cfg.main.sandbox' ───
        if cfg.main.sandbox {

            // ─── callback 'storage.tickinsert()' ───
            let _ = storage.tickinsert(&tradeuuid, Utc::now().timestamp_millis(), tradeopen).await;
        }

        // ─── callback 'storage.signaturelog()' ───
        let _ = storage.signaturelog(&tradeuuid, &mintaddr, &tradehash).await;

        // ─── define 'tradespread' ───
        let tradespread = (tradeopen - tokenprice) / tokenprice;

        // ─── compare 'cfg.main.debug' ───
        if cfg.main.debug == true {

            // ─── Console log ───
            info!("[Trade Opened]");
            info!("Mint: {}", mintaddr);
            info!("Price: {}", tradeopen);
            info!("Fees: {}", tradefees);
            info!("Cost: {}", tradecost);
            info!("UUID: {}", tradeuuid);
            info!("Signature: {}", tradehash);
            info!("Spread: {}", tradespread * 100.00);
            info!("Program: {}", programaddr);
            info!("{}", "-".repeat(140));
        }

        // ─── compare 'self.storage.tradeupdate()' ───
        if let Err(e) = self.storage.tradeupdate(&tradeuuid, poolbase, decimals, transactions, tradespread, supply as f64, latency).await {

            // ─── compare 'cfg.main.debug' ───
            if cfg.main.debug == true {

                // ─── Console log ───
                error!("Function 'tradeupdate' failed for {}: {}", tradeuuid, e);
                info!("{}", "-".repeat(140));
            }
        }

        // ─── callback 'drop()' ───
        drop(tradelock);
        drop(openflight);
        drop(mintguard);

        // ─── define 'storage' ───
        let storage = Arc::clone(&self.storage);

        // ─── define 'timexit' ───
        let timexit: i64 = maxhold.map(|v| v as i64).unwrap_or(0);

        // ─── define 'txuuid' ───
        let txuuid = tradeuuid.clone();

        // ─── define 'closelocks' ───
        let closelocks = Arc::clone(&self.closelocks);

        // ─── define 'readbudget' ───
        let readbudget = Arc::clone(&self.readbudget);

        // ─── define 'closerx' ───
        let closerx = Self::subscribeclose();

        // ─── proceed 'tokio' ───
        tokio::spawn(async move {

            // ─── callback 'TradeMonitor::handlerfollow()' ───
            TradeMonitor::handlerfollow(rpc, bot, storage, mintaddr, tradeuuid.clone(), tradecost, tradeunits,
                decimals, tradespread, wallet, timexit, closelocks, readbudget, closerx).await;
        });

        // ─── return 'Option' ───
        Some((tradecost, tradeunits, txuuid, tradespread))
    }

    // ─── fn 'handlerfollow' ───
    /// fn description
    pub async fn handlerfollow(rpc: String, config: Arc<TradeConfig>, storage: Arc<Storage>, mintaddr: Pubkey, tradeuuid: String,
        tradecost: f64, totalunits: f64, decimals: u8, spread: f64, wallet: Arc<WalletConfig>, timexit: i64, locks: Arc<RwLock<HashMap<String,
        Arc<Mutex<()>>>>>, realbudget: Arc<Semaphore>, mut closerx: broadcast::Receiver<CloseCmd>) {

        // ─── define 'bot' ───
        let bot = &config.bot;

        // ─── define 'exitreason' ───
        let mut exitreason = "Timeout";

        // ─── define 'timerstart' ───
        let timerstart = Utc::now().timestamp_millis();

        // ─── define 'trailingstatus' ───
        let mut trailingstatus = false;

        // ─── define 'trailinghigh' ───
        let mut trailinghigh = tradecost;

        // ─── define 'totaltrade' ───
        let mut totaltrade = tradecost;

        // ─── define 'tokenunits' ───
        let mut tokenunits = totalunits;

        // ─── define 'partialsell' ───
        let mut partialsell = false;

        // ─── define 'trailcount' ───
        let mut trailcount: i64 = 0;

        // ─── define 'nextlevel' ───
        let mut nextlevel: f64 = 0.0;

        // ─── define 'realized' ───
        let mut realized: f64 = 0.0;

        // ─── compare 'storage.tradestate()' ───
        if let Ok(Some((remamount, remtoken, ps, pc, nl, rz))) = storage.tradestate(&tradeuuid).await {

            // ─── compare 'remtoken' ───
            if remamount > 0.0 && remtoken > 0.0 {
                totaltrade = remamount;
                tokenunits = remtoken;
            }

            partialsell = ps;
            trailcount = pc;
            nextlevel = nl;
            realized = rz;
        }

        // ─── define 'primarylevel' ───
        let primarylevel = bot.orders.partialtrigger;

        // ─── compare 'nextlevel' ───
        if partialsell && nextlevel == 0.0 {
            nextlevel = primarylevel + bot.orders.trailingtrigger * (trailcount as f64 + 1.0);
        }

        // ─── define 'slipfraction' ───
        let slipfraction = bot.orders.sellslippage / 100.0;

        // ─── proceed 'while' ───
        while Utc::now().timestamp_millis() - timerstart < bot.orders.timeclose {

            // ─── define 'elapsed' ───
            let elapsed = Utc::now().timestamp_millis() - timerstart;

            // ─── define 'permit' ───
            let permit = realbudget.acquire().await.expect("semaphore closed");

            // ─── define 'loadprice' ───
            let loadprice = Self::fetchprice(storage.as_ref(), &mintaddr).await;

            // ─── callback 'drop()' ───
            drop(permit);

            // ─── define 'rawprice' ───
            let rawprice = match loadprice {
                Some(p) => p,
                None => return,
            };

            // ─── define 'shortprice' ───
            let shortprice = rawprice * (1.0 + spread) * (1.0 - slipfraction);

            // ─── callback 'storage.marketclose()' ───
            let _ = storage.marketclose(&tradeuuid, shortprice).await;

            // ─── compare 'bot.main.sandbox' ───
            if bot.main.sandbox {

                // ─── callback 'storage.tickupdate()' ───
                let _ = storage.tickupdate(&tradeuuid, Utc::now().timestamp_millis(), shortprice, 5.0, 1_000).await;
            }

            // ─── define 'loadoutput' ───
            let loadoutput = tokenunits * shortprice * (1.0 - 0.0);

            // ─── define 'performance' ───
            let performance = if totaltrade > 0.0 {
                (loadoutput - totaltrade) / totaltrade
            } else {
                0.0
            };

            // ─── define 'totalpnl' ───
            let totalpnl = if tradecost > 0.0 {
                (realized + loadoutput - tradecost) / tradecost
            } else {
                0.0
            };

            // ─── compare 'bot.orders.dropmax' ───
            if bot.orders.dropmax > 0.0 {

                // ─── define 'permit' ───
                let permit = realbudget.acquire().await.expect("semaphore closed");

                // ─── define 'liq' ───
                let liq = storage.tokenliquidity(&mintaddr).await;

                // ─── callback 'drop()' ───
                drop(permit);

                // ─── compare 'liq' ───
                if let Ok(Some((initialbase, initialquote, finalbase, finalquote))) = liq {

                    // ─── define 'initbase' ───
                    let initbase = initialbase.unwrap_or(0.0);

                    // ─── define 'initquote' ───
                    let initquote = initialquote.unwrap_or(0.0);

                    // ─── define 'lastbase' ───
                    let lastbase = finalbase.unwrap_or(0.0);

                    // ─── define 'lastquote' ───
                    let lastquote = finalquote.unwrap_or(0.0);

                    // ─── define 'initliq' ───
                    let initliq = initbase + rawprice * initquote;

                    // ─── define 'lastliq' ───
                    let lastliq = lastbase + rawprice * lastquote;

                    // ─── define 'dropratio' ───
                    let dropratio = if initliq > 0.0 {
                        (initliq - lastliq) / initliq
                    } else {
                        0.0
                    };

                    // ─── compare 'bot.orders.dropmax' ───
                    if dropratio >= bot.orders.dropmax {

                        // ─── compare 'cfg.main.debug' ───
                        if bot.main.debug == true {

                            // ─── Console log ───
                            error!("[Drain Detected]");
                            error!("Mint: {}", mintaddr);
                            error!("UUID: {}", &tradeuuid);
                            error!("Init Liquidity: {:.12} | Last Liquidity: {:.12} | Drop: {:.4}", initliq, lastliq, dropratio);
                            info!("{}", "-".repeat(140));
                        }

                        exitreason = "LiqDrain";

                        // ─── define 'lock' ───
                        let lock = Self::guardlock(&locks, &tradeuuid).await;

                        // ─── callback 'lock.lock()' ───
                        let _ = lock.lock().await;

                        // ─── callback 'Self::closenow()' ───
                        Self::closenow(&rpc, &storage, mintaddr, &tradeuuid, exitreason, tokenunits, &config, decimals, Arc::clone(&wallet)).await;

                        // ─── return ───
                        return;
                    }
                }
            }

            // ─── compare 'cfg.main.debug' ───
            if bot.main.debug == true {

                // ─── callback 'Self::tradelogger()' ───
                Self::tradelogger(trailingstatus, trailinghigh, loadoutput, performance, elapsed, timerstart, bot.orders.trailingdrop,
                    bot.orders.takeprofit, bot.orders.stoploss, timexit, bot.orders.timeclose, &tradeuuid);
            }

            // ─── compare 'trailingstatus' ───
            if performance >= bot.orders.trailingstop && !trailingstatus {
                trailingstatus = true;
                trailinghigh = loadoutput;

                // ─── compare 'cfg.main.debug' ───
                if bot.main.debug == true {

                    // ─── Console log ───
                    info!("[Trailing Activated]");
                    info!("Mint: {}", mintaddr);
                    info!("UUID: {}", &tradeuuid);
                    info!("{}", "-".repeat(140));
                }
            }

            // ─── compare 'trailingstatus' ───
            if trailingstatus {

                // ─── define 'trailinglimit' ───
                let trailinglimit = trailinghigh * (1.0 - bot.orders.trailingdrop);

                // ─── compare 'loadoutput' ───
                if loadoutput <= trailinglimit {

                    // ─── compare 'cfg.main.debug' ───
                    if bot.main.debug == true {

                        // ─── Console log ───
                        info!("[Trailing Stop]");
                        info!("Mint: {}", mintaddr);
                        info!("UUID: {}", &tradeuuid);
                        info!("High {:.12} | Limit {:.12} | Current {:.12}", trailinghigh, trailinglimit, loadoutput);
                        info!("{}", "-".repeat(140));
                    }

                    exitreason = "Trailing";

                    // ─── define 'lock' ───
                    let lock = Self::guardlock(&locks, &tradeuuid).await;

                    // ─── callback 'lock.lock()' ───
                    let _ = lock.lock().await;

                    // ─── callback 'Self::closenow()' ───
                    Self::closenow(&rpc, &storage, mintaddr, &tradeuuid, exitreason, tokenunits, &config, decimals, Arc::clone(&wallet)).await;

                    // ─── return ───
                    return;
                }
            }

            // ─── compare 'primarylevel' ───
            if !partialsell && totalpnl >= primarylevel {

                // ─── define 'shortunits' ───
                let shortunits = tokenunits * bot.orders.partialsell;

                // ─── compare 'shortunits' ───
                if shortunits > 0.0 {

                    // ─── define 'guardlock' ───
                    let guardlock = Self::guardlock(&locks, &tradeuuid).await;

                    // ─── callback 'guardlock.lock()' ───
                    let _ = guardlock.lock().await;

                    // ─── define 'attempts' ───
                    let mut attempts = 0_u64;

                    // ─── define 'success' ───
                    let mut success = false;

                    // ─── proceed 'while' ───
                    while attempts < bot.orders.attempts {

                        // ─── increment 'attempts' ───
                        attempts += 1;

                        // ─── match 'Self::swapsell()' ───
                        match Self::swapsell(&rpc, &mintaddr, shortunits, decimals, &wallet, bot.main.sandbox,
                            bot.priority.outputfees as u64, (bot.orders.sellslippage * 100.0).round() as u16).await {
                            Ok((proceeds, tradefees, tradehash)) => {

                                // ─── compare 'storage.walletbalance()' ───
                                if let Some(oldbal) = storage.walletbalance().await.ok().flatten() {

                                    // ─── callback 'storage.walletupdate()' ───
                                    let _ = storage.walletupdate(oldbal + proceeds).await;
                                }

                                // ─── callback 'storage.tradepartialsell()' ───
                                let _ = storage.tradepartialsell(&tradeuuid, proceeds).await;
                                realized += proceeds;

                                // ─── define 'prevunits' ───
                                let prevunits = tokenunits;
                                tokenunits = (tokenunits - shortunits).max(0.0);

                                // ─── compare 'prevunits' ───
                                if prevunits > 0.0 {
                                    totaltrade *= tokenunits / prevunits;
                                }

                                partialsell = true;
                                trailcount = 0;
                                nextlevel = primarylevel + bot.orders.trailingtrigger * 1.0;

                                // ─── callback 'storage.traderemaining()' ───
                                let _ = storage.traderemaining(&tradeuuid, totaltrade, tokenunits, partialsell, trailcount, nextlevel).await;

                                // ─── callback 'storage.signaturelog()' ───
                                let _ = storage.signaturelog(&tradeuuid, &mintaddr, &tradehash).await;
                                trailingstatus = true;

                                // ─── define 'newestimate' ───
                                let newestimate = tokenunits * shortprice * (1.0 - 0.0);
                                trailinghigh = newestimate;

                                // ─── compare 'cfg.main.debug' ───
                                if bot.main.debug == true {

                                    // ─── Console log ───
                                    info!("[Partial Sell #1 Executed]");
                                    info!("Mint: {}", mintaddr);
                                    info!("UUID: {}", &tradeuuid);
                                    info!("Units sold: {:.12}", shortunits);
                                    info!("Proceeds: {:.12}", proceeds);
                                    info!("Fees: {}", tradefees);
                                    info!("Signature: {}", tradehash);
                                    info!("Remaining: {:.12}", tokenunits);
                                    info!("New basis: {:.12}", totaltrade);
                                    info!("Next trailing: +{:.4}x", nextlevel);
                                    info!("{}", "-".repeat(140));
                                }

                                success = true;
                                break;
                            }
                            Err(e) => {

                                // ─── compare 'cfg.main.debug' ───
                                if bot.main.debug == true {

                                    // ─── Console log ───
                                    error!("[Attempt {}/{}] Partial #1 swapquote failed for mint {}: {}", attempts, bot.orders.attempts, mintaddr, e);
                                    info!("{}", "-".repeat(140));
                                }
                            }
                        }

                        // ─── callback 'sleep()' ───
                        sleep(Duration::from_millis(500)).await;
                    }

                    // ─── compare 'success' ───
                    if !success {

                        // ─── compare 'cfg.main.debug' ───
                        if bot.main.debug == true {

                            // ─── Console log ───
                            error!("Partial #1 aborted after {} attempts", bot.orders.attempts);
                            info!("{}", "-".repeat(140));
                        }
                    }
                }
            }

            // ─── proceed 'while' ───
            while partialsell && nextlevel > 0.0 && performance >= nextlevel {

                // ─── define 'shortunits' ───
                let shortunits = tokenunits * bot.orders.trailingsell;

                // ─── compare 'shortunits' ───
                if shortunits <= 0.0 {
                    break;
                }

                // ─── define 'guardlock' ───
                let guardlock = Self::guardlock(&locks, &tradeuuid).await;

                // ─── callback 'guardlock.lock()' ───
                let _ = guardlock.lock().await;

                // ─── define 'attempts' ───
                let mut attempts = 0_u64;

                // ─── define 'success' ───
                let mut success = false;

                // ─── proceed 'while' ───
                while attempts < bot.orders.attempts {

                    // ─── increment 'attempts' ───
                    attempts += 1;

                    // ─── match 'Self::swapsell()' ───
                    match Self::swapsell(&rpc, &mintaddr, shortunits, decimals, &wallet, bot.main.sandbox,
                        bot.priority.outputfees as u64, (bot.orders.sellslippage * 100.0).round() as u16).await {
                        Ok((proceeds, tradefees, tradehash)) => {

                            // ─── compare 'storage.walletbalance()' ───
                            if let Some(oldbal) = storage.walletbalance().await.ok().flatten() {

                                // ─── callback 'storage.walletupdate()' ───
                                let _ = storage.walletupdate(oldbal + proceeds).await;
                            }

                            // ─── callback 'storage.tradepartialsell()' ───
                            let _ = storage.tradepartialsell(&tradeuuid, proceeds).await;
                            realized += proceeds;

                            // ─── define 'prevunits' ───
                            let prevunits = tokenunits;
                            tokenunits = (tokenunits - shortunits).max(0.0);

                            // ─── compare 'prevunits' ───
                            if prevunits > 0.0 {
                                totaltrade *= tokenunits / prevunits;
                            }

                            trailcount += 1;
                            nextlevel = primarylevel + bot.orders.trailingtrigger * (trailcount as f64 + 1.0);

                            // ─── callback 'storage.traderemaining()' ───
                            let _ = storage.traderemaining(&tradeuuid, totaltrade, tokenunits, partialsell, trailcount, nextlevel).await;

                            // ─── callback 'storage.signaturelog()' ───
                            let _ = storage.signaturelog(&tradeuuid, &mintaddr, &tradehash).await;

                            // ─── define 'newestimate' ───
                            let newestimate = tokenunits * shortprice * (1.0 - 0.0);
                            trailinghigh = newestimate;

                            // ─── compare 'cfg.main.debug' ───
                            if bot.main.debug == true {

                                // ─── Console log ───
                                info!("[Trailing Sell #{trailcount} Executed]");
                                info!("Mint: {}", mintaddr);
                                info!("UUID: {}", &tradeuuid);
                                info!("Units sold: {:.12}", shortunits);
                                info!("Proceeds: {:.12}", proceeds);
                                info!("Fees: {}", tradefees);
                                info!("Signature: {}", tradehash);
                                info!("Remaining: {:.12}", tokenunits);
                                info!("New basis: {:.12}", totaltrade);
                                info!("Next trailing: +{:.4}x", nextlevel);
                                info!("{}", "-".repeat(140));
                            }

                            success = true;
                            break;
                        }
                        Err(e) => {

                            // ─── compare 'cfg.main.debug' ───
                            if bot.main.debug == true {

                                // ─── Console log ───
                                error!("[Attempt {}/{}] Trailing partial swapquote failed for mint {}: {}", attempts, bot.orders.attempts, mintaddr, e);
                                info!("{}", "-".repeat(140));
                            }
                        }
                    }

                    // ─── callback 'sleep()' ───
                    sleep(Duration::from_millis(500)).await;
                }

                // ─── compare 'success' ───
                if !success {

                    // ─── compare 'cfg.main.debug' ───
                    if bot.main.debug == true {

                        // ─── Console log ───
                        error!("Trailing partial (level {:.4}) aborted after {} attempts", nextlevel, bot.orders.attempts);
                        info!("{}", "-".repeat(140));
                    }

                    break;
                }

                // ─── define 'newoutput' ───
                let newoutput = tokenunits * shortprice * (1.0 - 0.0);

                // ─── define 'perfreload' ───
                let perfreload = if totaltrade > 0.0 {
                    (newoutput - totaltrade) / totaltrade
                } else {
                    0.0
                };

                // ─── compare 'nextlevel' ───
                if !(partialsell && nextlevel > 0.0 && perfreload >= nextlevel) {
                    break;
                }
            }

            // ─── compare 'trailingstatus' ───
            if trailingstatus {
                trailinghigh = trailinghigh.max(loadoutput);
            }

            // ─── compare 'Self::exitbundler()' ───
            if let Some(reason) = Self::exitbundler(trailingstatus, trailinghigh, loadoutput, totalpnl, elapsed, timexit, bot) {
                exitreason = reason;

                // ─── define 'guardlock' ───
                let guardlock = Self::guardlock(&locks, &tradeuuid).await;

                // ─── callback 'guardlock.lock()' ───
                let _ = guardlock.lock().await;

                // ─── callback 'Self::closenow()' ───
                Self::closenow(&rpc, &storage, mintaddr, &tradeuuid, exitreason, tokenunits, &config, decimals, Arc::clone(&wallet)).await;

                // ─── return ───
                return;
            }

            // ─── proceed 'tokio' ───
            tokio::select! {

                // ─── callback 'sleep()' ───
                _ = sleep(Duration::from_millis(500)) => {},
                Ok(cmd) = closerx.recv() => {

                    // ─── define 'hituuid' ───
                    let hituuid = cmd.uuid.as_deref() == Some(&tradeuuid);

                    // ─── define 'hit_mint' ───
                    let hitmint = cmd.mint.as_ref().map(|m| m == &mintaddr).unwrap_or(false);

                    // ─── compare 'hituuid/hitmint' ───
                    if hituuid || hitmint {

                        // ─── define 'guardlock' ───
                        let guardlock = Self::guardlock(&locks, &tradeuuid).await;

                        // ─── callback 'guardlock.lock()' ───
                        let _ = guardlock.lock().await;

                        // ─── callback 'Self::closenow()' ───
                        Self::closenow(&rpc, &storage, mintaddr, &tradeuuid, cmd.reason, tokenunits, &config, decimals, Arc::clone(&wallet)).await;

                        // ─── return ───
                        return;
                    }
                }
            }
        }

        // ─── define 'guardlock' ───
        let guardlock = Self::guardlock(&locks, &tradeuuid).await;

        // ─── callback 'guardlock.lock()' ───
        let _ = guardlock.lock().await;

        // ─── callback 'Self::closenow()' ───
        Self::closenow(&rpc, &storage, mintaddr, &tradeuuid, exitreason, tokenunits, &config, decimals, Arc::clone(&wallet)).await;
    }

    // ─── fn 'handlerclose' ───
    /// fn description
    pub async fn handlerclose(rpc: &str, storage: &Storage, mintaddr: Pubkey, tradeuuid: &str, exitreason: &str,
        tradeunits: f64, debug: bool, sellslippage: f64, outputfees: f64, decimals: u8, maxretries: u64,
        sandbox: bool, wallet: Arc<WalletConfig>) {

        // ─── define 'tokenclose' ───
        let tokenclose = Self::fetchprice(storage, &mintaddr).await;

        // ─── compare 'storage.walletbalance()' ───
        if let Some(oldbalance) = storage.walletbalance().await.ok().flatten() {
            let mut attempt = 0;

            // ─── proceed 'attempt' ───
            while attempt < maxretries {

                // ─── increment 'attempt' ───
                attempt += 1;

                // ─── match 'Self::swapsell()' ───
                match Self::swapsell(rpc, &mintaddr, tradeunits, decimals, wallet.as_ref(), sandbox,
                    outputfees as u64, (sellslippage * 100.0).round() as u16).await {
                    Ok((tradeoutput, tradefees, tradehash)) => {

                        // ─── define 'realized' ───
                        let realized = storage.traderealized(tradeuuid).await.unwrap_or(0.0);

                        // ─── define 'cumulative' ───
                        let cumulative = realized + tradeoutput;

                        // ─── define 'newbalance' ───
                        let newbalance = oldbalance + tradeoutput;

                        // ─── callback 'storage.walletupdate()' ───
                        let _ = storage.walletupdate(newbalance).await;

                        // ─── callback 'storage.tradeclose()' ───
                        let _ = storage.tradeclose(tradeuuid, cumulative, exitreason).await;

                        // ─── callback 'storage.signaturelog()' ───
                        let _ = storage.signaturelog(tradeuuid, &mintaddr, &tradehash).await;

                        // ─── compare 'tokenclose' ───
                        if let Some(tp) = tokenclose {

                            // ─── callback 'storage.marketclose()' ───
                            let _ = storage.marketclose(tradeuuid, tp).await;
                        }

                        // ─── compare 'debug' ───
                        if debug {

                            // ─── Console log ───
                            info!("[Trade Closed]");

                            // ─── compare 'tokenclose' ───
                            if let Some(tp) = tokenclose {
                                info!("Price RPC: {}", tp);
                            } else {
                                info!("Price RPC: <unavailable>");
                            }
                            // ─── compare 'tokenclose' ───
                            info!("Mint: {}", mintaddr);
                            info!("UUID: {}", tradeuuid);
                            info!("Fees: {}", tradefees);
                            info!("Signature: {}", tradehash);
                            info!("{}", "-".repeat(140));
                        }

                        // ─── return ' ───
                        return;
                    }
                    Err(e) => {

                        // ─── compare 'debug' ───
                        if debug {

                            // ─── Console log ───
                            error!("[Attempt {}/{}] Failed to sell token → SOL quote for {}: {}", attempt, maxretries, mintaddr, e);
                            info!("{}", "-".repeat(140));
                        }
                    }
                }

                // ─── callback 'sleep()' ───
                sleep(Duration::from_secs(1)).await;
            }

            // ─── callback 'storage.tradeclose()' ───
            let _ = storage.tradeclose(tradeuuid, 0.00, exitreason).await;

            // ─── compare 'tokenclose' ───
            if let Some(tp) = tokenclose {

                // ─── callback 'storage.marketclose()' ───
                let _ = storage.marketclose(tradeuuid, tp).await;

                // ─── compare 'sandbox' ───
                if sandbox {

                    // ─── callback 'storage.tickinsert()' ───
                    let _ = storage.tickinsert(tradeuuid, Utc::now().timestamp_millis(), tp).await;
                }
            }

            // ─── compare 'debug' ───
            if debug {

                // ─── Console log ───
                error!("Giving up on trade {} after {} failed attempts to convert token → SOL", tradeuuid, maxretries);
                info!("{}", "-".repeat(140));
            }

            // ─── return ' ───
            return;
        }
    }
}