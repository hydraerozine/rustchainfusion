use candid::{CandidType, Deserialize};
use ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    EcdsaPublicKeyResponse, SignWithEcdsaArgument, SignWithEcdsaResponse,
};
use ic_cdk_macros::update;
use sha3::{Digest, Keccak256};

mod evm_rpc;
use evm_rpc::{
    Block, BlockTag, EthMainnetService, EvmRpcCanister, GetBlockByNumberResult,
    MultiGetBlockByNumberResult, RpcServices,
};

#[derive(CandidType, Deserialize)]
struct EthereumAddress {
    address: String,
}

#[update]
async fn get_latest_ethereum_block() -> Block {
    let rpc_providers = RpcServices::EthMainnet(Some(vec![EthMainnetService::Cloudflare]));

    let cycles = 10_000_000_000;
    let result = EvmRpcCanister::eth_get_block_by_number(rpc_providers, None, BlockTag::Latest, cycles)
        .await
        .expect("Call failed");

    match result {
        MultiGetBlockByNumberResult::Consistent(r) => match r {
            GetBlockByNumberResult::Ok(block) => block,
            GetBlockByNumberResult::Err(err) => panic!("{err:?}"),
        },
        MultiGetBlockByNumberResult::Inconsistent(_) => {
            panic!("RPC providers gave inconsistent results")
        }
    }
}

#[update]
async fn get_ecdsa_public_key() -> EcdsaPublicKeyResponse {
    let (pub_key,) = ecdsa_public_key(EcdsaPublicKeyArgument {
        key_id: key_id(),
        ..Default::default()
    })
    .await
    .expect("Failed to get public key");
    pub_key
}

#[update]
async fn sign_hash_with_ecdsa(message_hash: Vec<u8>) -> SignWithEcdsaResponse {
    let (signature,) = sign_with_ecdsa(SignWithEcdsaArgument {
        message_hash,
        key_id: key_id(),
        ..Default::default()
    })
    .await
    .expect("Failed to sign");
    signature
}

#[update]
async fn get_ethereum_address() -> EthereumAddress {
    let key_id = key_id();
    let derivation_path = vec![]; // Empty for root key
    let canister_id = ic_cdk::id();

    let public_key_result = ecdsa_public_key(EcdsaPublicKeyArgument {
        canister_id: Some(canister_id),
        derivation_path,
        key_id,
    })
    .await;

    let public_key = match public_key_result {
        Ok((response,)) => response.public_key,
        Err(e) => panic!("Failed to get public key: {:?}", e),
    };

    let ethereum_address = public_key_to_ethereum_address(&public_key);

    EthereumAddress {
        address: format!("0x{}", hex::encode(ethereum_address)),
    }
}

fn key_id() -> EcdsaKeyId {
    EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: "dfx_test_key".to_string(), // use "key_1" for mainnet
    }
}

fn public_key_to_ethereum_address(public_key: &[u8]) -> [u8; 20] {
    // Remove the first byte (0x04) which indicates uncompressed public key
    let public_key = &public_key[1..];
    
    // Compute Keccak-256 hash
    let mut hasher = Keccak256::new();
    hasher.update(public_key);
    let result = hasher.finalize();
    
    // Take the last 20 bytes as Ethereum address
    let mut address = [0u8; 20];
    address.copy_from_slice(&result[12..]);
    
    address
}