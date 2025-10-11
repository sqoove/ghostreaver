// ─── const 'BASEDECIMALS' ───
/// const description
pub const BASEDECIMALS: u8 = 9;

// ─── const 'BONKBASEDEC' ───
/// const description
pub const BONKBASEDEC: usize = BONKDISC + 10;

// ─── const 'BONKDISC' ───
/// const description
pub const BONKDISC: usize = 8;

// ─── const 'BONKMINLEN' ───
/// const description
pub const BONKMINLEN: usize = BONKREALQUOTE + 8;

// ─── const 'BONKQUOTEDEC' ───
/// const description
pub const BONKQUOTEDEC: usize = BONKDISC + 11;

// ─── const 'BONKREALBASE' ───
/// const description
pub const BONKREALBASE: usize = BONKDISC + 45;

// ─── const 'BONKVIRTUALBASE' ───
/// const description
pub const BONKVIRTUALBASE: usize = BONKDISC + 29;

// ─── const 'BONKVIRTUALQUOTE' ───
/// const description
pub const BONKVIRTUALQUOTE: usize = BONKDISC + 37;

// ─── const 'DEFBATCHSIZE' ───
/// const description
pub const DEFBATCHSIZE: usize = 500;

// ─── const 'DEFBATCHTIMEOUT' ───
/// const description
pub const DEFBATCHTIMEOUT: u64 = 5;

// ─── const 'DEFCHANNELSIZE' ───
/// const description
pub const DEFCHANNELSIZE: usize = 100_000;

// ─── const 'DEFHPCHANNELSIZE' ───
/// const description
pub const DEFHPCHANNELSIZE: usize = 100_000;

// ─── const 'DEFLBCHANNELSIZE' ───
/// const description
pub const DEFLBCHANNELSIZE: usize = 200_000;

// ─── const 'DEFLLCHANNELSIZE' ───
/// const description
pub const DEFLLCHANNELSIZE: usize = 5_000;

// ─── const 'DEFMAXDECODINGSIZE' ───
/// const description
pub const DEFMAXDECODINGSIZE: usize = 1024 * 1024 * 128;

// ─── const 'DEFMETRICSPRINTINT' ───
/// const description
pub const DEFMETRICSPRINTINT: u64 = 10;

// ─── const 'DEFMETRICSWINSEC' ───
/// const description
pub const DEFMETRICSWINSEC: u64 = 5;

// ─── const 'DEFRETRYATTEMPTS' ───
/// const description
pub const DEFRETRYATTEMPTS: usize = 3;

// ─── const 'DEFRETRYWAITMS' ───
/// const description
pub const DEFRETRYWAITMS: u64 = 1;

// ─── const 'DEFTIMEOUTCONNECT' ───
/// const description
pub const DEFTIMEOUTCONNECT: u64 = 5;

// ─── const 'DEFTIMEOUTREQUEST' ───
/// const description
pub const DEFTIMEOUTREQUEST: u64 = 60;

// ─── const 'LAMPORTSPERSOL' ───
/// const description
pub const LAMPORTSPERSOL: f64 = 1_000_000_000.0;

// ─── const 'BONKREALQUOTE' ───
/// const description
pub const BONKREALQUOTE: usize = BONKDISC + 53;

// ─── const 'METADATAEVENTPOOLSIZE' ───
/// const description
pub const METADATAEVENTPOOLSIZE: usize = 1000;

// ─── const 'METADATATRANSFERPOOLSIZE' ───
/// const description
pub const METADATATRANSFERPOOLSIZE: usize = 2000;

// ─── const 'METRICSCHANNELBOUND' ───
/// const description
pub const METRICSCHANNELBOUND: usize = 100_000;

// ─── const 'METRICSFLUSHINT' ───
/// const description
pub const METRICSFLUSHINT: u64 = 500;

// ─── const 'PATHCONFIGBOT' ───
/// const description
pub const PATHCONFIGBOT: &str = "config/bot.yaml";

// ─── const 'PATHCONFIGENDPOINT' ───
/// const description
pub const PATHCONFIGENDPOINT: &str = "config/endpoint.yaml";

// ─── const 'PATHCONFIGWALLET' ───
/// const description
pub const PATHCONFIGWALLET: &str = "config/wallet.yaml";

// ─── const 'POSTGRESENRICHMINPERIOD' ───
/// const description
pub const POSTGRESENRICHMINPERIOD: i64 = 2_000;

// ─── const 'POSTGRESENRICHTXSTHRESHOLD' ───
/// const description
pub const POSTGRESENRICHTXSTHRESHOLD: i64 = 3;

// ─── const 'POSTGRESTICKCHANNEL' ───
/// const description
pub const POSTGRESTICKCHANNEL: usize = 10_000;

// ─── const 'POSTGRESTICKFLUSHMS' ───
/// const description
pub const POSTGRESTICKFLUSHMS: u64 = 20;

// ─── const 'POSTGRESTICKMAXBATCH' ───
/// const description
pub const POSTGRESTICKMAXBATCH: usize = 50_000;

// ─── const 'POSTGRESTOKENSFLUSHMS' ───
/// const description
pub const POSTGRESTOKENSFLUSHMS: u64 = 5;

// ─── const 'POSTGRESTOKENSHOTCACHECAP' ───
/// const description
pub const POSTGRESTOKENSHOTCACHECAP: usize = 50_000;

// ─── const 'POSTGRESTOKENSMAXBATCH' ───
/// const description
pub const POSTGRESTOKENSMAXBATCH: usize = 8_000;

// ─── const 'PROCMAXCONCURRENCYCAP' ───
/// const description
pub const PROCMAXCONCURRENCYCAP: usize = 256;

// ─── const 'RAYDIUMCLMMMINLEN' ───
/// const description
pub const RAYDIUMCLMMMINLEN: usize = 1536;

// ─── const 'RAYDIUMCLMMOFFSQRTPRICE' ───
/// const description
pub const RAYDIUMCLMMOFFSQRTPRICE: usize = 245;

// ─── const 'RAYDIUMCLMMOFFVAULTX' ───
/// const description
pub const RAYDIUMCLMMOFFVAULTX: usize = 129;

// ─── const 'RAYDIUMCLMMOFFVAULTY' ───
/// const description
pub const RAYDIUMCLMMOFFVAULTY: usize = 161;

// ─── const 'SCANNECALLTIMEOUT' ───
/// const description
pub const SCANNECALLTIMEOUT: u64 = 900;

// ─── const 'SCANNERATTEMPTS' ───
/// const description
pub const SCANNERATTEMPTS: usize = 5;

// ─── const 'SCANNERBASEDELAY' ───
/// const description
pub const SCANNERBASEDELAY: u64 = 60;

// ─── const 'SLOWPROCESSINGTHRESHOLD' ───
/// const description
pub const SLOWPROCESSINGTHRESHOLD: f64 = 20.0;