mod ledger;

use crate::ledger::{Transaction, Ledger};
use csv::{ReaderBuilder, Writer};
use std::io;
use std::path::Path;

fn main() {
    let args = std::env::args();

    let args = args.collect::<Vec<String>>();

    if args.len() > 2 {
        eprintln!("Warning: Extra arguments will be ignored");
    }

    let file_path = if let Some(file_path) = args.get(1) {
        Path::new(file_path)
    } else {
        eprintln!("A path to a CSV file must be provided.");
        std::process::exit(1);
    };

    if !file_path.exists() {
        eprintln!("File \"{}\" not found.", file_path.to_string_lossy());
        std::process::exit(1);
    }

    let mut reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(file_path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to open CSV file: {}", e);
            std::process::exit(1);
        });

    let records = reader.deserialize::<Transaction>();

    let mut ledger = Ledger::new();

    for record in records {
        let result = record
            .map_err(|err| err.to_string())
            .and_then(|transaction: Transaction| ledger.handle_new_transaction(&transaction));
        if let Err(e) = result {
            eprintln!("Warning: {}", e);
        }
    }

    let mut csv_writer = Writer::from_writer(io::stdout());
    if let Err(e) = csv_writer.write_record(["client", "available", "held", "total", "locked"]) {
        eprintln!("Failed to write CSV header: {}", e);
        // Exit if we're not able to write the CSV
        std::process::exit(1);
    }

    for (client_id, account_status) in ledger.client_accounts() {
        if let Err(e) = csv_writer.write_record([
            client_id.to_string(),
            account_status.available.to_string(),
            account_status.held.to_string(),
            account_status.total.to_string(),
            account_status.locked.to_string(),
        ]) {
            eprintln!(
                "Error writing the following line to the CSV row: {}, {}, {}, {}, {}. Error: {}",
                client_id.to_string(),
                account_status.available.to_string(),
                account_status.held.to_string(),
                account_status.total.to_string(),
                account_status.locked.to_string(),
                e.to_string()
            );
            // Exit if we're not able to write the CSV
            std::process::exit(1);
        }
    }

    if let Err(e) = csv_writer.flush() {
        eprintln!("Failed to flush CSV output: {}", e);
        std::process::exit(1);
    }
}
