// ─── import packages ───
use once_cell::sync::Lazy;

// ─── const 'DATABASEUSER' ───
/// const description
pub const DATABASEUSER: &str = "xxxxxxxxxxxxxxx";

// ─── const 'DATABASEPASS' ───
/// const description
pub const DATABASEPASS: &str = "xxxxxxxxxxxxxxx";

// ─── const 'DATABASENAME' ───
/// const description
pub const DATABASENAME: &str = "xxxxxxxxxxxxxxx";

// ─── static 'DATABASEPATH' ───
/// const description
pub static DATABASEPATH: Lazy<String> = Lazy::new(|| {
    format!("postgres://{}:{}@127.0.0.1:6432/{}", DATABASEUSER, DATABASEPASS, DATABASENAME)
});