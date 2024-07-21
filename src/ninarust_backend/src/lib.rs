use candid::{CandidType, Deserialize, Nat, Principal};
use ic_cdk::api::call::call_with_payment128;
use ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    SignWithEcdsaArgument,
};
use ic_cdk_macros::*;
use sha3::{Digest, Keccak256};
use ethabi::{Token, Function, Param, ParamType};
use hex;
use rlp::RlpStream;
use num_traits::cast::ToPrimitive;

const CIT_TOKEN_ADDRESS: &str = "0x93C31Cc3fF99265B744CE64D76313eDF2A76D3E5";
const BSC_TESTNET_CHAIN_ID: u64 = 97;
const EVM_RPC_CANISTER_ID: &str = "7hfb6-caaaa-aaaar-qadga-cai";

#[derive(CandidType, Deserialize, Debug)]
enum RpcServices {
    BscTestnet(Option<Vec<BscTestnetService>>),
}

#[derive(CandidType, Deserialize, Debug)]
enum BscTestnetService {
    Default,
}

#[derive(CandidType, Deserialize, Debug)]
enum BlockTag {
    Latest,
}

#[derive(CandidType, Deserialize, Debug)]
struct GetTransactionCountArgs {
    address: String,
    block: BlockTag,
}

#[derive(CandidType, Deserialize, Debug)]
enum MultiGetTransactionCountResult {
    Consistent(Result<Nat, String>),
    Inconsistent(Vec<(BscTestnetService, Result<Nat, String>)>),
}

#[derive(CandidType, Deserialize, Debug)]
struct FeeHistoryArgs {
    block_count: u64,
    newest_block: BlockTag,
    reward_percentiles: Option<Vec<f64>>,
}

#[derive(CandidType, Deserialize, Debug)]
struct FeeHistory {
    base_fee_per_gas: Vec<Nat>,
    gas_used_ratio: Vec<f64>,
    oldest_block: Nat,
    reward: Vec<Vec<Nat>>,
}

#[derive(CandidType, Deserialize, Debug)]
enum MultiFeeHistoryResult {
    Consistent(Result<FeeHistory, String>),
    Inconsistent(Vec<(BscTestnetService, Result<FeeHistory, String>)>),
}

#[derive(CandidType, Deserialize, Debug)]
enum MultiSendRawTransactionResult {
    Consistent(Result<String, String>),
    Inconsistent(Vec<(BscTestnetService, Result<String, String>)>),
}

#[derive(CandidType, Deserialize)]
struct EthereumAddress {
    address: String,
}

#[derive(CandidType, Deserialize)]
struct TransferRequest {
    to: String,
    amount: String,
}

#[derive(CandidType, Deserialize)]
struct SignRequest {
    to: String,
    value: Nat,
    gas: Nat,
    gas_price: Nat,
    nonce: Nat,
    data: Vec<u8>,
}

#[update]
async fn transfer_bsc_testnet_token(request: TransferRequest) -> Result<String, String> {
    let from_address = get_ethereum_address().await.address;
    let to_address = request.to;
    let amount = u128::from_str_radix(&request.amount, 10).map_err(|e| format!("Invalid amount: {}", e))?;

    // Create the transaction data for a token transfer
    let data = create_erc20_transfer_data(&to_address, amount)?;

    // Get the nonce for the from_address
    let nonce = get_nonce(&from_address).await?;

    // Get current gas price
    let gas_price = get_gas_price().await?;

    // Create and sign the transaction
    let sign_request = SignRequest {
        to: CIT_TOKEN_ADDRESS.to_string(),
        value: Nat::from(0),    // value is 0 for token transfers
        gas: Nat::from(100000), // You may want to estimate this
        gas_price: Nat::from(gas_price),
        nonce: Nat::from(nonce),
        data,
    };

    let signed_tx = sign_transaction(sign_request).await?;

    // Send the signed transaction
    let tx_hash = send_raw_transaction(signed_tx).await?;

    Ok(format!("Transaction sent. Hash: {}", tx_hash))
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
        name: "key_1".to_string(),
    }
}

fn public_key_to_ethereum_address(public_key: &[u8]) -> [u8; 20] {
    let public_key = &public_key[1..];
    let mut hasher = Keccak256::new();
    hasher.update(public_key);
    let result = hasher.finalize();
    let mut address = [0u8; 20];
    address.copy_from_slice(&result[12..]);
    address
}

