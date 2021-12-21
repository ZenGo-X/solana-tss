#!/usr/bin/env bash

title() {
  printf "\e[1;4;31m%s\e[0m\n" "$1"
}

cargo build
cd ./target/debug/ || exit
PATH="$PATH:."

title "Generate Keys"
keypair1=$( set -x ; solana-tss generate )
printf "%s\n\n" "$keypair1"
pubkey1=$(echo "$keypair1" | tac -s " " | head -1)
secretkey1=$(echo "$keypair1" | head -1 | cut -d " " -f3)

keypair2=$( set -x ; solana-tss generate )
printf "%s\n\n" "$keypair2"
pubkey2=$(echo "$keypair2" | tac -s " " | head -1)
secretkey2=$(echo "$keypair2" | head -1 | cut -d " " -f3)

title "Aggregate the keys"
aggkey_text=$( set -x ; solana-tss aggregate-keys "$pubkey1" "$pubkey2" )
printf "%s\n\n" "$aggkey_text"
aggkey=$(echo "$aggkey_text" | tac -s " " | head -1)

title "Request some SOLs from a faucet"
( set -x ; solana-tss airdrop --to "$aggkey" --amount 0.2 )
( set -x ; solana-tss balance "$aggkey" )

title "Reciever key and balance"
keypair_reciever=$( solana-tss generate )
reciever_key=$(echo "$keypair_reciever" | tac -s " " | head -1)
( solana-tss balance "$reciever_key" )
echo ""

title "Generate message 1 and send"
party1_raw=$( set -x ; solana-tss agg-send-step-one "$secretkey1" )
printf "%s\n\n" "$party1_raw"
party1msg1=$(echo "$party1_raw" | head -1 | cut -d " " -f3)
party1state=$(echo "$party1_raw" | tail -1 | cut -d " " -f3)

party2_raw=$( set -x ; solana-tss agg-send-step-one "$secretkey2" )
printf "%s\n\n" "$party2_raw"
party2msg1=$(echo "$party2_raw" | head -1 | cut -d " " -f3)
party2state=$(echo "$party2_raw" | tail -1 | cut -d " " -f3)

title "Process message 1 and generate message 2"
party1_raw=$( set -x ; solana-tss agg-send-step-two --first-messages "$party2msg1" --keypair "$secretkey1" --secret-state "$party1state" )
printf "%s\n\n" "$party1_raw"
party1msg2=$(echo "$party1_raw" | head -1 | cut -d " " -f3)
party1state=$(echo "$party1_raw" | tail -1 | cut -d " " -f3)


party2_raw=$( set -x ; solana-tss agg-send-step-two --first-messages "$party1msg1" --keypair "$secretkey2" --secret-state "$party2state" )
printf "%s\n\n" "$party2_raw"
party2msg2=$(echo "$party2_raw" | head -1 | cut -d " " -f3)
party2state=$(echo "$party2_raw" | tail -1 | cut -d " " -f3)

title "Check  recent block hash"
recent_block_hash=$( set -x ; solana-tss recent-block-hash )
printf "%s\n\n" "$recent_block_hash"
recent_block_hash=$(echo "$recent_block_hash" | cut -d " " -f4)


title "Process message 2 and generate message 3"
party1_raw=$( set -x ; solana-tss agg-send-step-three --keypair "$secretkey1" --to "$reciever_key" --amount 0.1 --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --second-messages "$party2msg2" --secret-state "$party1state" )
printf "%s\n\n" "$party1_raw"
partialsig1=$(echo "$party1_raw" | cut -d " " -f3)


party2_raw=$( set -x ; solana-tss agg-send-step-three --keypair "$secretkey2" --to "$reciever_key" --amount 0.1 --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --second-messages "$party1msg2" --secret-state "$party2state" )
printf "%s\n\n" "$party2_raw"
partialsig2=$(echo "$party2_raw" | cut -d " " -f3)

title "Combine the signatures and send"
raw=$( set -x ; solana-tss aggregate-signatures-and-broadcast --to "$reciever_key" --amount 0.1 --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --signatures "$partialsig1" --signatures "$partialsig2")
printf "%s\n\n" "$raw"

title "Reciever new balance"
( solana-tss balance "$reciever_key" )