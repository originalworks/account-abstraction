pub mod manager;

pub struct Config {
    pub use_kms: bool,
    pub private_key: Option<String>,
    pub signer_kms_id: Option<String>,
}

pub trait IntoSignerWalletConfig {
    fn into_signer_wallet_config(&self) -> Config;
}
