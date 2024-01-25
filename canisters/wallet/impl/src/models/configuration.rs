use super::{UserGroup, WalletAsset};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Configuration {
    /// The list of assets that are supported by the wallet canister (e.g. `ICP`, `BTC`, `ETH`, etc.)
    pub supported_assets: Vec<WalletAsset>,
    /// The list of available user groups.
    pub user_groups: Vec<UserGroup>,
}