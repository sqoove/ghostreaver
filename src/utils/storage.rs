// ─── imports packages ───
use futures::{TryStreamExt, future::join_all};
use solana_sdk::pubkey::Pubkey;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::QueryBuilder;
use sqlx::{Pool, Row, Postgres, Transaction};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

// ─── import crates ───
use crate::globals::constants::*;
use crate::globals::database::*;
use crate::globals::pubkeys::*;
use crate::globals::statics::*;
use crate::schema::trade::TradeInfo;
use crate::streaming::events::protocols::bonk::events::{BonkPoolCreateEvent, BonkTradeEvent};
use crate::streaming::events::protocols::pumpfun::events::{PumpFunCreateTokenEvent, PumpFunTradeEvent};
use crate::streaming::events::protocols::pumpswap::events::{PumpSwapCreatePoolEvent, PumpSwapBuyEvent, PumpSwapSellEvent};
use crate::streaming::events::protocols::raydiumamm::events::{RaydiumAmmV4Initialize2Event, RaydiumAmmV4AmmInfoAccountEvent};
use crate::streaming::events::protocols::raydiumclmm::events::{RaydiumClmmCreatePoolEvent, RaydiumClmmPoolStateAccountEvent};
use crate::streaming::events::protocols::raydiumcpmm::events::{RaydiumCpmmInitializeEvent, RaydiumCpmmSwapEvent};
use crate::trading::monitor::TradeMonitor;
use crate::trading::scanner::Scanner;
use crate::utils::loader::{TradeConfig, WalletConfig};
use crate::utils::scripts::Scripts;

// ─── struct 'TickKey' ───
/// struct description
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct TickKey {
    uuid: String,
    servtime: i64
}

// ─── struct 'TickMsg' ───
/// struct description
#[derive(Debug, Clone)]
struct TickMsg {
    uuid: String,
    servtime: i64,
    price: f64
}

// ─── struct 'TokenRow' ───
/// struct description
#[derive(Clone, Debug)]
pub struct TokenRow {
    pub uuid: String,
    pub signature: String,
    pub slot: i64,
    pub blocktime: i64,
    pub program: String,
    pub mint: String,
    pub creator: String,
    pub pool: String,
    pub basevault: String,
    pub quotevault: String
}

// ─── struct 'TokenUpdate' ───
/// struct description
#[derive(Debug, Clone)]
struct TokenUpdate {
    uuid: String,
    price: Option<f64>,
    lastbase: Option<i64>,
    lastquote: Option<i64>,
    decimals: Option<i32>,
    supply: Option<i64>,
    initbase: Option<i64>,
    initquote: Option<i64>,
    txsinc: i64
}

// ─── struct 'Storage' ───
/// struct description
#[derive(Clone)]
pub struct Storage {
    pub readpool: Pool<Postgres>,
    pub writepool: Pool<Postgres>,
    ticktx: mpsc::Sender<TickMsg>,
    tokentx: mpsc::Sender<TokenUpdate>
}

// ─── impl 'Storage' ───
/// impl description
impl Storage {

    // ─── fn 'new' ───
    /// fn description
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'dbpath' ───
        let dbpath = std::env::var("DATABASE_URL").unwrap_or_else(|_| DATABASEPATH.to_string());

        // ─── define 'baseopts' ───
        let baseopts = {

            // ─── define 'opts' ───
            let mut opts = PgConnectOptions::from_str(&dbpath)?;
            opts = opts.statement_cache_capacity(256);
            opts
        };

