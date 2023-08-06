use aleo_rust::{AleoAPIClient, Network, Plaintext, ProgramManager, Record, TransferType};
use anyhow::Error;
use snarkvm_console::{
    account::{Address, PrivateKey},
    network::Testnet3,
};
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::{env, thread};

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub fn find_unspent_records_on_chain<N: Network>(
    api_client: &AleoAPIClient<N>,
    amounts: Option<&Vec<u64>>,
    max_microcredits: Option<u64>,
    private_key: &PrivateKey<N>,
    block_hint: Option<u32>,
) -> Result<Vec<Record<N, Plaintext<N>>>> {
    let search_range = if let Some(block_index) = block_hint {
        block_index - 1..block_index + 1
    } else {
        0..api_client.latest_height()?
    };
    let records =
        api_client.get_unspent_records(private_key, search_range, max_microcredits, amounts)?;
    Ok(records.into_iter().map(|(_, record)| record).collect())
}

fn lines_from_file(filename: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(filename).expect("no such file");
    let buf = BufReader::new(file);
    buf.lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 7 {
        println!("Use: aleo_gas_dispenser <amount> <fee> <private key> <file with addresses> <retries> <delay between tx in milliseconds>");
        return;
    }

    let amount = args[1].parse::<u64>().unwrap();
    let fee = args[2].parse::<u64>().unwrap();
    let pk = &args[3];
    let recipient_addr_file = &args[4];
    let max_retry_count = args[5].parse::<u64>().unwrap();
    let sleep_millis = args[6].parse::<u64>().unwrap();

    let private_key = PrivateKey::<Testnet3>::from_str(pk).unwrap();
    println!("Using private key: {}", private_key);

    let api_client = AleoAPIClient::<Testnet3>::testnet3();

    let addresses = lines_from_file(recipient_addr_file);
    for addr in addresses.iter() {
        let recipient_address = Address::<Testnet3>::from_str(addr).unwrap();
        println!("Sending to {}", recipient_address);

        let records = find_unspent_records_on_chain(
            &api_client,
            Some(&vec![fee, amount]),
            None,
            &private_key,
            None,
        )
        .unwrap();
        println!("Fee record: {}", records[1]);
        println!("Amount record: {}", records[0]);
        let mut num_failures = 0;
        loop {
            println!("Starting private transfer...");

            let program_manager = ProgramManager::<Testnet3>::new(
                Some(private_key),
                None,
                Some(AleoAPIClient::<Testnet3>::testnet3()),
                None,
            )
            .unwrap();

            match program_manager.transfer(
                amount,
                fee,
                recipient_address,
                TransferType::Private,
                None,
                Some(records[0].clone()),
                records[1].clone(),
            ) {
                Ok(msg) => {
                    println!("Transfer result {}", msg);
                    println!("Sleeping 1 min...");
                    thread::sleep(Duration::from_millis(sleep_millis));
                    println!("Wake up!");
                    break;
                }
                Err(err) => {
                    println!("err {}\nRetrying...", err);
                    num_failures += 1;
                    if num_failures == max_retry_count {
                        println!("Max retries exceeded!!!");
                        return;
                    }
                }
            }
        }
    }
}
