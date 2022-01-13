#!/usr/bin/env bash
set -e

party_1() {
  printf "\e[1;4;31mParty 1: %s\e[0m\n" "$1"
}

party_2() {
  printf "\e[1;4;34mParty 2: %s\e[0m\n" "$1"
}

all_parties() {
  printf "\e[1;4;33m%s\e[0m\n" "$1"
}

short_print() {
  printf "%s..%s" "${1::4}" "${1: -4}"
}

cargo build
cd ./target/debug/ || exit
PATH="$PATH:."

party_1 "Generate Shares"
echo "$ solana-tss generate"
sleep 0.6s
keypair1=$(solana-tss generate)
pubkey1=$(echo "$keypair1" | tac -s " " | head -1)
secretkey1=$(echo "$keypair1" | head -1 | cut -d " " -f3)
printf "secret share: %s\npublic share: %s \n" "$(short_print "$secretkey1")" "$(short_print "$pubkey1")"
sleep 0.3s

party_2 "Generate Shares"
echo "$ solana-tss generate"
sleep 0.6s
keypair2=$(solana-tss generate)
pubkey2=$(echo "$keypair2" | tac -s " " | head -1)
secretkey2=$(echo "$keypair2" | head -1 | cut -d " " -f3)
printf "secret share: %s\npublic share: %s \n\n" "$(short_print "$secretkey2")" "$(short_print "$pubkey2")"
sleep 0.3s

all_parties "Aggregate the Shares(either party can execute)"
printf "$ solana-tss aggregate-keys %s %s\n" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")"
sleep 0.6s
aggkey_text=$( solana-tss aggregate-keys "$pubkey1" "$pubkey2" )
aggkey=$(echo "$aggkey_text" | tac -s " " | head -1)
printf "The Aggregated Public Key: %s\n\n" "$(short_print "$aggkey")"
sleep 0.3s

all_parties "Request some SOLs from a faucet"
printf "$ solana-tss airdrop --net devnet --to %s --amount 0.2\n" "$(short_print "$aggkey")"
sleep 0.6s
solana-tss airdrop --net devnet --to "$aggkey" --amount 0.2
balance=$(solana-tss balance --net devnet "$aggkey" | cut -d " " -f6)
printf "The balance of %s is: %s\n\n" "$(short_print "$aggkey"))" "$balance"
sleep 0.3s


all_parties "Reciever key and balance"
echo "$ solana-tss generate"
sleep 0.6s
keypair_reciever=$( solana-tss generate )
reciever_key=$(echo "$keypair_reciever" | tac -s " " | head -1)
balance=$(solana-tss balance --net devnet "$reciever_key" | cut -d " " -f6)
printf "The balance of %s is: %s\n\n" "$(short_print "$reciever_key")" "$balance"
sleep 0.3s

printf "\e[1;4;32mSending 0.1 SOL to %s\e[0m\n\n" "$(short_print "$reciever_key")"

party_1 "Generate message 1"
printf "$ solana-tss agg-send-step-one %s\n" "$(short_print "$secretkey1")"
sleep 0.6s
party1_raw=$( solana-tss agg-send-step-one "$secretkey1" )
party1msg1=$(echo "$party1_raw" | head -1 | cut -d " " -f3)
party1state=$(echo "$party1_raw" | tail -1 | cut -d " " -f3)
printf "Message 1: %s (send to all other parties)\nSecret state: %s (keep this a secret, and pass it back to \`agg-send-step-two\`)\n" "$(short_print "$party1msg1")" "$(short_print "$party1state")"
sleep 0.3s

party_2 "Generate message 1"
printf "$ solana-tss agg-send-step-one %s\n" "$(short_print "$secretkey2")"
sleep 0.6s
party2_raw=$( solana-tss agg-send-step-one "$secretkey2" )
party2msg1=$(echo "$party2_raw" | head -1 | cut -d " " -f3)
party2state=$(echo "$party2_raw" | tail -1 | cut -d " " -f3)
printf "Message 1: %s (send to all other parties)\nSecret state: %s (keep this a secret, and pass it back to \`agg-send-step-two\`)\n\n" "$(short_print "$party2msg1")" "$(short_print "$party2state")"
sleep 0.3s

all_parties "Check recent block hash"
echo "$ solana-tss recent-block-hash --net devnet"
sleep 0.6s
recent_block_hash=$( solana-tss recent-block-hash --net devnet )
recent_block_hash=$(echo "$recent_block_hash" | cut -d " " -f4)
printf "Recent block hash: %s\n\n" "$(short_print "$recent_block_hash")"
sleep 0.3s

party_1 "Process message 1 and generate message 2"
printf "$ solana-tss agg-send-step-two --keypair %s --to %s --amount 0.1 --memo \"ZenGo: 2 Party Signing\" --keys %s --keys %s --recent-block-hash %s --first-messages %s --secret-state %s\n" \
  "$(short_print "$secretkey1")" "$(short_print "$reciever_key")" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")" "$(short_print "$recent_block_hash")" "$(short_print "$party2msg1")" "$(short_print "$party1state")"
sleep 0.6s
party1_raw=$( solana-tss agg-send-step-two --keypair "$secretkey1" --to "$reciever_key" --amount 0.1 --memo "ZenGo: 2 Party Signing" --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --first-messages "$party2msg1" --secret-state "$party1state" )
partialsig1=$(echo "$party1_raw" | cut -d " " -f3)
printf "Partial signature: %s\n" "$(short_print "$partialsig1")"
sleep 0.3s


party_2 "Process message 1 and generate message 2"
printf "$ solana-tss agg-send-step-two --keypair %s --to %s --amount 0.1 --memo \"ZenGo: 2 Party Signing\" --keys %s --keys %s --recent-block-hash %s --first-messages %s --secret-state %s\n" \
  "$(short_print "$secretkey2")" "$(short_print "$reciever_key")" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")" "$(short_print "$recent_block_hash")" "$(short_print "$party1msg1")" "$(short_print "$party2state")"
sleep 0.6s
party2_raw=$( solana-tss agg-send-step-two --keypair "$secretkey2" --to "$reciever_key" --amount 0.1 --memo "ZenGo: 2 Party Signing" --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --first-messages "$party1msg1" --secret-state "$party2state" )
partialsig2=$(echo "$party2_raw" | cut -d " " -f3)
printf "Partial signature: %s\n\n" "$(short_print "$partialsig2")"
sleep 0.3s

all_parties "Combine the signatures and send"
printf "$ solana-tss aggregate-signatures-and-broadcast --net devnet --to %s --amount 0.1 --memo \"ZenGo: 2 Party Signing\" --keys %s --keys %s --recent-block-hash %s --signatures %s --signatures %s\n" \
  "$(short_print "$reciever_key")" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")" "$(short_print "$recent_block_hash")" "$(short_print "$partialsig1")" "$(short_print "$partialsig2")"
sleep 0.6s
raw=$( solana-tss aggregate-signatures-and-broadcast --net devnet --to "$reciever_key" --amount 0.1 --memo "ZenGo: 2 Party Signing" --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --signatures "$partialsig1" --signatures "$partialsig2")
printf "%s\n\n" "$raw"
sleep 0.3s

all_parties "Reciever new balance"
sleep 0.6s
balance=$(solana-tss balance --net devnet "$reciever_key" | cut -d " " -f6)
printf "The balance of %s is: %s\n" "$(short_print "$reciever_key")" "$balance"