// ─── import packages ───
use std::{fs, error::Error};
use serde::Deserialize;

// ─── struct 'EndpointConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct EndpointConfig {
    pub geyser: String,
    pub rpc: String,
    pub xtoken: String,
}

// ─── struct 'ServerConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct ServerConfig {
    pub endpoint: EndpointConfig
}

// ─── impl 'ServerConfig' ───
/// impl description
impl ServerConfig {

    // ─── fn 'loadconfig' ───
    /// fn description
    pub fn loadconfig(file_path: &str) -> Result<Self, Box<dyn Error>> {

        // ─── define 'contents' ───
        let contents = fs::read_to_string(file_path)?;

        // ─── define 'config' ───
        let config: ServerConfig = serde_yaml::from_str(&contents)?;

        // ─── return 'Result' ───
        Ok(config)
    }
}

// ─── struct 'WalletKeys' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct WalletKeys {
    pub publicaddr: String,
    pub privatekey: String,
}

// ─── struct 'WalletConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct WalletConfig {
    pub wallet: WalletKeys,
}

// ─── impl 'WalletConfig' ───
/// impl description
impl WalletConfig {

    // ─── fn 'loadconfig' ───
    /// fn description
    pub fn loadconfig(file_path: &str) -> Result<Self, Box<dyn Error>> {

        // ─── define 'contents' ───
        let contents = fs::read_to_string(file_path)?;

        // ─── define 'config' ───
        let config: WalletConfig = serde_yaml::from_str(&contents)?;

        // ─── return 'Result' ───
        Ok(config)
    }
}

// ─── struct 'MainConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct MainConfig {
    pub status: bool,
    pub sandbox: bool,
    pub debug: bool,
    pub balance: f64,
    pub opentrades: u32,
    pub maxtrades: u32
}

// ─── struct 'MonitoringConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct MonitoringConfig {
    pub programs: String,
    pub retries: u32
}

// ─── struct 'OrdersConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct OrdersConfig {
    pub amount: f64,
    pub buyslippage: f64,
    pub sellslippage: f64,
    pub stoploss: f64,
    pub takeprofit: f64,
    pub partialtrigger: f64,
    pub partialsell: f64,
    pub trailingtrigger: f64,
    pub trailingsell: f64,
    pub trailingstop: f64,
    pub trailingdrop: f64,
    pub timeclose: i64,
    pub attempts: u64,
    pub dropmax: f64
}

// ─── struct 'PriorityConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct PriorityConfig {
    pub inputfees: f64,
    pub outputfees: f64
}

// ─── struct 'RulesConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct RulesConfig {
    pub maxtokenage: Option<u64>,
    pub bonkmaxtokens: Option<u64>,
    pub bonkexit: Option<u64>,
    pub pumpfunmaxtokens: Option<u64>,
    pub pumpfunexit: Option<u64>,
    pub pumpswapmaxtokens: Option<u64>,
    pub pumpswapexit: Option<u64>,
    pub raydiumammmaxtokens: Option<u64>,
    pub raydiumammexit: Option<u64>,
    pub raydiumclmmmaxtokens: Option<u64>,
    pub raydiumclmmexit: Option<u64>,
    pub raydiumcpmmmaxtokens: Option<u64>,
    pub raydiumcpmmexit: Option<u64>
}

// ─── struct 'BotConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct BotConfig {
    pub main: MainConfig,
    pub monitoring: MonitoringConfig,
    pub orders: OrdersConfig,
    pub priority: PriorityConfig,
    pub rules: RulesConfig
}

// ─── struct 'TradeConfig' ───
/// struct description
#[derive(Deserialize, Debug)]
pub struct TradeConfig {
    #[serde(flatten)]
    pub bot: BotConfig
}

// ─── impl 'TradeConfig' ───
/// impl description
impl TradeConfig {

    // ─── fn 'loadconfig' ───
    /// fn description
    pub fn loadconfig(file_path: &str) -> Result<Self, Box<dyn Error>> {

        // ─── define 'contents' ───
        let contents = fs::read_to_string(file_path)?;

        // ─── define 'config' ───
        let config: TradeConfig = serde_yaml::from_str(&contents)?;

        // ─── return 'Result' ───
        Ok(config)
    }
}