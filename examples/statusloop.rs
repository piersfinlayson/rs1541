use clap::Parser;
/// Loops retrieving status from a drive
use rs1541::{Cbm, Rs1541Error, MAX_DEVICE_NUM, MIN_DEVICE_NUM};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

pub const SLEEP_DURATION: Duration = Duration::from_millis(10);

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(8..=30))]
    device: u8,
}

fn main() -> Result<(), Rs1541Error> {
    // Parse command line arguments
    let args = Args::parse();
    if args.device < MIN_DEVICE_NUM || args.device > MAX_DEVICE_NUM {
        eprintln!("Error: device number must be between {MIN_DEVICE_NUM} and {MAX_DEVICE_NUM}");
        std::process::exit(1);
    }

    // Set up ctrl-c handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Create CBM interface
    let cbm = Cbm::new()?;

    let mut success_count = 0u64;
    let mut fail_count = 0u64;

    while running.load(Ordering::SeqCst) {
        // Try to get drive status
        match cbm.get_status(args.device) {
            Ok(_) => success_count += 1,
            Err(_) => fail_count += 1,
        }

        // Update display (carriage return without newline)
        print!(
            "\rSuccesses: {:<20} Failures: {:<20}",
            success_count, fail_count
        ); // Ensure the output is displayed immediately
        std::io::Write::flush(&mut std::io::stdout()).expect("Failed to flush stdout");

        // Wait 10ms
        sleep(SLEEP_DURATION);
    }

    // Print final newline
    println!();

    Ok(())
}
