use super::{
    BlockchainApi, BlockchainApiResult, BlockchainTransactionFee, BlockchainTransactionSubmitted,
    TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY,
};
use crate::{
    core::ic_cdk::api::{id as station_canister_self_id, print},
    models::{Account, Metadata, Transfer},
};
use alloy::{
    consensus::SignableTransaction,
    eips::eip2718::Encodable2718,
    primitives::{hex, Address, TxKind},
    signers::k256::ecdsa,
};
use async_trait::async_trait;
use candid::Principal;
use evm_rpc_canister_types::{
    EthSepoliaService, MultiSendRawTransactionResult, RpcServices, SendRawTransactionResult,
    SendRawTransactionStatus, EVM_RPC,
};
use maplit::hashmap;
use num_bigint::BigUint;
use std::str::FromStr;

use ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    SignWithEcdsaArgument,
};

#[derive(Debug)]
pub struct Ethereum {
    station_canister_id: Principal,
    chain: alloy_chains::Chain,
}

pub enum EthereumNetwork {
    Mainnet,
    Sepolia,
}

impl Ethereum {
    pub fn create() -> Self {
        Self {
            station_canister_id: station_canister_self_id(),
            chain: alloy_chains::Chain::sepolia(),
        }
    }
}

#[async_trait]
impl BlockchainApi for Ethereum {
    async fn generate_address(&self, account: &Account) -> BlockchainApiResult<String> {
        let address = get_address_from_account(account).await;
        Ok(address)
    }

    async fn balance(&self, account: &Account) -> BlockchainApiResult<BigUint> {
        Ok(BigUint::from(123u32))
    }

    async fn decimals(&self, account: &Account) -> BlockchainApiResult<u32> {
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
        _transfer: &Transfer,
    ) -> BlockchainApiResult<BlockchainTransactionSubmitted> {
        let nonce = 0u64;
        let gas_limit = 100000u128;
        let max_fee_per_gas = 100u128;
        let max_priority_fee_per_gas = 100u128;

        let transaction = alloy::consensus::TxEip1559 {
            chain_id: self.chain.id(),
            nonce,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            to: TxKind::Call(
                Address::from_str(&_transfer.to_address)
                    .expect("failed to parse the destination address"),
            ),
            value: alloy::primitives::U256::from_be_slice(&_transfer.amount.0.to_bytes_be()),
            access_list: alloy::eips::eip2930::AccessList::default(),
            input: alloy::primitives::Bytes::default(),
        };

        let signature = {
            let (signature,) = sign_with_ecdsa(SignWithEcdsaArgument {
                message_hash: transaction.signature_hash().to_vec(),
                derivation_path: principal_to_derivation_path(&account),
                key_id: get_key_id(),
            })
            .await
            .expect("failed to sign transaction");

            let sig_bytes = signature.signature.as_slice();
            alloy::signers::Signature::try_from(sig_bytes).expect("failed to decode signature")
        };

        let tx_signed = transaction.into_signed(signature);
        let tx_envelope: alloy::consensus::TxEnvelope = tx_signed.into();
        let tx_encoded = tx_envelope.encoded_2718();

        let sent_tx_hash = send_raw_transaction(&self.chain, &tx_encoded).await;

        Ok(BlockchainTransactionSubmitted {
            details: vec![(
                TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY.to_owned(),
                sent_tx_hash,
            )],
        })
    }
}

async fn ecdsa_pubkey_of(account: &Account) -> Vec<u8> {
    let (key,) = ecdsa_public_key(EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: principal_to_derivation_path(&account),
        key_id: get_key_id(),
    })
    .await
    .expect("failed to get public key");
    key.public_key
}

async fn get_address_from_account(account: &Account) -> String {
    let public_key = ecdsa_pubkey_of(&account).await;
    let address = get_address_from_public_key(&public_key);
    hex::encode_prefixed(&address)
}

fn get_address_from_public_key(public_key: &[u8]) -> Address {
    let verifying_key = ecdsa::VerifyingKey::from_sec1_bytes(&public_key)
        .expect("Failed to create VerifyingKey from public key bytes");
    alloy::signers::utils::public_key_to_address(&verifying_key)
}

fn get_key_id() -> EcdsaKeyId {
    // TODO: check what we should use as a name
    let name: String = "dfx_test_key".to_string();

    EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name,
    }
}

fn principal_to_derivation_path(account: &Account) -> Vec<Vec<u8>> {
    let account_principal = Principal::from_slice(&account.id);
    const SCHEMA: u8 = 1;
    vec![vec![SCHEMA], account_principal.as_slice().to_vec()]
}

pub async fn send_raw_transaction(chain: &alloy_chains::Chain, raw_tx: &[u8]) -> String {
    let config = None;
    let services = hashmap! {
        alloy_chains::Chain::sepolia().id() => RpcServices::EthSepolia(Some(vec![EthSepoliaService::Alchemy])),
        // alloy_chains::Chain::mainnet().id() => RpcServices::EthMainnet(None), // TODO: support mainnet
    }.remove(&chain.id()).expect("chain not supported");

    let cycles = 10000000;

    let raw_tx_hex = hex::encode_prefixed(raw_tx);
    let status = match EVM_RPC
        .eth_send_raw_transaction(services, config, raw_tx_hex, cycles)
        .await
    {
        Ok((res,)) => match res {
            MultiSendRawTransactionResult::Consistent(status) => match status {
                SendRawTransactionResult::Ok(status) => status,
                SendRawTransactionResult::Err(e) => {
                    ic_cdk::trap(format!("Error: {:?}", e).as_str());
                }
            },
            MultiSendRawTransactionResult::Inconsistent(_) => {
                ic_cdk::trap("Status is inconsistent");
            }
        },
        Err(e) => ic_cdk::trap(format!("Error: {:?}", e).as_str()),
    };
    let tx_hash = match status {
        SendRawTransactionStatus::Ok(status) => status,
        error => {
            ic_cdk::trap(format!("Error: {:?}", error).as_str());
        }
    };
    tx_hash.expect("tx hash is none")
}
