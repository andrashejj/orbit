use super::{
    nat_to_u256, BlockchainApi, BlockchainApiResult, BlockchainTransactionFee,
    BlockchainTransactionSubmitted, TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY,
};
use crate::factories::blockchains::ethereum::{
    get_address_from_account, request_evm_rpc, sign_and_send_transaction,
};
use crate::{
    core::ic_cdk::api::print,
    models::{Account, Metadata, Transfer},
};
use alloy::dyn_abi::DynSolValue;
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
    async fn get_balance_from_chain(&self, address: &Address) -> U256 {
        let deserialized = request_evm_rpc(
            &self.chain,
            "eth_call",
            serde_json::json!([
                {
                    "to": self.token_address.to_string(),
                    "data": ERC20_INTERFACE.encode_input("balanceOf", &[
                        DynSolValue::from(address.clone()),
                    ]).expect("failed to parse the input"),
                },
                "latest",
            ]),
        )
        .await;
        print(format!("erc20 balance deserialized: {:?}", deserialized));
        let balance_hex = deserialized
            .as_str()
            .expect("balance result is not a string");

        let balance = U256::from_str(balance_hex).expect("failed to decode balance hex");

        balance
    }
}

#[async_trait]
impl BlockchainApi for EthereumErc20 {
    async fn generate_address(&self, account: &Account) -> BlockchainApiResult<String> {
        let address = get_address_from_account(account).await;
        Ok(address.to_string())
    }

    async fn balance(&self, account: &Account) -> BlockchainApiResult<BigUint> {
        let address = get_address_from_account(account).await;
        let balance = self.get_balance_from_chain(&address).await;
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
        let nonce = 0u64;
        let gas_limit = 50000u128;
        let max_fee_per_gas: u128 = 40 * 10u128.pow(9); // gwei
        let max_priority_fee_per_gas = 100u128;

        let transaction = alloy::consensus::TxEip1559 {
            chain_id: self.chain.id(),
            nonce,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            to: TxKind::Call(self.token_address),
            value: U256::from(0),
            access_list: alloy::eips::eip2930::AccessList::default(),
            input: ERC20_INTERFACE
                .encode_input(
                    "transfer",
                    &[
                        DynSolValue::from(transfer.to_address.clone()),
                        DynSolValue::from(nat_to_u256(&transfer.amount)),
                    ],
                )
                .expect("failed to parse the input")
                .into(),
        };

        let sent_tx_hash = sign_and_send_transaction(&account, &self.chain, transaction).await;

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
