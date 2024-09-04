use super::{
    estimate_transaction_fee, eth_get_transaction_count, get_metadata_value, nat_to_u256,
    BlockchainApi, BlockchainApiResult, BlockchainTransactionFee, BlockchainTransactionSubmitted,
    METADATA_KEY_GAS_LIMIT, METADATA_KEY_MAX_FEE_PER_GAS, METADATA_KEY_MAX_PRIORITY_FEE_PER_GAS,
    TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY,
};
use crate::errors::BlockchainApiError;
use crate::factories::blockchains::ethereum::{
    get_address_from_account, request_evm_rpc, sign_and_send_transaction,
};
use crate::{
    core::ic_cdk::api::print,
    models::{Account, Metadata, Transfer},
};
use alloy::dyn_abi::DynSolValue;
use alloy::hex::FromHex;
use alloy::{
    contract::Interface,
    primitives::{Address, TxKind, U256},
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use num_bigint::BigUint;
use std::str::FromStr;

#[derive(Debug)]
pub struct EthereumErc20 {
    chain: alloy_chains::Chain,
    token_address: Address,
}

impl EthereumErc20 {
    pub fn create(token_address: Address) -> Self {
        Self {
            chain: alloy_chains::Chain::sepolia(),
            token_address,
        }
    }
}

impl EthereumErc20 {
    async fn get_balance_from_chain(&self, address: &str) -> Result<U256, BlockchainApiError> {
        let address =
            Address::from_hex(address).map_err(|_e| BlockchainApiError::FetchBalanceFailed {
                account_id: address.to_string(),
            })?;
        let deserialized = request_evm_rpc(
            &self.chain,
            "eth_call",
            serde_json::json!([
                {
                    "to": self.token_address.to_string(),
                    "data": ERC20_INTERFACE.encode_input("balanceOf", &[
                        DynSolValue::from(address),
                    ]).map_err(|_e| BlockchainApiError::FetchBalanceFailed {
                        account_id: address.to_string(),
                    })?,
                },
                "latest",
            ]),
        )
        .await?;
        print(format!("erc20 balance deserialized: {:?}", deserialized));
        let balance_hex =
            deserialized
                .as_str()
                .ok_or_else(|| BlockchainApiError::FetchBalanceFailed {
                    account_id: address.to_string(),
                })?;

        let balance =
            U256::from_str(balance_hex).map_err(|_| BlockchainApiError::FetchBalanceFailed {
                account_id: address.to_string(),
            })?;

        Ok(balance)
    }

    async fn estimate_transaction_fee(
        &self,
        to_address: &str,
        data: &alloy::primitives::Bytes,
        value: U256,
    ) -> BlockchainApiResult<BlockchainTransactionFee> {
        estimate_transaction_fee(&self.chain, to_address, data, value).await
    }
}

#[async_trait]
impl BlockchainApi for EthereumErc20 {
    async fn generate_address(&self, account: &Account) -> BlockchainApiResult<String> {
        let address = get_address_from_account(account).await?;
        Ok(address)
    }

    async fn balance(&self, account: &Account) -> BlockchainApiResult<BigUint> {
        let balance = self.get_balance_from_chain(&account.address).await?;
        Ok(BigUint::from_bytes_be(&balance.to_be_bytes_vec()))
    }

    async fn decimals(&self, _account: &Account) -> BlockchainApiResult<u32> {
        Ok(18)
    }

    async fn transaction_fee(
        &self,
        _account: &Account,
    ) -> BlockchainApiResult<BlockchainTransactionFee> {
        Ok(BlockchainTransactionFee {
            fee: BigUint::from(0u32),
            metadata: Metadata::default(),
        })
    }

    fn default_network(&self) -> String {
        alloy_chains::Chain::mainnet().to_string()
    }

    async fn submit_transaction(
        &self,
        account: &Account,
        transfer: &Transfer,
    ) -> BlockchainApiResult<BlockchainTransactionSubmitted> {
        let nonce = eth_get_transaction_count(&self.chain, &account.address).await?;
        let value = U256::from(0);
        let to_address = self.token_address;

        let data = ERC20_INTERFACE
            .encode_input(
                "transfer",
                &[
                    DynSolValue::from(transfer.to_address.clone()),
                    DynSolValue::from(nat_to_u256(&transfer.amount)),
                ],
            )
            .map_err(|e| BlockchainApiError::TransactionSubmitFailed {
                info: e.to_string(),
            })?
            .into();
        let fee = self
            .estimate_transaction_fee(&to_address.to_string(), &data, value)
            .await?;
        let gas_limit = get_metadata_value::<u128>(&fee.metadata, METADATA_KEY_GAS_LIMIT)?;
        let max_fee_per_gas =
            get_metadata_value::<u128>(&fee.metadata, METADATA_KEY_MAX_FEE_PER_GAS)?;
        let max_priority_fee_per_gas =
            get_metadata_value::<u128>(&fee.metadata, METADATA_KEY_MAX_PRIORITY_FEE_PER_GAS)?;

        let transaction = alloy::consensus::TxEip1559 {
            chain_id: self.chain.id(),
            nonce,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            to: TxKind::Call(to_address),
            value,
            access_list: alloy::eips::eip2930::AccessList::default(),
            input: data,
        };

        let sent_tx_hash = sign_and_send_transaction(&account, &self.chain, transaction).await?;

        Ok(BlockchainTransactionSubmitted {
            details: vec![(
                TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY.to_owned(),
                sent_tx_hash,
            )],
        })
    }
}

lazy_static! {
    static ref ERC20_INTERFACE: Interface = Interface::new(
        serde_json::from_str(include_str!("./Erc20Abi.json"))
            .expect("failed to parse the erc20 abi"),
    );
}
