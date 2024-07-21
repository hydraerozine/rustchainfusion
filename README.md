# `ninarust`

This canister is involved in sending CIT tokens from main wallet to other wallets. It will work with solidity contracts to perform swaps on different DEXes. Currently still developing the transfer function. Then will be working on contract calling. Thus far generated EVM wallet

canister id on Mainnet: canister ninarust_backend 4nnva-zqaaa-aaaap-qho4q-cai

URLs Mainnet: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=4nnva-zqaaa-aaaap-qho4q-cai

use to update cargo (make sure to delete lock)

cargo update                                                          
dfx build
dfx canister install ninarust_backend --mode reinstall

command to get ethereum public address
dfx canister call ninarust_backend get_ethereum_address --ic

command to send CIT token to an address
dfx canister --ic call ninarust_backend transfer_bsc_testnet_token '(record { to = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"; amount = "1000000000000000000" })'