        // ─── define 'readpool' ───
        let readpool = PgPoolOptions::new()
            .max_connections(48)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Some(Duration::from_secs(120)))
            .max_lifetime(Some(Duration::from_secs(900)))
            .test_before_acquire(false)
            .connect_lazy_with(baseopts.clone().application_name("ghostreaver_read"));

        // ─── define 'writepool' ───
        let writepool = PgPoolOptions::new()
            .max_connections(24)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Some(Duration::from_secs(120)))
            .max_lifetime(Some(Duration::from_secs(900)))
            .test_before_acquire(false)
            .connect_lazy_with(baseopts.clone().application_name("ghostreaver_write"));

        // ─── define 'tickpool' ───
        let tickpool = PgPoolOptions::new()
            .max_connections(12)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Some(Duration::from_secs(120)))
            .max_lifetime(Some(Duration::from_secs(900)))
            .test_before_acquire(false)
            .connect_lazy_with(baseopts.clone().application_name("ghostreaver_tick"));

        // ─── callback 'Self::buildschema()' ───
        Self::buildschema(&readpool).await?;

        // ─── callback 'Self::buildindexes()' ───
        Self::buildindexes(&readpool).await?;

        // ─── define 'ticktx' ───
        let ticktx = Self::tickwriter(tickpool);

        // ─── define 'tokentx' ───
        let tokentx = Self::tokenwriter(writepool.clone());

        // ─── callback 'Self::tradecache()' ───
        Self::tradecache(&readpool).await?;

        // ─── callback 'Self::prewarmfast()' ───
        Self::prewarmfast(&readpool, &writepool, &ticktx).await.ok();

        // ─── result 'Result' ───
        Ok(Self { readpool, writepool, ticktx, tokentx })
    }

    // ─── fn 'primepool' ───
    /// fn description
    async fn primetouch(pool: &Pool<Postgres>) -> sqlx::Result<()> {

        // ─── define 'conn' ───
        let mut conn = pool.acquire().await?;

        // ─── callback 'sqlx::query_as()' ───
        let _: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&mut *conn).await?;
        Ok(())
    }

    // ─── fn 'primepool' ───
    /// fn description
    async fn primepool(pool: &Pool<Postgres>, n: usize) -> sqlx::Result<()> {

        // ─── define 'tasks' ───
        let tasks = (0..n).map(|_| Self::primetouch(pool));

        // ─── callback 'join_all()' ───
        let _ = join_all(tasks).await;
        Ok(())
    }

    // ─── fn 'primewriter' ───
    /// fn description
    async fn primewriter(ticktx: &mpsc::Sender<TickMsg>) -> anyhow::Result<()> {

        // ─── callback 'ticktx.send()' ───
        let _ = ticktx.send(TickMsg {
            uuid: "prewarm".to_string(),
            servtime: (chrono::Utc::now().timestamp_millis() / 1000) * 1000,
            price: 0.0,
        }).await;
        Ok(())
    }

    // ─── fn 'prewarmfast' ───
    /// fn description
    async fn prewarmfast(readpool: &Pool<Postgres>, writepool: &Pool<Postgres>, ticktx: &mpsc::Sender<TickMsg>) -> anyhow::Result<()> {

        // ─── callback 'Self::primepool()' ───
        let _ = Self::primepool(readpool, 6).await;

        // ─── callback 'Self::primepool()' ───
        let _ = Self::primepool(writepool, 4).await;

        // ─── callback 'Self::primewriter()' ───
        let _ = Self::primewriter(ticktx).await;
        Ok(())
    }

    // ─── fn 'buildschema' ───
    /// fn description
    async fn buildschema(pool: &Pool<Postgres>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'conn' ───
        let mut conn = pool.acquire().await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE TABLE IF NOT EXISTS locks (
            mint        TEXT PRIMARY KEY,
            acquired    BIGINT NOT NULL,
            holder      TEXT NOT NULL
        )").execute(&mut *conn).await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE TABLE IF NOT EXISTS market (
            uuid            TEXT PRIMARY KEY,
            open            DOUBLE PRECISION,
            close           DOUBLE PRECISION
        )").execute(&mut *conn).await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE UNLOGGED TABLE IF NOT EXISTS ticks (
            uuid            TEXT NOT NULL,
            servtime        BIGINT NOT NULL,
            price           DOUBLE PRECISION NOT NULL,
            PRIMARY KEY     (uuid, servtime)
        )").execute(&mut *conn).await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE TABLE IF NOT EXISTS tokens (
            uuid            TEXT PRIMARY KEY,
            signature       TEXT UNIQUE,
            slot            BIGINT,
            blocktime       BIGINT,
            program         TEXT,
            mint            TEXT,
            creator         TEXT,
            pool            TEXT,
            price           DOUBLE PRECISION NULL,
            basevault       TEXT NOT NULL,
            quotevault      TEXT NOT NULL,
            initbase        BIGINT NULL,
            initquote       BIGINT NULL,
            lastbase        BIGINT NULL,
            lastquote       BIGINT NULL,
            decimals        INTEGER NULL,
            supply          BIGINT NULL,
            txs             BIGINT NULL,
            servtime        BIGINT NOT NULL,
            tokenage        BIGINT NULL
        )").execute(&mut *conn).await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE TABLE IF NOT EXISTS trades (
            id              BIGSERIAL PRIMARY KEY,
            uuid            TEXT NOT NULL,
            mint            TEXT NOT NULL,
            amount          DOUBLE PRECISION NOT NULL,
            token           DOUBLE PRECISION NOT NULL,
            hash            TEXT NOT NULL,
            program         TEXT NOT NULL,
            profit          DOUBLE PRECISION,
            total           DOUBLE PRECISION,
            exit            TEXT,
            poolsize        DOUBLE PRECISION,
            decimals        INTEGER,
            duration        BIGINT DEFAULT 0,
            slot            BIGINT NOT NULL,
            txs             BIGINT DEFAULT 0,
            servtime        BIGINT NOT NULL,
            spread          DOUBLE PRECISION,
            supply          DOUBLE PRECISION,
            latency         BIGINT,
            remamount       DOUBLE PRECISION,
            remtoken        DOUBLE PRECISION,
            realized        DOUBLE PRECISION DEFAULT 0.0,
            partialsell     INTEGER DEFAULT 0,
            trailcount      BIGINT DEFAULT 0,
            nextlevel       DOUBLE PRECISION DEFAULT 0.0
        )").execute(&mut *conn).await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE TABLE IF NOT EXISTS signature (
            id              BIGSERIAL PRIMARY KEY,
            uuid            TEXT NOT NULL,
            mint            TEXT NOT NULL,
            signature       TEXT NOT NULL,
            servtime        BIGINT NOT NULL
        )").execute(&mut *conn).await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("CREATE TABLE IF NOT EXISTS wallet (
            id              BIGINT PRIMARY KEY,
            balance         DOUBLE PRECISION NOT NULL,
            profit          DOUBLE PRECISION DEFAULT 0.0,
            baseline        DOUBLE PRECISION
        )").execute(&mut *conn).await?;

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    async fn buildindexes(pool: &Pool<Postgres>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'con' ───
        let mut conn = pool.begin().await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("SET LOCAL statement_timeout = '60s'").execute(&mut *conn).await?;
        sqlx::query("SET LOCAL lock_timeout = '3s'").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_signature_mint ON signature (mint)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_signature_uuid ON signature (uuid)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_creator ON tokens (creator)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_mint ON tokens (mint)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_cover ON tokens (mint, program, servtime DESC) INCLUDE (uuid, signature, slot, blocktime, creator, pool, basevault, quotevault)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_pool ON tokens (pool)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_program ON tokens (program)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_slot ON tokens (slot)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_uuid ON tokens (uuid)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tokens_vaults ON tokens (basevault, quotevault)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_hash ON trades (hash)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_mint ON trades (mint) WHERE total IS NULL").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_uuid ON trades (uuid)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_null ON trades (uuid) WHERE total IS NULL").execute(&mut *conn).await?;

        // ─── analyze fresh stats ───
        sqlx::query("ANALYZE tokens").execute(&mut *conn).await?;
        sqlx::query("ANALYZE trades").execute(&mut *conn).await?;
        sqlx::query("ANALYZE signature").execute(&mut *conn).await?;
        sqlx::query("ANALYZE wallet").execute(&mut *conn).await?;

        // ─── callback 'conn.commit()' ───
        conn.commit().await?;

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'droptables' ───
    /// fn description
    pub async fn droptables(&self) -> Result<(), sqlx::Error> {

        // ─── define 'conn' ───
        let mut conn = self.writepool.acquire().await?;

        // ─── callback 'sqlx::query()' ───
        sqlx::query("DROP TABLE IF EXISTS market CASCADE").execute(&mut *conn).await?;
        sqlx::query("DROP TABLE IF EXISTS signature CASCADE").execute(&mut *conn).await?;
        sqlx::query("DROP TABLE IF EXISTS ticks CASCADE").execute(&mut *conn).await?;
        sqlx::query("DROP TABLE IF EXISTS tokens CASCADE").execute(&mut *conn).await?;
        sqlx::query("DROP TABLE IF EXISTS trades CASCADE").execute(&mut *conn).await?;
        sqlx::query("DROP TABLE IF EXISTS wallet CASCADE").execute(&mut *conn).await?;

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'exporttables' ───
    /// fn description
    pub async fn exporttables(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'outdir' ───
        let outdir = PathBuf::from("database");

        // ─── ensure dir ───
        if !outdir.exists() {
            fs::create_dir_all(&outdir)?;
        }

        // ─── define 'filemarket' ───
        let mut filemarket = File::create(outdir.join("market.csv"))?;

        // ─── callback 'writeln' ───
        writeln!(filemarket, "uuid,open,close")?;

        // ─── define 'rowsmarket' ───
        let mut rowsmarket = sqlx::query("SELECT uuid, open, close FROM market ORDER BY uuid").fetch(&self.readpool);

        // ─── proceed 'while' ───
        while let Some(r) = rowsmarket.try_next().await? {

            // ─── define 'uuidvvv' ───
            let uuid: String = r.try_get("uuid")?;

            // ─── define 'open' ───
            let open: Option<f64> = r.try_get("open")?;

            // ─── define 'close' ───
            let close: Option<f64> = r.try_get("close")?;

            // ─── callback 'writeln' ───
            writeln!(filemarket, "{},{},{}", Scripts::csvescape(&uuid), Scripts::optstring(open), Scripts::optstring(close))?;
        }

        // ─── define 'fileticks' ───
        let mut fileticks = File::create(outdir.join("ticks.csv"))?;

        // ─── callback 'writeln' ───
        writeln!(fileticks, "uuid,servtime,price")?;

        // ─── define 'rowsticks' ───
        let mut rowsticks = sqlx::query("SELECT uuid, servtime, price FROM ticks ORDER BY uuid, servtime").fetch(&self.readpool);

        // ─── proceed 'while' ───
        while let Some(r) = rowsticks.try_next().await? {

            // ─── define 'uuid' ───
            let uuid: String = r.try_get("uuid")?;

            // ─── define 'servtime' ───
            let servtime: i64 = r.try_get("servtime")?;

            // ─── define 'price' ───
            let price: f64 = r.try_get("price")?;

            // ─── callback 'writeln' ───
            writeln!(fileticks, "{},{},{}", Scripts::csvescape(&uuid), servtime, price)?;
        }

        // ─── define 'filetokens' ───
        let mut filetokens = File::create(outdir.join("tokens.csv"))?;

        // ─── callback 'writeln' ───
        writeln!(filetokens, "uuid,signature,slot,blocktime,program,mint,creator,pool,price,basevault,quotevault,initbase,initquote,lastbase,lastquote,decimals,supply,txs,servtime,tokenage")?;

        // ─── define 'rowstokens' ───
        let mut rowstokens = sqlx::query(r#"SELECT uuid, signature, slot, blocktime, program,
            mint, creator, pool, price, basevault, quotevault, initbase, initquote, lastbase, lastquote, decimals,
            supply, txs, servtime, tokenage FROM tokens ORDER BY servtime"#).fetch(&self.readpool);

        // ─── proceed 'while' ───
        while let Some(r) = rowstokens.try_next().await? {

            // ─── define 'uuid' ───
            let uuid: String = r.try_get("uuid")?;

            // ─── define 'signature' ───
            let signature: String = r.try_get("signature")?;

            // ─── define 'slot' ───
            let slot: Option<i64> = r.try_get("slot")?;

            // ─── define 'blocktime' ───
            let blocktime: Option<i64> = r.try_get("blocktime")?;

            // ─── define 'program' ───
            let program: String = r.try_get("program")?;

            // ─── define 'mint' ───
            let mint: String = r.try_get("mint")?;

            // ─── define 'creator' ───
            let creator: String = r.try_get("creator")?;

            // ─── define 'pool' ───
            let pool: String = r.try_get("pool")?;

            // ─── define 'price' ───
            let price: Option<f64> = r.try_get("price")?;

            // ─── define 'basevault' ───
            let basevault: String = r.try_get("basevault")?;

            // ─── define 'quotevault' ───
            let quotevault: String = r.try_get("quotevault")?;

            // ─── define 'initbase' ───
            let initbase: Option<i64> = r.try_get("initbase")?;

            // ─── define 'initquote' ───
            let initquote: Option<i64> = r.try_get("initquote")?;

            // ─── define 'lastbase' ───
            let lastbase: Option<i64> = r.try_get("lastbase")?;

            // ─── define 'lastquote' ───
            let lastquote: Option<i64> = r.try_get("lastquote")?;

            // ─── define 'decimals' ───
            let decimals: Option<i32> = r.try_get("decimals")?;

            // ─── define 'supply' ───
            let supply: Option<i64> = r.try_get("supply")?;

            // ─── define 'txs' ───
            let txs: Option<i64> = r.try_get("txs")?;

            // ─── define 'servtime' ───
            let servtime: i64 = r.try_get("servtime")?;

            // ─── define 'tokenage' ───
            let tokenage: Option<i64> = r.try_get("tokenage")?;

            // ─── callback 'writeln' ───
            writeln!(filetokens, "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                 Scripts::csvescape(&uuid),
                 Scripts::csvescape(&signature),
                 Scripts::optstring(slot),
                 Scripts::optstring(blocktime),
                 Scripts::csvescape(&program),
                 Scripts::csvescape(&mint),
                 Scripts::csvescape(&creator),
                 Scripts::csvescape(&pool),
                 Scripts::optstring(price),
                 Scripts::csvescape(&basevault),
                 Scripts::csvescape(&quotevault),
                 Scripts::optstring(initbase),
                 Scripts::optstring(initquote),
                 Scripts::optstring(lastbase),
                 Scripts::optstring(lastquote),
                 Scripts::optstring(decimals),
                 Scripts::optstring(supply),
                 Scripts::optstring(txs),
                 servtime,
                 Scripts::optstring(tokenage)
            )?;
        }

        // ─── define 'filetrades' ───
        let mut filetrades = File::create(outdir.join("trades.csv"))?;

        // ─── callback 'writeln' ───
        writeln!(filetrades, "id,uuid,mint,amount,token,hash,program,profit,total,exit,poolsize,decimals,duration,slot,txs,servtime,spread,supply,latency,remamount,remtoken")?;

        // ─── define 'rowstrades' ───
        let mut rowstrades = sqlx::query(r#"SELECT id, uuid, mint, amount, token,
            hash, program, profit, total, exit, poolsize, decimals, duration, slot, txs, servtime, spread,
            supply, latency, remamount, remtoken FROM trades ORDER BY id"#).fetch(&self.readpool);

        // ─── proceed 'while' ───
        while let Some(r) = rowstrades.try_next().await? {

            // ─── define 'rowstrades' ───
            let id: i64 = r.try_get("id")?;

            // ─── define 'rowstrades' ───
            let uuid: String = r.try_get("uuid")?;

            // ─── define 'rowstrades' ───
            let mint: String = r.try_get("mint")?;

            // ─── define 'rowstrades' ───
            let amount: f64 = r.try_get("amount")?;

            // ─── define 'rowstrades' ───
            let token: f64 = r.try_get("token")?;

            // ─── define 'rowstrades' ───
            let hash: String = r.try_get("hash")?;

            // ─── define 'rowstrades' ───
            let program: String = r.try_get("program")?;

            // ─── define 'rowstrades' ───
            let profit: Option<f64> = r.try_get("profit")?;

            // ─── define 'rowstrades' ───
            let total: Option<f64> = r.try_get("total")?;

            // ─── define 'rowstrades' ───
            let exit: Option<String> = r.try_get("exit")?;

            // ─── define 'rowstrades' ───
            let poolsize: Option<f64> = r.try_get("poolsize")?;

            // ─── define 'rowstrades' ───
            let decimals: Option<i32> = r.try_get("decimals")?;

            // ─── define 'rowstrades' ───
            let duration: i64 = r.try_get("duration")?;

            // ─── define 'rowstrades' ───
            let slot: i64 = r.try_get("slot")?;

            // ─── define 'rowstrades' ───
            let txs: i64 = r.try_get("txs")?;

            // ─── define 'rowstrades' ───
            let servtime: i64 = r.try_get("servtime")?;

            // ─── define 'rowstrades' ───
            let spread: Option<f64> = r.try_get("spread")?;

            // ─── define 'rowstrades' ───
            let supply: Option<f64> = r.try_get("supply")?;

            // ─── define 'rowstrades' ───
            let latency: Option<i64> = r.try_get("latency")?;

            // ─── define 'rowstrades' ───
            let remamount: Option<f64> = r.try_get("remamount")?;

            // ─── define 'rowstrades' ───
            let remtoken: Option<f64> = r.try_get("remtoken")?;

            // ─── callback 'writeln' ───
            writeln!(filetrades, "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                id,
                Scripts::csvescape(&uuid),
                Scripts::csvescape(&mint),
                amount,
                token,
                Scripts::csvescape(&hash),
                Scripts::csvescape(&program),
                Scripts::optstring(profit),
                Scripts::optstring(total),
                exit.map(|v| Scripts::csvescape(&v)).unwrap_or_default(),
                Scripts::optstring(poolsize),
                Scripts::optstring(decimals),
                duration,
                slot,
                txs,
                servtime,
                Scripts::optstring(spread),
                Scripts::optstring(supply),
                Scripts::optstring(latency),
                Scripts::optstring(remamount),
                Scripts::optstring(remtoken)
            )?;
        }

        // ─── define 'filesignature' ───
        let mut filesignature = File::create(outdir.join("signature.csv"))?;

        // ─── callback 'writeln' ───
        writeln!(filesignature, "id,uuid,mint,signature,servtime")?;

        // ─── define 'rowssignature' ───
        let mut rowssignature = sqlx::query("SELECT id, uuid, mint, signature, servtime FROM signature ORDER BY id").fetch(&self.readpool);

        // ─── proceed 'while' ───
        while let Some(r) = rowssignature.try_next().await? {

            // ─── define 'id' ───
            let id: i64 = r.try_get("id")?;

            // ─── define 'uuid' ───
            let uuid: String = r.try_get("uuid")?;

            // ─── define 'mint' ───
            let mint: String = r.try_get("mint")?;

            // ─── define 'signature' ───
            let signature: String = r.try_get("signature")?;

            // ─── define 'servtime' ───
            let servtime: i64 = r.try_get("servtime")?;

            // ─── callback 'writeln' ───
            writeln!(filesignature, "{},{},{},{},{}",
                 id,
                 Scripts::csvescape(&uuid),
                 Scripts::csvescape(&mint),
                 Scripts::csvescape(&signature),
                 servtime
            )?;
        }

        // ─── define 'filewallet' ───
        let mut filewallet = File::create(outdir.join("wallet.csv"))?;

        // ─── callback 'writeln' '───
        writeln!(filewallet, "id,balance,profit,baseline")?;

        // ─── define 'rowswallet' ───
        let mut rowswallet = sqlx::query("SELECT id, balance, profit, baseline FROM wallet ORDER BY id").fetch(&self.readpool);

        // ─── proceed 'while' ───
        while let Some(r) = rowswallet.try_next().await? {

            // ─── define 'id' ───
            let id: i64 = r.try_get("id")?;

            // ─── define 'balance' ───
            let balance: f64 = r.try_get("balance")?;

            // ─── define 'profit' '───
            let profit: f64 = r.try_get("profit")?;

            // ─── define 'baseline' '───
            let baseline: f64 = r.try_get("baseline")?;

            // ─── callback 'writeln' '───
            writeln!(filewallet, "{},{},{},{}", id, balance, profit, baseline)?;
        }

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn marketclose(&self, uuid: &str, price: f64) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("INSERT INTO market (uuid, close) VALUES ($1, $2) ON CONFLICT(uuid) DO UPDATE SET close = EXCLUDED.close")
            .bind(uuid)
            .bind(price)
            .execute(&self.writepool)
            .await?;

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn marketopen(&self, uuid: &str, price: f64) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("INSERT INTO market (uuid, open) VALUES ($1, $2) ON CONFLICT(uuid) DO UPDATE SET open = EXCLUDED.open")
            .bind(uuid)
            .bind(price)
            .execute(&self.writepool)
            .await?;

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'mintlock' ───
    /// fn description
    pub async fn mintlock(&self, mint: &Pubkey, holder: &Uuid) -> sqlx::Result<bool> {

        // ─── define 'now' ───
        let now = chrono::Utc::now().timestamp_millis();

        // ─── define 'r' ───
        let r: Option<(String,)> = sqlx::query_as("INSERT INTO locks (mint, acquired, holder)
            VALUES ($1, $2, $3) ON CONFLICT (mint) DO NOTHING RETURNING mint")
            .bind(mint.to_string())
            .bind(now)
            .bind(holder.to_string())
            .fetch_optional(&self.writepool)
            .await?;

        // ─── result 'Result' ───
        Ok(r.is_some())
    }

    // ─── fn 'mintunlock' ───
    /// fn description
    pub async fn mintunlock(&self, mint: &Pubkey, holder: &Uuid) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("DELETE FROM locks WHERE mint = $1 AND holder = $2")
            .bind(mint.to_string())
            .bind(holder.to_string())
            .execute(&self.writepool)
            .await?;

        // ─── result 'Result' ───
        Ok(())
    }

    // ─── fn 'ticksflush' ───
    /// fn description
    async fn ticksflush(conn: &mut sqlx::pool::PoolConnection<Postgres>, pool: &Pool<Postgres>, map: &mut HashMap<TickKey, f64>) {

        // ─── compare 'map.is_empty()' ───
        if map.is_empty() {
            return;
        }

        // ─── define 'batch' ───
        let mut batch: Vec<(String, i64, f64)> = Vec::with_capacity(map.len());

        // ─── proceed 'for' ───
        for (k, v) in map.drain() {
            batch.push((k.uuid, k.servtime, v));
        }

        // ─── define 'start' ───
        let mut start = 0usize;

        // ─── proceed 'while' ───
        while start < batch.len() {

            // ─── define 'end' ───
            let end = (start + POSTGRESTICKMAXBATCH.max(2000)).min(batch.len());

            // ─── define 'slice' ───
            let slice = &batch[start..end];

            // ─── define 'qb' ───
            let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO ticks (uuid, servtime, price) ");
            qb.push_values(slice, |mut b, (uuid, ts, price)| {
                b.push_bind(uuid).push_bind(*ts).push_bind(*price);
            });
            qb.push(" ON CONFLICT (uuid, servtime) DO UPDATE SET price = EXCLUDED.price");

            // ─── compare 'qb.build()' ───
            if let Err(e) = qb.build().execute(&mut **conn).await {
                eprintln!("tickwriter: execute error: {e}");

                // ─── match 'pool.acquire()' ───
                match pool.acquire().await {
                    Ok(c) => *conn = c,
                    Err(e) => {
                        eprintln!("tickwriter: reacquire failed: {e}");

                        // ─── callback 'tokio::time::sleep()' ───
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                }
            }

            // ─── return 'start' ───
            start = end;
        }
    }

    // ─── fn 'tickwriter' ───
    /// fn description
    fn tickwriter(pool: Pool<Postgres>) -> mpsc::Sender<TickMsg> {

        // ─── define '(tx, rx)' ───
        let (tx, mut rx) = mpsc::channel::<TickMsg>(POSTGRESTICKCHANNEL.max(32768));

        // ─── proceed 'tokio' ───
        tokio::spawn(async move {

            // ─── define 'conn' ───
            let mut conn = match pool.acquire().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("tickwriter: cannot acquire dedicated conn: {e}");
                    return;
                }
            };

            // ─── define 'map' ───
            let mut map: HashMap<TickKey, f64> = HashMap::with_capacity(16_384);

            // ─── define 'lastflush' ───
            let mut lastflush = Instant::now();

            // ─── proceed 'loop' ───
            loop {

                // ─── define 'drained' ───
                let mut drained = 0usize;

                // ─── proceed 'while' ───
                while let Ok(msg) = rx.try_recv() {

                    // ─── define 'key' ───
                    let key = TickKey { uuid: msg.uuid, servtime: msg.servtime };
                    map.insert(key, msg.price);
                    drained += 1;

                    // ─── compare 'map.len()' ───
                    if map.len() >= POSTGRESTICKMAXBATCH {
                        break;
                    }
                }

                // ─── compare 'drained' ───
                if drained == 0 {

                    // ─── proceed 'tokio' ───
                    tokio::select! {
                        Some(msg) = rx.recv() => {

                            // ─── define 'key' ───
                            let key = TickKey { uuid: msg.uuid, servtime: msg.servtime };
                            map.insert(key, msg.price);
                        }

                        // ─── callback 'tokio::time::sleep()' ───
                        _ = tokio::time::sleep(Duration::from_millis(POSTGRESTICKFLUSHMS)) => {}
                    }
                }

                // ─── compare 'map.len()' ───
                if map.len() >= (POSTGRESTICKMAXBATCH / 2) || lastflush.elapsed() >= Duration::from_millis(POSTGRESTICKFLUSHMS) {

                    // ─── callback 'Self::ticksflush()' ───
                    Self::ticksflush(&mut conn, &pool, &mut map).await;
                    lastflush = Instant::now();
                }
            }
        });

        // ─── return 'tx' ───
        tx
    }

    // ─── fn 'tickinsert' ───
    /// fn description
    pub async fn tickinsert(&self, uuid: &str, servtime: i64, price: f64) -> sqlx::Result<()> {

        // ─── define 'tsms' ───
        let tsms = (servtime / 1000) * 1000;

        // ─── define 'msg' ───
        let msg = TickMsg { uuid: uuid.to_string(), servtime: tsms, price };

        // ─── callback 'self.ticktx.send()' ───
        let _ = self.ticktx.send(msg).await;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tickupdate' ───
    /// fn description
    pub async fn tickupdate(&self, uuid: &str, servtime: i64, price: f64, thresholdbps: f64, intms: i64) -> sqlx::Result<bool> {

        // ─── define 'should' ───
        let should = {

            // ─── compare 'POSTGRESLASTTICK' ───
            if let Some((lasttime, lastprice)) = POSTGRESLASTTICK.get(uuid).map(|v| *v) {

                // ─── define 'tsn' ───
                let tsn = servtime - lasttime < intms;

                // ─── define 'rlm' ───
                let rlm = if lastprice > 0.0 {
                    ((price - lastprice) / lastprice).abs()
                } else { f64::INFINITY };

                // ─── define 'thr' ───
                let thr = (thresholdbps / 10_000.0).max(0.0);
                !(tsn && rlm < thr)
            } else {
                true
            }
        };

        // ─── compare 'should' ───
        if should {
            POSTGRESLASTTICK.insert(uuid.to_string(), (servtime, price));

            // ─── define 'tsms' ───
            let tsms = (servtime / 1000) * 1000;

            // ─── callback 'self.ticktx.send()' ───
            let _ = self.ticktx.send(TickMsg { uuid: uuid.to_string(), servtime: tsms, price }).await;

            // ─── return 'Result' ───
            return Ok(true);
        }

        // ─── return 'Result' ───
        Ok(false)
    }

    // ─── fn 'tokensflush' ───
    /// fn description
    async fn tokensflush(conn: &mut sqlx::pool::PoolConnection<Postgres>, pending: &mut HashMap<String, TokenUpdate>) {

        // ─── compare 'pending.is_empty()' ───
        if pending.is_empty() {
            return;
        }

        // ─── define 'items' ───
        let mut items: Vec<TokenUpdate> = Vec::with_capacity(pending.len());

        // ─── proceed 'for' ───
        for (_, v) in pending.drain() {
            items.push(v);
        }

        // ─── define 'uuids' ───
        let mut uuids: Vec<String> = Vec::with_capacity(items.len());

        // ─── define 'price' ───
        let mut price: Vec<Option<f64>> = Vec::with_capacity(items.len());

        // ─── define 'lastbase' ───
        let mut lastbase: Vec<Option<i64>>= Vec::with_capacity(items.len());

        // ─── define 'lastquote' ───
        let mut lastquote: Vec<Option<i64>>=Vec::with_capacity(items.len());

        // ─── define 'decimals' ───
        let mut decimals: Vec<Option<i32>>= Vec::with_capacity(items.len());

        // ─── define 'supply' ───
        let mut supply: Vec<Option<i64>> = Vec::with_capacity(items.len());

        // ─── define 'initbase' ───
        let mut initbase: Vec<Option<i64>>= Vec::with_capacity(items.len());

        // ─── define 'initquote' ───
        let mut initquote: Vec<Option<i64>>=Vec::with_capacity(items.len());

        // ─── define 'txsinc' ───
        let mut txsinc: Vec<i64> = Vec::with_capacity(items.len());

        // ─── proceed 'for' ───
        for it in items.into_iter() {
            uuids.push(it.uuid);
            price.push(it.price);
            lastbase.push(it.lastbase);
            lastquote.push(it.lastquote);
            decimals.push(it.decimals);
            supply.push(it.supply);
            initbase.push(it.initbase);
            initquote.push(it.initquote);
            txsinc.push(it.txsinc);
        }

        // ─── define 'q' ───
        let q = r#"WITH v AS (SELECT u.uuid, u.price, u.lastbase, u.lastquote, u.decimals, u.supply, u.initbase, u.initquote,
            u.txsinc FROM UNNEST($1::text[], $2::double precision[], $3::bigint[], $4::bigint[], $5::integer[], $6::bigint[],
            $7::bigint[], $8::bigint[], $9::bigint[]) AS u(uuid, price, lastbase, lastquote, decimals, supply, initbase, initquote, txsinc))
            UPDATE tokens AS t SET price = COALESCE(v.price, t.price), lastbase = COALESCE(v.lastbase, t.lastbase),
            lastquote = COALESCE(v.lastquote, t.lastquote), decimals = COALESCE(t.decimals, v.decimals), supply = COALESCE(t.supply, v.supply),
            initbase = COALESCE(t.initbase, v.initbase), initquote = COALESCE(t.initquote,v.initquote), txs = COALESCE(t.txs, 0) + COALESCE(v.txsinc, 0)
            FROM v WHERE t.uuid = v.uuid"#;

        // ─── compare 'sqlx::query()' ───
        if let Err(e) = sqlx::query(q).bind(&uuids).bind(&price).bind(&lastbase).bind(&lastquote).bind(&decimals)
            .bind(&supply).bind(&initbase).bind(&initquote).bind(&txsinc).execute(&mut **conn).await {
            eprintln!("tokenwriter: batch update error: {e}");
        }
    }

    // ─── fn 'tokenwriter' ───
    /// fn description
    fn tokenwriter(pool: Pool<Postgres>) -> mpsc::Sender<TokenUpdate> {

        // ─── define '(tx, rx)' ───
        let (tx, mut rx) = mpsc::channel::<TokenUpdate>(32768);

        // ─── proceed 'tokio' ───
        tokio::spawn(async move {

            // ─── define 'conn' ───
            let mut conn = match pool.acquire().await {
                Ok(mut c) => {

                    // ─── callback 'sqlx::query()' ───
                    let _ = sqlx::query("SET lock_timeout = '800ms'").execute(&mut *c).await;
                    let _ = sqlx::query("SET statement_timeout = '2s'").execute(&mut *c).await;
                    c
                },
                Err(e) => {
                    eprintln!("tokenwriter: cannot acquire dedicated conn: {e}");
                    return;
                }
            };

            // ─── define 'pending' ───
            let mut pending: HashMap<String, TokenUpdate> = HashMap::with_capacity(4096);

            // ─── define 'lastflush' ───
            let mut lastflush = Instant::now();

            // ─── proceed 'loop' ───
            loop {

                // ─── define 'drained' ───
                let mut drained = 0usize;

                // ─── proceed 'while' ───
                while let Ok(msg) = rx.try_recv() {

                    // ─── define 'entry' ───
                    let entry = pending.entry(msg.uuid.clone()).or_insert(TokenUpdate {
                        uuid: msg.uuid.clone(),
                        price: None, lastbase: None, lastquote: None,
                        decimals: None, supply: None,
                        initbase: None, initquote: None,
                        txsinc: 0
                    });

                    // ─── compare 'msg.price' ───
                    if let Some(v) = msg.price {
                        entry.price = Some(v);
                    }

                    // ─── compare 'msg.lastbase' ───
                    if let Some(v) = msg.lastbase {
                        entry.lastbase = Some(v);
                    }

                    // ─── compare 'msg.lastquote' ───
                    if let Some(v) = msg.lastquote {
                        entry.lastquote = Some(v);
                    }

                    // ─── compare 'msg.decimals' ───
                    if let Some(v) = msg.decimals {
                        entry.decimals = Some(v);
                    }

                    // ─── compare 'msg.supply' ───
                    if let Some(v) = msg.supply {
                        entry.supply = Some(v);
                    }

                    // ─── compare 'msg.initbase' ───
                    if let Some(v) = msg.initbase {
                        entry.initbase = Some(v);
                    }

                    // ─── compare 'msg.initquote' ───
                    if let Some(v) = msg.initquote {
                        entry.initquote = Some(v);
                    }

                    // ─── push 'entry.txsinc' ───
                    entry.txsinc = entry.txsinc.saturating_add(msg.txsinc);

                    // ─── increment 'drained' ───
                    drained += 1;

                    // ─── compare 'POSTGRESTOKENSMAXBATCH' ───
                    if pending.len() >= POSTGRESTOKENSMAXBATCH {
                        break;
                    }
                }

                // ─── compare 'drained' ───
                if drained == 0 {

                    // ─── proceed 'tokio' ───
                    tokio::select! {
                        Some(msg) = rx.recv() => {

                            // ─── define 'entry' ───
                            let entry = pending.entry(msg.uuid.clone()).or_insert(TokenUpdate {
                                uuid: msg.uuid.clone(),
                                price: None, lastbase: None, lastquote: None,
                                decimals: None, supply: None,
                                initbase: None, initquote: None,
                                txsinc: 0,
                            });

                            // ─── compare 'msg.price' ───
                            if let Some(v) = msg.price {
                                entry.price = Some(v);
                            }

                            // ─── compare 'msg.lastbase' ───
                            if let Some(v) = msg.lastbase {
                                entry.lastbase = Some(v);
                            }

                            // ─── compare 'msg.lastquote' ───
                            if let Some(v) = msg.lastquote {
                                entry.lastquote = Some(v);
                            }

                            // ─── compare 'msg.decimals' ───
                            if let Some(v) = msg.decimals {
                                entry.decimals = Some(v);
                            }

                            // ─── compare 'msg.supply' ───
                            if let Some(v) = msg.supply {
                                entry.supply = Some(v);
                            }

                            // ─── compare 'msg.initbase' ───
                            if let Some(v) = msg.initbase {
                                entry.initbase = Some(v);
                            }

                            // ─── compare 'msg.initquote' ───
                            if let Some(v) = msg.initquote {
                                entry.initquote = Some(v);
                            }

                            // ─── push 'entry.txsinc' ───
                            entry.txsinc = entry.txsinc.saturating_add(msg.txsinc);
                        }

                        // ─── callback 'tokio::time::sleep()' ───
                        _ = tokio::time::sleep(Duration::from_millis(POSTGRESTOKENSFLUSHMS)) => {}
                    }
                }

                // ─── compare 'POSTGRESTOKENSMAXBATCH' ───
                if pending.len() >= (POSTGRESTOKENSMAXBATCH / 2) || lastflush.elapsed() >= Duration::from_millis(POSTGRESTOKENSFLUSHMS) {

                    // ─── callback 'Self::tokensflush()' ───
                    Self::tokensflush(&mut conn, &mut pending).await;
                    lastflush = Instant::now();
                }
            }
        });

        // ─── return 'tx' ───
        tx
    }

    // ─── fn 'tokenhandler' ───
    /// fn description
    async fn tokenhandler(storage: Arc<Storage>, row: TokenRow, tsms: i64, decimals: u8, price: f64) {

        // ─── define 'baseline' ───
        let baseline = storage.walletbalance().await.ok().flatten().unwrap_or(0.0);

        // ─── define '_permit' ───
        let _permit = POSTGRESSEMAPHORE.acquire().await.expect("semaphore");

        // ─── define 'monitor' ───
        let monitor = TradeMonitor::new(Arc::clone(&storage), baseline);

        // ─── define 'botconf' ───
        let botconf = Arc::new(TradeConfig::loadconfig(PATHCONFIGBOT).unwrap());

        // ─── define 'walletconf' ───
        let walletconf = Arc::new(WalletConfig::loadconfig(PATHCONFIGWALLET).unwrap());

        // ─── callback 'monitor.handlerorder()' ───
        let _ = monitor.handlerorder(row.mint.parse().unwrap_or_default(), row.program.parse().unwrap_or_default(),
            row.slot, tsms, decimals, price, botconf, walletconf).await;
    }

    // ─── fn 'tokenadvance' ───
    /// fn description
    async fn tokenadvance(&self, row: TokenRow) -> Result<(), sqlx::Error> {

        // ─── define 'servtime' ───
        let servtime = chrono::Utc::now().timestamp_millis();

        // ─── define 'tokenage' ───
        let tokenage = servtime - row.blocktime;

        // ─── define 'res' ───
        let res = sqlx::query(r#"INSERT INTO tokens (uuid, signature, slot, blocktime, program, mint,
            creator, pool, basevault, quotevault, servtime, tokenage) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT(signature) DO NOTHING"#)
            .bind(&row.uuid)
            .bind(&row.signature)
            .bind(row.slot)
            .bind(row.blocktime)
            .bind(&row.program)
            .bind(&row.mint)
            .bind(&row.creator)
            .bind(&row.pool)
            .bind(&row.basevault)
            .bind(&row.quotevault)
            .bind(servtime)
            .bind(tokenage)
            .execute(&self.writepool)
            .await?;

        // ─── compare 'res.rows_affected()' ───
        if res.rows_affected() == 1 {

            // ─── compare 'POSTGRESTOKENSHOTCACHECAP' ───
            if POSTGRESTOKENSCACHE.len() >= POSTGRESTOKENSHOTCACHECAP {

                // ─── define 'removed' ───
                let mut removed = 0usize;

                // ─── proceed 'for' ───
                for k in POSTGRESTOKENSCACHE.iter().take(5000) {
                    POSTGRESTOKENSCACHE.remove(k.key());
                    removed += 1;

                    // ─── compare 'removed' ───
                    if removed >= 5000 {
                        break;
                    }
                }
            }

            // ─── callback 'POSTGRESTOKENSCACHE.insert()' ───
            POSTGRESTOKENSCACHE.insert((row.mint.clone(), row.program.clone()), row.clone());

            // ─── define 'storage' ───
            let storage = Arc::new(self.clone());

            // ─── define 'rows' ───
            let rows = row.clone();

            // ─── proceed 'tokio' ───
            tokio::spawn(async move {

                // ─── compare 'Scanner::tokenenrich()' ───
                if let Ok(e) = Scanner::tokenenrich(&rows).await {

                    // ─── callback 'storage.tokentx.send()' ───
                    let _ = storage.tokentx.send(TokenUpdate {
                        uuid: rows.uuid.clone(),
                        price: Some(e.price),
                        lastbase: Some(e.initbase),
                        lastquote: Some(e.initquote),
                        decimals: Some(e.decimals as i32),
                        supply: Some(e.supply),
                        initbase: Some(e.initbase),
                        initquote: Some(e.initquote),
                        txsinc: 0,
                    }).await;

                    // ─── callback 'Storage::tokenhandler()' ───
                    Storage::tokenhandler(Arc::clone(&storage), rows.clone(), servtime, e.decimals, e.price).await;
                }
            });
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tokenselect' ───
    /// fn description
    async fn tokenselect(pool: &Pool<Postgres>, mint: &str, program: &str) -> Result<Option<TokenRow>, sqlx::Error> {

        // ─── compare 'POSTGRESTOKENSCACHE' ───
        if let Some(v) = POSTGRESTOKENSCACHE.get(&(mint.to_string(), program.to_string())) {

            // ─── return 'Result' ───
            return Ok(Some(v.clone()));
        }

        // ─── define 'r' ───
        let r: Option<(String, String, i64, i64, String, String, String, String, String, String)> =
            sqlx::query_as(r#"SELECT uuid, signature, slot, blocktime, program, mint, creator,
                pool, basevault, quotevault FROM tokens WHERE mint = $1 AND program = $2 ORDER BY servtime DESC LIMIT 1"#)
                .bind(mint)
                .bind(program)
                .fetch_optional(pool)
                .await?;

        // ─── compare 'r' ───
        if let Some((uuid, signature, slot, blocktime, program, mint, creator, pool, basevault, quotevault)) = r {

            // ─── define 'row' ───
            let row = TokenRow {
                uuid,
                signature,
                slot,
                blocktime,
                program,
                mint,
                creator,
                pool,
                basevault,
                quotevault,
            };

            // ─── callback 'POSTGRESTOKENSCACHE.insert()' ───
            POSTGRESTOKENSCACHE.insert((row.mint.clone(), row.program.clone()), row.clone());

            // ─── return 'Result' ───
            Ok(Some(row))
        } else {

            // ─── return 'Result' ───
            Ok(None)
        }
    }

    // ─── fn 'tokenfetchmint' ───
    /// fn description
    async fn tokenfetchmint(pool: &Pool<Postgres>, mint: &str, program: &str) -> Result<Option<TokenRow>, sqlx::Error> {

        // ─── callback 'Self::tokenselect()' ───
        Self::tokenselect(pool, mint, program).await
    }

    // ─── fn 'tokenenqueue' ───
    /// fn description
    async fn tokenenqueue(&self, uuid: &str, price: Option<f64>, lastbase: Option<i64>, lastquote: Option<i64>,
        decimals: Option<u8>, supply: Option<i64>, txsinc: i64, initlast: bool) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokentx.send()' ───
        let _ = self.tokentx.send(TokenUpdate {
            uuid: uuid.to_string(),
            price,
            lastbase,
            lastquote,
            decimals: decimals.map(|d| d as i32),
            supply,
            initbase: if initlast { lastbase } else { None },
            initquote: if initlast { lastquote } else { None },
            txsinc,
        }).await;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tokenchange' ───
    /// fn description
    async fn tokenchange(&self, mint: String, program: String, txsinc: i64) -> Result<(), sqlx::Error> {

        // ─── define 'tradeopen' ───
        let tradeopen = POSTGRESOPENTRADE.get(&mint).map(|c| *c > 0).unwrap_or(false);

        // ─── compare 'tradeopen' ───
        if !tradeopen {

            // ─── return 'Result' ───
            return Ok(());
        }

        // ─── compare 'Self::tokenfetchmint()' ───
        if let Some(row) = Self::tokenfetchmint(&self.readpool, &mint, &program).await? {

            // ─── callback 'self.tokenenqueue()' ───
            self.tokenenqueue(&row.uuid, None, None, None, None, None, txsinc, false).await?;

            // ─── define 'now' ───
            let now = chrono::Utc::now().timestamp_millis();

            // ─── define 'key' ───
            let key = (mint.clone(), program.clone());

            // ─── define 'enrich' ───
            let mut enrich = false;

            // ─── compare 'POSTGRESLASTENRICH.get_mut()' ───
            if let Some(mut ent) = POSTGRESLASTENRICH.get_mut(&key) {

                // ─── define '(timelast, acc)' ───
                let (timelast, acc) = *ent;

                // ─── define 'newacc' ───
                let newacc = acc.saturating_add(txsinc);

                // ─── compare 'now' ───
                if now - timelast >= POSTGRESENRICHMINPERIOD || newacc >= POSTGRESENRICHTXSTHRESHOLD {
                    *ent = (now, 0);
                    enrich = true;
                } else {
                    *ent = (timelast, newacc);
                }
            } else {
                POSTGRESLASTENRICH.insert(key.clone(), (now, 0));
                enrich = true;
            }

            // ─── compare 'enrich' ───
            if enrich {

                // ─── define 'storage' ───
                let storage = Arc::new(self.clone());

                // ─── define 'rows' ───
                let rows = row.clone();

                // ─── proceed 'tokio' ───
                tokio::spawn(async move {

                    // ─── compare 'Scanner::tokenenrich()' ───
                    if let Ok(e) = Scanner::tokenenrich(&rows).await {

                        // ─── callback 'storage.tokentx.send()' ───
                        let _ = storage.tokentx.send(TokenUpdate {
                            uuid: rows.uuid.clone(),
                            price: Some(e.price),
                            lastbase: Some(e.initbase),
                            lastquote: Some(e.initquote),
                            decimals: Some(e.decimals as i32),
                            supply: Some(e.supply),
                            initbase: None,
                            initquote: None,
                            txsinc: 0
                        }).await;

                        // ─── callback 'Storage::tokenhandler()' ───
                        Storage::tokenhandler(std::sync::Arc::clone(&storage), rows.clone(), now, e.decimals, e.price).await;
                    }
                });
            }
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn tokeninsertbonk(&self, e: &BonkPoolCreateEvent) -> Result<(), sqlx::Error> {

        // ─── compare 'bonk_pubkeys::PROGRAM' ───
        if e.metadata.program_id.to_string() == bonk_pubkeys::PROGRAM.to_string()
            && e.quote_mint.to_string() == system_pubkeys::WRAPPER.to_string() {

            // ─── define 'row' ───
            let row = TokenRow {
                uuid: Uuid::new_v4().to_string(),
                signature: e.metadata.signature.to_string(),
                slot: e.metadata.slot as i64,
                blocktime: e.metadata.block_time_ms,
                program: e.metadata.program_id.to_string(),
                mint: e.base_mint.to_string(),
                creator: e.creator.to_string(),
                pool: e.pool_state.to_string(),
                basevault: e.base_vault.to_string(),
                quotevault: e.quote_vault.to_string(),
            };

            // ─── return 'self.tokenadvance()' ───
            return self.tokenadvance(row).await;
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn tokeninsertpumpfun(&self, e: &PumpFunCreateTokenEvent) -> Result<(), sqlx::Error> {

        // ─── compare 'pumpfun_pubkeys::PROGRAM' ───
        if e.metadata.program_id.to_string() == pumpfun_pubkeys::PROGRAM.to_string() {

            // ─── define 'fallback' ───
            let fallback = e.bonding_curve.to_string();

            // ─── define 'row' ───
            let row = TokenRow {
                uuid: Uuid::new_v4().to_string(),
                signature: e.metadata.signature.to_string(),
                slot: e.metadata.slot as i64,
                blocktime: e.metadata.block_time_ms,
                program: e.metadata.program_id.to_string(),
                mint: e.mint.to_string(),
                creator: e.creator.to_string(),
                pool: e.bonding_curve.to_string(),
                basevault: fallback.clone(),
                quotevault: fallback.clone(),
            };

            // ─── return 'self.tokenadvance()' ───
            return self.tokenadvance(row).await;
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn tokeninsertpumpswap(&self, e: &PumpSwapCreatePoolEvent) -> Result<(), sqlx::Error> {

        // ─── compare 'pumpswap_pubkeys::PROGRAM' ───
        if e.metadata.program_id.to_string() == pumpswap_pubkeys::PROGRAM.to_string()
            && e.base_mint.to_string() == system_pubkeys::WRAPPER.to_string() {

            // ─── define 'row' ───
            let row = TokenRow {
                uuid: Uuid::new_v4().to_string(),
                signature: e.metadata.signature.to_string(),
                slot: e.metadata.slot as i64,
                blocktime: e.metadata.block_time_ms,
                program: e.metadata.program_id.to_string(),
                mint: e.quote_mint.to_string(),
                creator: e.creator.to_string(),
                pool: e.pool.to_string(),
                basevault: e.pool_base_token_account.to_string(),
                quotevault: e.pool_quote_token_account.to_string(),
            };

            // ─── return 'self.tokenadvance()' ───
            return self.tokenadvance(row).await;
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn tokeninsertraydiumamm(&self, e: &RaydiumAmmV4Initialize2Event) -> Result<(), sqlx::Error> {

        // ─── compare 'raydiumamm_pubkeys::PROGRAM' ───
        if e.metadata.program_id.to_string() == raydiumamm_pubkeys::PROGRAM.to_string()
            && e.pc_mint.to_string() == system_pubkeys::WRAPPER.to_string() {

            // ─── define 'row' ───
            let row = TokenRow {
                uuid: Uuid::new_v4().to_string(),
                signature: e.metadata.signature.to_string(),
                slot: e.metadata.slot as i64,
                blocktime: e.metadata.block_time_ms,
                program: e.metadata.program_id.to_string(),
                mint: e.coin_mint.to_string(),
                creator: e.user_wallet.to_string(),
                pool: e.amm.to_string(),
                basevault: e.pool_pc_token_account.to_string(),
                quotevault: e.pool_coin_token_account.to_string(),
            };

            // ─── return 'self.tokenadvance()' ───
            return self.tokenadvance(row).await;
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn tokeninsertraydiumclmm(&self, e: &RaydiumClmmCreatePoolEvent) -> Result<(), sqlx::Error> {

        // ─── compare 'raydiumclmm_pubkeys::PROGRAM' ───
        if e.metadata.program_id.to_string() == raydiumclmm_pubkeys::PROGRAM.to_string()
            && e.token_mint0.to_string() == system_pubkeys::WRAPPER.to_string() {

            // ─── define 'row' ───
            let row = TokenRow {
                uuid: Uuid::new_v4().to_string(),
                signature: e.metadata.signature.to_string(),
                slot: e.metadata.slot as i64,
                blocktime: e.metadata.block_time_ms,
                program: e.metadata.program_id.to_string(),
                mint: e.token_mint1.to_string(),
                creator: e.pool_creator.to_string(),
                pool: e.pool_state.to_string(),
                basevault: e.token_vault0.to_string(),
                quotevault: e.token_vault1.to_string(),
            };

            // ─── return 'self.tokenadvance()' ───
            return self.tokenadvance(row).await;
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'new' ───
    /// fn description
    pub async fn tokeninsertraydiumcpmm(&self, e: &RaydiumCpmmInitializeEvent) -> Result<(), sqlx::Error> {

        // ─── compare 'raydiumcpmm_pubkeys::PROGRAM' ───
        if e.metadata.program_id.to_string() == raydiumcpmm_pubkeys::PROGRAM.to_string()
            && e.token0_mint.to_string() == system_pubkeys::WRAPPER.to_string() {

            // ─── define 'row' ───
            let row = TokenRow {
                uuid: Uuid::new_v4().to_string(),
                signature: e.metadata.signature.to_string(),
                slot: e.metadata.slot as i64,
                blocktime: e.metadata.block_time_ms,
                program: e.metadata.program_id.to_string(),
                mint: e.token1_mint.to_string(),
                creator: e.creator.to_string(),
                pool: e.pool_state.to_string(),
                basevault: e.token0_vault.to_string(),
                quotevault: e.token1_vault.to_string(),
            };

            // ─── return 'self.tokenadvance()' ───
            return self.tokenadvance(row).await;
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tokenliquidity' ───
    /// fn description
    pub async fn tokenliquidity(&self, mint: &Pubkey) -> sqlx::Result<Option<(Option<f64>, Option<f64>, Option<f64>, Option<f64>)>> {

        // ─── define 'row' ───
        let row: Option<(Option<f64>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as("SELECT CAST(initbase AS DOUBLE PRECISION),
            CAST(initquote AS DOUBLE PRECISION), CAST(lastbase AS DOUBLE PRECISION), CAST(lastquote AS DOUBLE PRECISION)
             FROM tokens WHERE mint = $1")
            .bind(mint.to_string())
            .fetch_optional(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(row)
    }

    // ─── fn 'tokenpools' ───
    /// fn description
    pub async fn tokenpools(&self, mint: &Pubkey) -> sqlx::Result<Option<(Option<f64>, Option<f64>)>> {

        // ─── define 'row' ───
        let row: Option<(Option<f64>, Option<f64>)> = sqlx::query_as::<_, (Option<f64>, Option<f64>)>("SELECT CAST(initbase AS DOUBLE PRECISION)
            AS initbase, CAST(initquote AS DOUBLE PRECISION) AS initquote FROM tokens WHERE mint = $1")
            .bind(mint.to_string())
            .fetch_optional(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(row)
    }

    // ─── fn 'tokenprice' ───
    /// fn description
    pub async fn tokenprice(&self, mint: &Pubkey) -> sqlx::Result<Option<f64>> {

        // ─── define 'row' ───
        let row: Option<(Option<f64>,)> = sqlx::query_as("SELECT price FROM tokens WHERE mint = $1")
            .bind(mint.to_string())
            .fetch_optional(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(row.and_then(|(v,)| v))
    }

    // ─── fn 'tokenrefresh' ───
    /// fn description
    async fn tokenrefresh(tx: &mut Transaction<'_, Postgres>, uuid: &str, price: Option<f64>, lastbase: Option<i64>,
        lastquote: Option<i64>, decimals: Option<u8>, supply: Option<i64>, txsinc: i64) -> Result<(), sqlx::Error> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query(r#"UPDATE tokens SET price = COALESCE($2, price),
            lastbase = COALESCE($3, lastbase),
            lastquote = COALESCE($4, lastquote),
            decimals = COALESCE(decimals, $5),
            supply = COALESCE(supply, $6),
            initbase = COALESCE(initbase, $7),
            initquote = COALESCE(initquote, $8),
            txs = COALESCE(txs, 0) + $9
            WHERE uuid = $1"#)
            .bind(uuid)
            .bind(price)
            .bind(lastbase)
            .bind(lastquote)
            .bind(decimals.map(|d| d as i32))
            .bind(supply)
            .bind(lastbase)
            .bind(lastquote)
            .bind(txsinc)
            .execute(&mut **tx)
            .await?;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tokensupply' ───
    /// fn description
    pub async fn tokensupply(&self, mint: &Pubkey) -> sqlx::Result<Option<i64>> {

        // ─── define 'row' ───
        let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT supply FROM tokens WHERE mint = $1")
            .bind(mint.to_string())
            .fetch_optional(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(row.and_then(|(v,)| v))
    }

    // ─── fn 'tokensync' ───
    /// fn description
    pub async fn tokensync(tx: &mut Transaction<'_, Postgres>, uuid: &str, price: f64, initbase: i64,
        initquote: i64, decimals: u8, supply: i64) -> Result<(), sqlx::Error> {

        // ─── callback 'Self::tokenrefresh()' ───
        Self::tokenrefresh(tx, uuid, Some(price), Some(initbase), Some(initquote), Some(decimals), Some(supply), 0).await
    }

    // ─── fn 'tokentxs' ───
    /// fn description
    pub async fn tokentxs(&self, mint: &Pubkey) -> sqlx::Result<Option<i64>> {

        // ─── define 'row' ───
        let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT txs FROM tokens WHERE mint = $1")
            .bind(mint.to_string())
            .fetch_optional(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(row.and_then(|(v,)| v))
    }

    // ─── fn 'tokenupdatebonk' ───
    /// fn description
    pub async fn tokenupdatebonk(&self, e: &BonkTradeEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.base_token_mint.to_string(), bonk_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tokenupdatepumpfun' ───
    /// fn description
    pub async fn tokenupdatepumpfun(&self, e: &PumpFunTradeEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.mint.to_string(), pumpfun_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tokenupdatepumpswapbuy' ───
    /// fn description
    pub async fn tokenupdatepumpswapbuy(&self, e: &PumpSwapBuyEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.quote_mint.to_string(), pumpswap_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tokenupdatepumpswapsell' ───
    /// fn description
    pub async fn tokenupdatepumpswapsell(&self, e: &PumpSwapSellEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.quote_mint.to_string(), pumpswap_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tokenupdateraydiumamm' ───
    /// fn description
    pub async fn tokenupdateraydiumamm(&self, e: &RaydiumAmmV4AmmInfoAccountEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.pubkey.to_string(), raydiumamm_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tokenupdateraydiumclmm' ───
    /// fn description
    pub async fn tokenupdateraydiumclmm(&self, e: &RaydiumClmmPoolStateAccountEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.pubkey.to_string(), raydiumclmm_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tokenupdateraydiumcpmm' ───
    /// fn description
    pub async fn tokenupdateraydiumcpmm(&self, e: &RaydiumCpmmSwapEvent) -> Result<(), sqlx::Error> {

        // ─── callback 'self.tokenchange()' ───
        self.tokenchange(e.input_token_mint.to_string(), raydiumcpmm_pubkeys::PROGRAM.to_string(), 1).await
    }

    // ─── fn 'tradecache' ───
    /// fn description
    async fn tradecache(readpool: &Pool<Postgres>) -> sqlx::Result<()> {

        // ─── define 'rows' ───
        let rows: Vec<(String, i64)> = sqlx::query_as("SELECT mint, COUNT(*) AS cnt FROM trades WHERE total IS NULL GROUP BY mint")
            .fetch_all(readpool)
            .await?;

        // ─── proceed 'for' ───
        for (mint, cnt) in rows {

            // ─── compare 'cnt' ───
            if cnt > 0 {
                POSTGRESOPENTRADE.insert(mint, cnt as u32);
            }
        }

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tradeclose' ───
    /// fn description    
    pub async fn tradeclose(&self, uuid: &str, total: f64, exit: &str) -> sqlx::Result<()> {

        // ─── define 'row' ───
        let row = sqlx::query_as::<_, (String, f64, i64)>("SELECT mint, amount, servtime FROM trades WHERE uuid = $1")
            .bind(uuid)
            .fetch_optional(&self.readpool)
            .await?;

        // ─── compare 'row' ───
        if let Some((mint, amount, servtime)) = row {

            // ─── define 'profit' ───
            let profit = if amount != 0.0 {
                ((total - amount) / amount) * 100.0
            } else {
                0.0
            };

            // ─── define 'timenow' ───
            let timenow = chrono::Utc::now().timestamp_millis();

            // ─── define 'duration' ───
            let duration = timenow - servtime;

            // ─── callback 'sqlx::query()' ───
            sqlx::query("UPDATE trades SET profit = $1, total = $2, exit = $3, duration = $4 WHERE uuid = $5")
                .bind(profit)
                .bind(total)
                .bind(exit)
                .bind(duration)
                .bind(uuid)
                .execute(&self.writepool)
                .await?;

            // ─── compare 'POSTGRESOPENTRADE.get_mut()' ───
            if let Some(mut entry) = POSTGRESOPENTRADE.get_mut(&mint) {

                // ─── define 'countnew' ───
                let countnew = {

                    // ─── define 'c' ───
                    let c = entry.value_mut();

                    // ─── compare 'c' ───
                    if *c > 0 {
                        *c -= 1;
                    }
                    *c
                };

                // ─── callback 'drop()' ───
                drop(entry);

                // ─── compare 'countnew' ───
                if countnew == 0 {
                    POSTGRESOPENTRADE.remove(&mint);
                }
            }
        }

        Ok(())
    }

    // ─── fn 'tradescount' ───
    /// fn description
    pub async fn tradescount(&self) -> sqlx::Result<i64> {

        // ─── define 'cnt' ───
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM trades WHERE total IS NULL")
            .fetch_one(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(cnt)
    }

    // ─── fn 'tradecreate' ───
    /// fn description
    pub async fn tradecreate(&self, t: &TradeInfo) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("INSERT INTO trades (uuid, mint, amount, token, hash, program, slot, servtime, remamount, remtoken,
            realized, partialsell, trailcount, nextlevel) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)")
            .bind(&t.uuid)
            .bind(t.mint.to_string())
            .bind(t.amount)
            .bind(t.token)
            .bind(&t.hash)
            .bind(t.program.to_string())
            .bind(t.slot)
            .bind(t.servtime)
            .bind(t.amount)
            .bind(t.token)
            .bind(0.0_f64)
            .bind(0_i32)
            .bind(0_i64)
            .bind(0.0_f64)
            .execute(&self.writepool)
            .await?;

        // ─── define 'key' ───
        let key = t.mint.to_string();

        // ─── callback 'POSTGRESOPENTRADE.entry()' ───
        POSTGRESOPENTRADE.entry(key).and_modify(|c| *c = c.saturating_add(1)).or_insert(1);

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tradeexists' ───
    /// fn description
    pub async fn tradeexists(&self, mint: &Pubkey) -> sqlx::Result<bool> {

        // ─── define 'cnt' ───
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM trades WHERE mint = $1")
            .bind(mint.to_string())
            .fetch_one(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(cnt > 0)
    }

    // ─── fn 'tradepartialsell' ───
    /// fn description
    pub async fn tradepartialsell(&self, uuid: &str, proceeds: f64) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("UPDATE trades SET realized = COALESCE(realized, 0.0) + $1 WHERE uuid = $2 AND total IS NULL")
            .bind(proceeds)
            .bind(uuid)
            .execute(&self.writepool)
            .await?;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'traderealized' ───
    /// fn description
    pub async fn traderealized(&self, uuid: &str) -> sqlx::Result<f64> {

        // ─── define 'row' ───
        let row: Option<(Option<f64>,)> = sqlx::query_as("SELECT realized FROM trades WHERE uuid = $1")
            .bind(uuid)
            .fetch_optional(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(row.and_then(|(v,)| v).unwrap_or(0.0))
    }

    // ─── fn 'traderemaining' ───
    /// fn description
    pub async fn traderemaining(&self, uuid: &str, remamount: f64, remtoken: f64, partialsell: bool, trailcount: i64, nextlevel: f64) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("UPDATE trades SET remamount = $1, remtoken = $2, token = $3, partialsell = $4, trailcount = $5, nextlevel = $6
            WHERE uuid = $7 AND total IS NULL")
            .bind(remamount)
            .bind(remtoken)
            .bind(remtoken)
            .bind(if partialsell { 1_i32 } else { 0_i32 })
            .bind(trailcount)
            .bind(nextlevel)
            .bind(uuid)
            .execute(&self.writepool)
            .await?;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'tradestate' ───
    /// fn description
    pub async fn tradestate(&self, uuid: &str) -> sqlx::Result<Option<(f64, f64, bool, i64, f64, f64)>> {

        // ─── define 'row' ───
        let row: Option<(Option<f64>, Option<f64>, Option<i32>, Option<i64>, Option<f64>, Option<f64>)> =
            sqlx::query_as("SELECT remamount, remtoken, partialsell, trailcount, nextlevel, realized
             FROM trades WHERE uuid = $1 AND total IS NULL")
                .bind(uuid)
                .fetch_optional(&self.readpool)
                .await?;

        Ok(row.map(|(ra, rt, pt, pc, nl, rz)| {
            (ra.unwrap_or(0.0), rt.unwrap_or(0.0), pt.unwrap_or(0) != 0,
             pc.unwrap_or(0), nl.unwrap_or(0.0), rz.unwrap_or(0.0))
        }))
    }

    // ─── fn 'tradestotal' ───
    /// fn description
    pub async fn tradestotal(&self) -> sqlx::Result<i64> {

        // ─── define 'cnt' ───
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM trades WHERE total IS NOT NULL")
            .fetch_one(&self.readpool)
            .await?;

        // ─── return 'Result' ───
        Ok(cnt)
    }

    // ─── fn 'tradeupdate' ───
    /// fn description
    pub async fn tradeupdate(&self, uuid: &str, poolsize: f64, decimals: u8, txs: i64, spread: f64, supply: f64, latency: i64) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("UPDATE trades SET poolsize = $1, decimals = $2, txs = $3, spread = $4, supply = $5, latency = $6 WHERE uuid = $7")
            .bind(poolsize)
            .bind(decimals as i32)
            .bind(txs)
            .bind(spread)
            .bind(supply)
            .bind(latency)
            .bind(uuid)
            .execute(&self.writepool)
            .await?;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'signaturelog' ───
    /// fn description
    pub async fn signaturelog(&self, uuid: &str, mint: &Pubkey, signature: &str) -> sqlx::Result<()> {

        // ─── callback 'sqlx::query()' ───
        sqlx::query("INSERT INTO signature (uuid, mint, signature, servtime) VALUES ($1, $2, $3, $4)")
            .bind(uuid)
            .bind(mint.to_string())
            .bind(signature)
            .bind(chrono::Utc::now().timestamp_millis())
            .execute(&self.writepool)
            .await?;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'walletbalance' ───
    /// fn description
    pub async fn walletbalance(&self) -> sqlx::Result<Option<f64>> {

        // ─── define 'row' ───
        let row: Option<(f64,)> = sqlx::query_as("SELECT balance FROM wallet WHERE id = 1").fetch_optional(&self.readpool).await?;

        // ─── return 'Result' ───
        Ok(row.map(|(balance,)| balance))
    }

    // ─── fn 'walletupdate' ───
    /// fn description
    pub async fn walletupdate(&self, balance: f64) -> sqlx::Result<()> {

        // ─── define 'row' ───
        let row: Option<(Option<f64>,)> = sqlx::query_as("SELECT baseline FROM wallet WHERE id = 1")
            .fetch_optional(&self.readpool)
            .await?;

        // ─── define 'baseline' ───
        let baseline = row.and_then(|(b,)| b).unwrap_or(balance);

        // ─── define 'profit' ───
        let profit = if baseline != 0.0 {
            ((balance - baseline) / baseline) * 100.0
        } else {
            0.0
        };

        // ─── callback 'sqlx::query()' ───
        sqlx::query("INSERT INTO wallet (id, balance, profit, baseline) VALUES (1, $1, $2, $3)
            ON CONFLICT(id) DO UPDATE SET balance = EXCLUDED.balance, profit  = EXCLUDED.profit,
            baseline = COALESCE(wallet.baseline, EXCLUDED.baseline)")
            .bind(balance)
            .bind(profit)
            .bind(baseline)
            .execute(&self.writepool)
            .await?;

        // ─── return 'Result' ───
        Ok(())
    }
}