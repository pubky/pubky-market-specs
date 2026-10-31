// Crate version (synced with Cargo.toml at compile time)
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

// Path constants. Pubky Market records live under the `pubky.app` app
// directory, so these values are shared with the pubky-app social specs.
pub static PUBLIC_PATH: &str = "/pub/";
// Authenticated private storage: the homeserver refuses reads, listings, and
// writes from anyone but the owner's session. Records under this prefix are
// deliberately invisible to watchers/indexers (never wired into the URI
// parser's resource resolution).
pub static PRIVATE_PATH: &str = "/priv/";
pub static APP_PATH: &str = "pubky.app/";
pub static PROTOCOL: &str = "pubky://";
pub static MARKETPLACE_PATH: &str = "marketplace/v1/";
