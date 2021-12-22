#!/usr/bin/env bash

title() {
  printf "\e[1;4;31m%s\e[0m\n" "$1"
}

short_print() {
  printf "%s..%s" "${1::4}" "${1: -4}"
}
cargo build
cd ./target/debug/ || exit
PATH="$PATH:."

title "Generate Keys"
echo "$ solana-tss generate"
keypair1=$(solana-tss generate)
pubkey1=$(echo "$keypair1" | tac -s " " | head -1)
secretkey1=$(echo "$keypair1" | head -1 | cut -d " " -f3)
printf "secret key: %s\npublic key: %s \n\n" "$(short_print "$secretkey1")" "$(short_print "$pubkey1")"

echo "$ solana-tss generate"
keypair2=$(solana-tss generate)
pubkey2=$(echo "$keypair2" | tac -s " " | head -1)
secretkey2=$(echo "$keypair2" | head -1 | cut -d " " -f3)
printf "secret key: %s\npublic key: %s \n\n" "$(short_print "$secretkey2")" "$(short_print "$pubkey2")"

title "Aggregate the keys"
printf "$ solana-tss aggregate-keys %s %s\n" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")"
aggkey_text=$( solana-tss aggregate-keys "$pubkey1" "$pubkey2" )
aggkey=$(echo "$aggkey_text" | tac -s " " | head -1)
printf "The Aggregated Public Key: %s\n\n" "$(short_print "$aggkey")"

title "Request some SOLs from a faucet"
printf "$ solana-tss airdrop --to %s --amount 0.2\n" "$(short_print "$aggkey")"
solana-tss airdrop --to "$aggkey" --amount 0.2
balance=$(solana-tss balance "$aggkey" | cut -d " " -f6)
printf "The balance of %s is: %s\n" "$(short_print "$aggkey"))" "$balance"


title "Reciever key and balance"
echo "$ solana-tss generate"
keypair_reciever=$( solana-tss generate )
reciever_key=$(echo "$keypair_reciever" | tac -s " " | head -1)
balance=$(solana-tss balance "$reciever_key" | cut -d " " -f6)
printf "The balance of %s is: %s\n\n" "$(short_print "$reciever_key"))" "$balance"

title "Generate message 1 and send"
printf "$ solana-tss agg-send-step-one %s\n" "$(short_print "$secretkey1")"
party1_raw=$( solana-tss agg-send-step-one "$secretkey1" )
party1msg1=$(echo "$party1_raw" | head -1 | cut -d " " -f3)
party1state=$(echo "$party1_raw" | tail -1 | cut -d " " -f3)
printf "Message 1: %s (send to all other parties)\nSecret state: %s (keep this a secret, and pass it back to \`agg-send-step-two\`)\n\n" "$(short_print "$party1msg1")" "$(short_print "$party1state")"

printf "$ solana-tss agg-send-step-one %s\n" "$(short_print "$secretkey2")"
party2_raw=$( solana-tss agg-send-step-one "$secretkey2" )
party2msg1=$(echo "$party2_raw" | head -1 | cut -d " " -f3)
party2state=$(echo "$party2_raw" | tail -1 | cut -d " " -f3)
printf "Message 1: %s (send to all other parties)\nSecret state: %s (keep this a secret, and pass it back to \`agg-send-step-two\`)\n" "$(short_print "$party2msg1")" "$(short_print "$party2state")"

title "Process message 1 and generate message 2"
printf "$ solana-tss agg-send-step-two --first-messages %s --keypair %s --secret-state %s \n\n" "$(short_print "$party2msg1")" "$(short_print "$secretkey1")" "$(short_print "$party1state")"
party1_raw=$( solana-tss agg-send-step-two --first-messages "$party2msg1" --keypair "$secretkey1" --secret-state "$party1state" )
party1msg2=$(echo "$party1_raw" | head -1 | cut -d " " -f3)
party1state=$(echo "$party1_raw" | tail -1 | cut -d " " -f3)
printf "Message 2: %s (send to all other parties)\nSecret state: %s (keep this a secret, and pass it back to \`agg-send-step-three\`)\n" "$(short_print "$party1msg2")" "$(short_print "$party1state")"


