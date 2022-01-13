# Solana Threshold Signatures PoC
A Proof-Of-Concept showing n-of-n offchain multisignatures on Solana


## Demo
![gif](./demo.gif)

## Building
### From Sources
With Rust's package manager cargo, you can install solana-tss via:

```sh
cargo install --git https://github.com/ZenGo-X/solana-tss.git
```

# Usage

Help:
```
solana-tss 0.1.0
A PoC for managing a Solana TSS wallet

USAGE:
    solana-tss <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    generate
            Generate a pair of keys
    balance
            Check the balance of an address
    airdrop
            Request an airdrop from a faucet
    send-single
            Send a transaction using a single private key
    aggregate-keys
            Aggregate a list of addresses into a single address that they can all sign on together
    agg-send-step-one
            Start aggregate signing
    recent-block-hash
            Print the hash of a recent block, can be used to pass to the `agg-send` steps
    agg-send-step-two
            Step 2 of aggregate signing, you should pass in the secret data from step 1. It's
            important that all parties pass in exactly the same transaction details
            (amount,to,net,memo,recent_block_hash)
    aggregate-signatures-and-broadcast
            Aggregate all the partial signatures together into a full signature, and send the
            transaction to Solana
    help
            Print this message or the help of the given subcommand(s)
```

## Choosing a different network
By default, the tool uses `testnet` but this can be overriden by passing `--net mainnet / devnet / testnet`