fn create_erc20_transfer_data(to: &str, amount: u128) -> Result<Vec<u8>, String> {
    let function = Function {
        name: "transfer".to_string(),
        inputs: vec![
            Param { name: "to".to_string(), kind: ParamType::Address, internal_type: None },
            Param { name: "amount".to_string(), kind: ParamType::Uint(256), internal_type: None },
        ],
        outputs: vec![],
        state_mutability: ethabi::StateMutability::NonPayable,
    };

    let to_address = hex::decode(to.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid 'to' address: {}", e))?;
    let to_address = ethabi::Address::from_slice(&to_address);
    
    let params = vec![
        Token::Address(to_address),
        Token::Uint(amount.into()),
    ];

    function.encode_input(&params)
        .map_err(|e| format!("Failed to encode function input: {:?}", e))
}

async fn get_nonce(address: &str) -> Result<u64, String> {
    use candid::Principal;

    let args = GetTransactionCountArgs {
        address: address.to_string(),
        block: BlockTag::Latest,
    };

    let cycles = 10_000_000_000;
    let result: Result<(MultiGetTransactionCountResult,), _> = 
        ic_cdk::api::call::call_with_payment128(
            Principal::from_text(EVM_RPC_CANISTER_ID).unwrap(),
            "eth_getTransactionCount",
            (RpcServices::BscTestnet(None), args),
            cycles,
        ).await;

    match result {
        Ok((MultiGetTransactionCountResult::Consistent(Ok(count)),)) => Ok(count.0.to_u64().unwrap()),
        Ok((MultiGetTransactionCountResult::Consistent(Err(e)),)) => Err(format!("RPC error: {:?}", e)),
        Ok((MultiGetTransactionCountResult::Inconsistent(results),)) => {
            Err(format!("Inconsistent results: {:?}", results))
        },
        Err(e) => Err(format!("Failed to call EVM RPC canister: {:?}", e)),
    }
}

async fn get_gas_price() -> Result<u64, String> {
    let services = RpcServices::BscTestnet(None);
    let config = ();
    let args = FeeHistoryArgs {
        block_count: 1,
        newest_block: BlockTag::Latest,
        reward_percentiles: None,
    };

    let cycles = 10_000_000_000;
    let result: Result<(MultiFeeHistoryResult,), _> = call_with_payment128(
        Principal::from_text(EVM_RPC_CANISTER_ID).unwrap(),
        "eth_feeHistory",
        (services, config, args),
        cycles,
    )
    .await;

    match result {
        Ok((MultiFeeHistoryResult::Consistent(Ok(fee_history)),)) => {
            let base_fee = fee_history.base_fee_per_gas.last().cloned().unwrap_or_default();
            Ok(base_fee.0.to_u64().unwrap())
        },
        Ok((MultiFeeHistoryResult::Consistent(Err(e)),)) => Err(format!("RPC error: {:?}", e)),
        Ok((MultiFeeHistoryResult::Inconsistent(results),)) => {
            Err(format!("Inconsistent results: {:?}", results))
        },
        Err(e) => Err(format!("Failed to get gas price: {:?}", e)),
    }
}

#[update(guard = "caller_is_not_anonymous")]
async fn sign_transaction(req: SignRequest) -> Result<String, String> {
    let rlp_encoded = rlp_encode_transaction(&req);
    let message_hash = keccak256(&rlp_encoded);
    let (_, signature) = pubkey_and_signature(&ic_cdk::caller(), message_hash.to_vec()).await;
    
    // Combine RLP encoded transaction and signature
    let mut signed_tx = rlp_encoded;
    signed_tx.extend_from_slice(&signature);
    
    Ok(hex::encode(signed_tx))
}

fn rlp_encode_transaction(req: &SignRequest) -> Vec<u8> {
    let mut rlp = RlpStream::new();
    rlp.begin_list(9);
    rlp.append(&req.nonce.0.to_u64().unwrap_or(0));
    rlp.append(&req.gas_price.0.to_u64().unwrap_or(0));
    rlp.append(&req.gas.0.to_u64().unwrap_or(0));
    rlp.append(&hex::decode(req.to.trim_start_matches("0x")).unwrap());
    rlp.append(&req.value.0.to_u64().unwrap_or(0));
    rlp.append(&req.data);
    rlp.append(&BSC_TESTNET_CHAIN_ID);
    rlp.append(&0u8);
    rlp.append(&0u8);
    rlp.out().to_vec()
}

async fn send_raw_transaction(raw_tx: String) -> Result<String, String> {
    let services = RpcServices::BscTestnet(None);
    let config = ();

    let cycles = 10_000_000_000;
    let result: Result<(MultiSendRawTransactionResult,), _> = call_with_payment128(
        Principal::from_text(EVM_RPC_CANISTER_ID).unwrap(),
        "eth_sendRawTransaction",
        (services, config, raw_tx),
        cycles,
    )
    .await;

    match result {
        Ok((MultiSendRawTransactionResult::Consistent(Ok(hash)),)) => Ok(hash),
        Ok((MultiSendRawTransactionResult::Consistent(Err(e)),)) => Err(format!("RPC error: {:?}", e)),
        Ok((MultiSendRawTransactionResult::Inconsistent(results),)) => {
            Err(format!("Inconsistent results: {:?}", results))
        },
        Err(e) => Err(format!("Failed to send transaction: {:?}", e)),
    }
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

async fn pubkey_and_signature(_caller: &Principal, message_hash: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let derivation_path = vec![]; // Empty for root key
    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: "key_1".to_string(),
    };

    let public_key = ecdsa_public_key(EcdsaPublicKeyArgument {
        canister_id: Some(ic_cdk::id()),
        derivation_path: derivation_path.clone(),
        key_id: key_id.clone(),
    })
    .await
    .unwrap().0.public_key;

    let signature = sign_with_ecdsa(SignWithEcdsaArgument {
        message_hash,
        derivation_path,
        key_id,
    })
    .await
    .unwrap().0.signature;

    (public_key, signature)
}

fn caller_is_not_anonymous() -> Result<(), String> {
    if ic_cdk::caller() == Principal::anonymous() {
        Err("Anonymous calls are not allowed".to_string())
    } else {
        Ok(())
    }
}