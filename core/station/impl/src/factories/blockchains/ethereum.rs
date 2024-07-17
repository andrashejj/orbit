use super::{
    BlockchainApi, BlockchainApiResult, BlockchainTransactionFee, BlockchainTransactionSubmitted,
    TRANSACTION_SUBMITTED_DETAILS_BLOCK_HEIGHT_KEY,
    TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY,
};
use crate::{
    core::ic_cdk::api::{id as station_canister_self_id, print},
    errors::BlockchainApiError,
    mappers::HelperMapper,
    models::{
        Account, AccountId, Blockchain, BlockchainStandard, Metadata, Transfer, METADATA_MEMO_KEY,
    },
};
use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use candid::Principal;
use ic_ledger_types::{
    account_balance, query_blocks, transfer, AccountBalanceArgs, AccountIdentifier, GetBlocksArgs,
    Memo, QueryBlocksResponse, Subaccount, Timestamp, Tokens, Transaction, TransferArgs,
    TransferError as LedgerTransferError, DEFAULT_FEE,
};
use num_bigint::BigUint;
use orbit_essentials::{
    api::ApiError,
    cdk::{self},
};
use sha2::{Digest, Sha256};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};
use uuid::Uuid;

use ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    SignWithEcdsaArgument,
};

#[derive(Debug)]
pub struct Ethereum {
    station_canister_id: Principal,
}

pub enum EthereumNetwork {
    Mainnet,
    Sepolia,
}

impl Ethereum {
    pub fn create() -> Self {
        Self {
            station_canister_id: station_canister_self_id(),
        }
    }
}

#[async_trait]
impl BlockchainApi for Ethereum {
    async fn generate_address(&self, account: &Account) -> BlockchainApiResult<String> {
        // TODO: check what we should use as a Principal
        let account_principal = Principal::from_slice(&account.id);
        let public_key = ecdsa_pubkey_of(&account_principal).await;
        Ok(format!("0x{}", hex::encode(&public_key)))
    }

    async fn balance(&self, account: &Account) -> BlockchainApiResult<BigUint> {
        Ok(BigUint::from(0u32))
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
        "mainnet".to_string()
    }

    async fn submit_transaction(
        &self,
        _account: &Account,
        _transfer: &Transfer,
    ) -> BlockchainApiResult<BlockchainTransactionSubmitted> {
        Ok(BlockchainTransactionSubmitted {
            details: vec![
                (
                    TRANSACTION_SUBMITTED_DETAILS_BLOCK_HEIGHT_KEY.to_string(),
                    "0".to_string(),
                ),
                (
                    TRANSACTION_SUBMITTED_DETAILS_TRANSACTION_HASH_KEY.to_string(),
                    "0x1234".to_string(),
                ),
            ],
        })
    }
}

fn principal_to_derivation_path(p: &Principal) -> Vec<Vec<u8>> {
    const SCHEMA: u8 = 1;

    vec![vec![SCHEMA], p.as_slice().to_vec()]
}

/// Computes a signature for an [EIP-1559](https://eips.ethereum.org/EIPS/eip-1559) transaction.
// #[update(guard = "caller_is_not_anonymous")]
// async fn sign_transaction(req: SignRequest) -> String {
//     use ethers_core::types::transaction::eip1559::Eip1559TransactionRequest;
//     use ethers_core::types::Signature;

//     const EIP1559_TX_ID: u8 = 2;

//     let caller = ic_cdk::caller();

//     let data = req.data.as_ref().map(|s| decode_hex(s));

//     let tx = Eip1559TransactionRequest {
//         chain_id: Some(nat_to_u64(&req.chain_id)),
//         from: None,
//         to: Some(
//             Address::from_str(&req.to)
//                 .expect("failed to parse the destination address")
//                 .into(),
//         ),
//         gas: Some(nat_to_u256(&req.gas)),
//         value: Some(nat_to_u256(&req.value)),
//         nonce: Some(nat_to_u256(&req.nonce)),
//         data,
//         access_list: AccessList::default(),
//         max_priority_fee_per_gas: Some(nat_to_u256(&req.max_priority_fee_per_gas)),
//         max_fee_per_gas: Some(nat_to_u256(&req.max_fee_per_gas)),
//     };

//     let mut unsigned_tx_bytes = tx.rlp().to_vec();
//     unsigned_tx_bytes.insert(0, EIP1559_TX_ID);

//     let txhash = keccak256(&unsigned_tx_bytes);

//     let (pubkey, signature) = pubkey_and_signature(&caller, txhash.to_vec()).await;

//     let signature = Signature {
//         v: y_parity(&txhash, &signature, &pubkey),
//         r: U256::from_big_endian(&signature[0..32]),
//         s: U256::from_big_endian(&signature[32..64]),
//     };

//     let mut signed_tx_bytes = tx.rlp_signed(&signature).to_vec();
//     signed_tx_bytes.insert(0, EIP1559_TX_ID);

//     format!("0x{}", hex::encode(&signed_tx_bytes))
// }

/// Returns the public key and a message signature for the specified principal.
async fn pubkey_and_signature(caller: &Principal, message_hash: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    // Fetch the pubkey and the signature concurrently to reduce latency.
    let (pubkey, response) = futures::join!(
        ecdsa_pubkey_of(caller),
        sign_with_ecdsa(SignWithEcdsaArgument {
            message_hash,
            derivation_path: principal_to_derivation_path(caller),
            key_id: get_key_id(),
        })
    );
    (
        pubkey,
        response.expect("failed to sign the message").0.signature,
    )
}

async fn ecdsa_pubkey_of(principal: &Principal) -> Vec<u8> {
    let (key,) = ecdsa_public_key(EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: principal_to_derivation_path(principal),
        key_id: get_key_id(),
    })
    .await
    .expect("failed to get public key");
    key.public_key
}

fn get_key_id() -> EcdsaKeyId {
    // TODO: check what we should use as a name
    let name: String = "sample-key-name".to_string();

    EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name,
    }
}
