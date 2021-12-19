use crate::cli::Options;
use crate::error::Error;
use rand::thread_rng;
use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use solana_sdk::{
    native_token,
    signature::{keypair_from_seed, Signer},
    system_instruction,
};
use structopt::StructOpt;

mod cli;
mod error;

fn main() -> Result<(), Error> {
    let opts = Options::from_args();
    let mut rng = thread_rng();
    match opts {
        Options::Generate => {
            let keypair = Keypair::generate(&mut rng);
            println!("secret key: {}", bs58::encode(keypair.secret()).into_string());
            println!("public key: {}", keypair.pubkey());
            return Ok(());
        }
        Options::Airdrop { to, amount, net } => {
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            let amount = native_token::sol_to_lamports(amount);
            let sig = rpc_client.request_airdrop(&to, amount).map_err(Error::AirdropFailed)?;
            println!("Airdrop transaction ID: {}", sig);
            let recent_hash = rpc_client.get_latest_blockhash().map_err(Error::RecentHashFailed)?;
            rpc_client
                .confirm_transaction_with_spinner(&sig, &recent_hash, rpc_client.commitment())
                .map_err(Error::ConfirmingTransactionFailed)?;
        }
        _ => todo!(),
    }
    let seed = [138u8; 32];
    let keypair = keypair_from_seed(&seed).unwrap();
    println!("Hello, world!, {}", keypair.pubkey());

    let server = "https://api.mainnet-beta.solana.com";
    let testnet = "https://api.testnet.solana.com";
    let devnet = "https://api.devnet.solana.com";

    let rpc_client = RpcClient::new(devnet.to_string());
    let balance = rpc_client.get_balance(&keypair.pubkey()).unwrap();
    println!("Balance: {}", balance);

    let amount = native_token::sol_to_lamports(5f64);

    let sig = rpc_client
        .request_airdrop(&keypair.pubkey(), amount)
        .unwrap();
    println!("sent: {}", sig);

    let recent_hash= rpc_client.get_latest_blockhash().unwrap();

    rpc_client.confirm_transaction_with_spinner(&sig, &recent_hash, rpc_client.commitment()).unwrap();
    let balance = rpc_client.get_balance(&keypair.pubkey()).unwrap();
    println!("Balance: {}", balance);

    let other_privkey = [142; 32];
    let other_keypair = keypair_from_seed(&other_privkey).unwrap();
    let balance = rpc_client.get_balance(&other_keypair.pubkey()).unwrap();
    println!("Other Balance: {}", balance);

    let sending = native_token::sol_to_lamports(1.4);

    let instrcutions = vec![system_instruction::transfer(&keypair.pubkey(), &other_keypair.pubkey(), sending)].with_memo(Some("Threshold Solana"));
    let msg = Message::new(&instrcutions, Some(&keypair.pubkey()));
    let mut tx = Transaction::new_unsigned(msg);
    tx.sign(&[&keypair], recent_hash);
    let sig = rpc_client.send_transaction(&tx).unwrap();
    println!("sent: {}", sig);
    rpc_client.confirm_transaction_with_spinner(&sig, &recent_hash, rpc_client.commitment()).unwrap();

    let balance = rpc_client.get_balance(&other_keypair.pubkey()).unwrap();
    println!("Other Balance: {}", balance);

    Ok(())
}

pub trait WithMemo {
    fn with_memo<T: AsRef<str>>(self, memo: Option<T>) -> Self;
}

impl WithMemo for Vec<Instruction> {
    fn with_memo<T: AsRef<str>>(mut self, memo: Option<T>) -> Self {
        if let Some(memo) = &memo {
            let memo = memo.as_ref();
            let memo_ix = Instruction {
                program_id: Pubkey::new(&spl_memo::id().to_bytes()),
                accounts: vec![],
                data: memo.as_bytes().to_vec(),
            };
            self.push(memo_ix);
        }
        self
    }
}