printf "$ solana-tss agg-send-step-two --first-messages %s --keypair %s --secret-state %s \n\n" "$(short_print "$party1msg1")" "$(short_print "$secretkey2")" "$(short_print "$party2state")"
party2_raw=$( solana-tss agg-send-step-two --first-messages "$party1msg1" --keypair "$secretkey2" --secret-state "$party2state" )
party2msg2=$(echo "$party2_raw" | head -1 | cut -d " " -f3)
party2state=$(echo "$party2_raw" | tail -1 | cut -d " " -f3)
printf "Message 2: %s (send to all other parties)\nSecret state: %s (keep this a secret, and pass it back to \`agg-send-step-three\`)\n" "$(short_print "$party2msg2")" "$(short_print "$party2state")"

title "Check  recent block hash"
echo "$ solana-tss recent-block-hash"
recent_block_hash=$( solana-tss recent-block-hash )
recent_block_hash=$(echo "$recent_block_hash" | cut -d " " -f4)
printf "Recent block hash: %s\n" "$(short_print "$recent_block_hash")"

title "Process message 2 and generate message 3"
printf "$ solana-tss agg-send-step-three --keypair %s --to %s --amount 0.1 --memo \"2 Party Signing\" --keys %s --keys %s --recent-block-hash %s --second-messages %s --secret-state %s\n" \
  "$(short_print "$secretkey1")" "$(short_print "$reciever_key")" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")" "$(short_print "$recent_block_hash")" "$(short_print "$party2msg2")" "$(short_print "$party1state")"
party1_raw=$( solana-tss agg-send-step-three --keypair "$secretkey1" --to "$reciever_key" --amount 0.1 --memo "2 Party Signing" --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --second-messages "$party2msg2" --secret-state "$party1state" )
partialsig1=$(echo "$party1_raw" | cut -d " " -f3)
printf "Partial signature: %s\n" "$(short_print "$partialsig1")"

printf "$ solana-tss agg-send-step-three --keypair %s --to %s --amount 0.1 --memo \"2 Party Signing\" --keys %s --keys %s --recent-block-hash %s --second-messages %s --secret-state %s\n" \
  "$(short_print "$secretkey2")" "$(short_print "$reciever_key")" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")" "$(short_print "$recent_block_hash")" "$(short_print "$party1msg2")" "$(short_print "$party2state")"
party2_raw=$( solana-tss agg-send-step-three --keypair "$secretkey2" --to "$reciever_key" --amount 0.1 --memo "2 Party Signing" --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --second-messages "$party1msg2" --secret-state "$party2state" )
partialsig2=$(echo "$party2_raw" | cut -d " " -f3)
printf "Partial signature: %s\n" "$(short_print "$partialsig2")"

title "Combine the signatures and send"
printf "$ solana-tss aggregate-signatures-and-broadcast --to %s --amount 0.1 --memo \"2 Party Signing\" --keys %s --keys %s --recent-block-hash %s --signatures %s --signatures %s\n" \
  "$(short_print "$reciever_key")" "$(short_print "$pubkey1")" "$(short_print "$pubkey2")" "$(short_print "$recent_block_hash")" "$(short_print "$partialsig1")" "$(short_print "$partialsig2")"
raw=$( solana-tss aggregate-signatures-and-broadcast --to "$reciever_key" --amount 0.1 --memo "2 Party Signing" --keys "$pubkey1" --keys "$pubkey2" --recent-block-hash "$recent_block_hash" --signatures "$partialsig1" --signatures "$partialsig2")
printf "%s\n\n" "$raw"

title "Reciever new balance"
balance=$(solana-tss balance "$reciever_key" | cut -d " " -f6)
printf "The balance of %s is: %s\n" "$(short_print "$reciever_key"))" "$balance"