use candid::{CandidType, Deserialize};
use ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    EcdsaPublicKeyResponse, SignWithEcdsaArgument, SignWithEcdsaResponse,
};
use ic_cdk_macros::update;
use sha3::{Digest, Keccak256};
use ethabi::{Token, Function, Param, ParamType};
use hex;

mod evm_rpc;
use evm_rpc::{
    Block, BlockTag, EthMainnetService, EvmRpcCanister, GetBlockByNumberResult,
    MultiGetBlockByNumberResult, RpcServices,
};

#[derive(CandidType, Deserialize)]
struct EthereumAddress {
    address: String,
}

#[derive(CandidType, Deserialize)]
struct TransferRequest {
    to: String,
    amount: String,
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

#[update]
async fn transfer_bsc_testnet_token(request: TransferRequest) -> String {
    let from_address = get_ethereum_address().await.address;
    let to_address = request.to;
    let amount = u128::from_str_radix(&request.amount, 10).expect("Invalid amount");

    // BSC Testnet Chain ID
    let chain_id = 97;

    // Token contract address on BSC Testnet
    let token_address = "0x93C31Cc3fF99265B744CE64D76313eDF2A76D3E5";

    // Create the transaction data for a token transfer
    let data = create_erc20_transfer_data(&to_address, amount);

    // Get the nonce for the from_address
    let nonce = get_nonce(&from_address).await;

    // Create and sign the transaction
    let signed_tx = create_and_sign_transaction(
        from_address,
        token_address.to_string(),
        0, // value is 0 for token transfers
        data,
        chain_id,
        nonce,
    ).await;

    // Send the signed transaction
    let tx_hash = send_raw_transaction(signed_tx).await;

    format!("Transaction sent. Hash: {}", tx_hash)
}

fn key_id() -> EcdsaKeyId {
    EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: "key_1".to_string(), // use "key_1" for mainnet
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

fn create_erc20_transfer_data(to: &str, amount: u128) -> Vec<u8> {
    let function = Function {
        name: "transfer".to_string(),
        inputs: vec![
            Param { name: "to".to_string(), kind: ParamType::Address, internal_type: None },
            Param { name: "amount".to_string(), kind: ParamType::Uint(256), internal_type: None },
        ],
        outputs: vec![],
        constant: None,
        state_mutability: ethabi::StateMutability::NonPayable,
    };

    let to_address = ethabi::Address::from_slice(&hex::decode(to).unwrap());
    let params = vec![
        Token::Address(to_address),
        Token::Uint(amount.into()),
    ];

    function.encode_input(&params).unwrap()
}

async fn get_nonce(_address: &str) -> u64 {
    // Implement this function to get the nonce from the BSC testnet
    // Need to make an RPC call to the BSC testnet
    unimplemented!()
}

async fn create_and_sign_transaction(
    _from: String,
    _to: String,
    _value: u64,
    _data: Vec<u8>,
    _chain_id: u64,
    _nonce: u64,
) -> Vec<u8> {
    // Implement this function to create and sign the transaction
    // Need to use the ECDSA API to sign the transaction
    unimplemented!()
}

async fn send_raw_transaction(_signed_tx: Vec<u8>) -> String {
    // Implement this function to send the raw transaction to the BSC testnet
    // Need to make an RPC call to the BSC testnet
    unimplemented!()
}