// ─── import packages ───
use solana_sdk::pubkey::Pubkey;
use solana_sdk::pubkey;

// ─── mod 'system_pubkeys' ───
/// mod description
pub mod system_pubkeys {
    use super::*;
    pub const WRAPPER: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
    pub const SYSTEM: Pubkey = pubkey!("11111111111111111111111111111111");
    pub const TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    pub const TOKEN2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
}

// ─── mod 'bonk_pubkeys' ───
/// mod description
pub mod bonk_pubkeys {
    use super::*;
    pub const PROGRAM: Pubkey = pubkey!("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj");
}

// ─── mod 'pumpfun_pubkeys' ───
/// mod description
pub mod pumpfun_pubkeys {
    use super::*;
    pub const PROGRAM: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
}

// ─── mod 'pumpswap_pubkeys' ───
/// mod description
pub mod pumpswap_pubkeys {
    use super::*;
    pub const PROGRAM: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
}

// ─── mod 'raydiumamm_pubkeys' ───
/// mod description
pub mod raydiumamm_pubkeys {
    use super::*;
    pub const PROGRAM: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
}

// ─── mod 'raydiumclmm_pubkeys' ───
/// mod description
pub mod raydiumclmm_pubkeys {
    use super::*;
    pub const PROGRAM: Pubkey = pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");
}

// ─── mod 'raydiumcpmm_pubkeys' ───
/// mod description
pub mod raydiumcpmm_pubkeys {
    use super::*;
    pub const PROGRAM: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
}