use rs1541::{AsciiString, Cbm, CbmString, Error, DEVICE_MAX_NUM, DEVICE_MIN_NUM};
use xum1541::{Device, RemoteUsbDeviceConfig};

use clap::Parser;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn, LevelFilter};
use rustyline::{error::ReadlineError, DefaultEditor};
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Device number to test (8-15)
    #[arg(short, long, default_value_t = 8)]
    device: u8,

    /// Verbosity level (-v for Info, -vv for Debug, -vvv for Trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Use remote USB connection instead of local USB
    #[arg(short, long)]
    remote: bool,

    /// Remote server IP address (IPv4 only)
    #[arg(long, default_value = "127.0.0.1")]
    remote_ip: String,

    /// Remote server port number
    #[arg(long, default_value_t = xum1541::device::remoteusb::DEFAULT_PORT)]
    remote_port: u16,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Setup logging
    env_logger::builder()
        .filter_level(match args.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        })
        .filter_module("rustyline", LevelFilter::Error) // Disable rustline logging
        .init();

    info!("rs1541 Test Application");

    // Create Cbm object and run the program
    if args.remote {
        let addr: SocketAddr = format!("{}:{}", args.remote_ip, args.remote_port)
            .parse()
            .map_err(|e| Error::Validation {
                message: format!("Invalid remote address: {}", e),
            })?;

        let config = RemoteUsbDeviceConfig {
            serial_num: None,
            remote_addr: Some(addr),
        };
        let cbm = Cbm::new_remote_usb(Some(config))?;
        run(cbm, args)
    } else {
        let cbm = Cbm::new_usb(None)?;
        run(cbm, args)
    }
}

fn run<D: Device>(cbm: Cbm<D>, args: Args) -> Result<(), Error> {
    let mut device = args.device;

    // Setup command line editor
    let mut rl = DefaultEditor::new()
        .inspect_err(|e| {
            println!("Hit error in rustline: {}", e);
            std::process::exit(1);
        })
        .unwrap();

    loop {
        let readline = rl.readline("rs1541-cli> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())
                    .inspect_err(|e| {
                        println!("Hit error in rusttline: {}", e);
                        std::process::exit(1);
                    })
                    .unwrap();

                let cmd: Vec<String> = {
                    let mut cmd = Vec::new();
                    let mut current = String::new();
                    let mut in_quotes = false;
                    let chars: Vec<char> = line.chars().collect();
                    let mut i = 0;

                    while i < chars.len() {
                        match chars[i] {
                            '"' => {
                                in_quotes = !in_quotes;
                                if !in_quotes && !current.is_empty() {
                                    cmd.push(current.clone());
                                    current.clear();
                                }
                            }
                            ' ' if !in_quotes => {
                                if !current.is_empty() {
                                    cmd.push(current.clone());
                                    current.clear();
                                }
                            }
                            _ => current.push(chars[i]),
                        }
                        i += 1;
                    }
                    if !current.is_empty() {
                        cmd.push(current);
                    }
                    cmd
                };

                if cmd.is_empty() {
                    continue;
                }

                match cmd[0].as_str() {
                    "quit" | "exit" | "q" | "x" => break,

                    "identify" | "id" | "i" => match cbm.identify(device) {
                        Ok(info) => println!(
                            "Device type: {} description: {}",
                            info.device_type, info.description
                        ),
                        Err(e) => println!("Error: {}", e),
                    },

                    "scan" | "a" => {
                        let (min, max) = if cmd.len() == 3 {
                            match (cmd[1].parse::<u8>(), cmd[2].parse::<u8>()) {
                                (Ok(min), Ok(max))
                                    if min <= max
                                        && (8..=15).contains(&min)
                                        && (DEVICE_MIN_NUM..=DEVICE_MAX_NUM).contains(&max) =>
                                {
                                    (min, max)
                                }
                                _ => {
                                    println!("Invalid device numbers. Must be between 8{}-{} and min must be <= max", DEVICE_MIN_NUM, DEVICE_MAX_NUM);
                                    continue;
                                }
                            }
                        } else if cmd.len() == 1 {
                            (8, 11) // Default range
                        } else {
                            println!("Usage: scan [min max] - min and max must be between 8-15");
                            continue;
                        };
                        scan(&cbm, min, max);
                    }

                    "status" | "getstatus" | "s" => match cbm.get_status(device) {
                        Ok(status) => println!("Status: {}", status),
                        Err(e) => println!("Error: {}", e),
                    },

                    "dir" | "d" => {
                        let drive_num = if cmd.len() > 1 {
                            Some(match cmd[1].parse::<u8>() {
                                Ok(num) if num <= 1 => num,
                                _ => {
                                    println!("Invalid drive number. Must be 0 or 1");
                                    continue;
                                }
                            })
                        } else {
                            None
                        };

                        match cbm.dir(device, drive_num) {
                            Ok(listing) => {
                                let drive_num = match drive_num {
                                    Some(dn) => dn,
                                    None => 0,
                                };
                                println!(
                                    "Directory listing for drive {:?}:\n{}",
                                    drive_num, listing
                                );
                            }
                            Err(e) => println!("Error reading directory: {}", e),
                        }
                    }

                    "reset" | "resetbus" | "busreset" | "r" | "b" => match cbm.reset_bus() {
                        Ok(()) => println!("Bus reset complete"),
                        Err(e) => println!("Error: {}", e),
                    },

                    "u" | "usbreset" | "resetusb" => 
                    /*match cbm.usb_device_reset() {
                        Ok(()) => println!("USB reset complete"),
                        Err(e) => println!("Error: {}", e),
                    }*/println!("Not currently supported"),

                    "command" | "cmd" | "c" => {
                        if cmd.len() < 2 {
                            println!("Usage: command <cmd-string>");
                            continue;
                        }
                        let cmd_str = cmd[1..].join(" ");
                        match cbm
                            .send_command(device, &CbmString::from_ascii_bytes(cmd_str.as_bytes()))
                        {
                            Ok(()) => {
                                println!("Command sent successfully");
                                // Get status after command
                                if let Ok(status) = cbm.get_status(device) {
                                    println!("Status: {}", status);
                                }
                            }
                            Err(e) => println!("Error: {}", e),
                        }
                    }

                    "format" | "f" => {
                        if cmd.len() != 3 {
                            println!("Usage: format <name> <id>");
                            continue;
                        }
                        let disk_name = AsciiString::from_ascii_str(cmd[1].as_str());
                        let disk_id = AsciiString::from_ascii_str(cmd[2].as_str());
                        match cbm.format_disk(device, &disk_name, &disk_id) {
                            Ok(()) => match cbm.get_status(device) {
                                Ok(status) => println!("Format complete: {}", status),
                                Err(e) => println!("Error during get_status after format: {}", e),
                            },
                            Err(e) => println!("Error during format: {}", e),
                        }
                    }

                    "load" | "l" => {
                        if cmd.len() < 2 {
                            println!("Usage: load \"file name\"  or  load filename ");
                            continue;
                        }

                        // Rejoin the remaining parts and trim any quotes
                        let filename_part = cmd[1..].join(" ");
                        let filename =
                            if filename_part.starts_with('"') && filename_part.ends_with('"') {
                                // Remove the surrounding quotes
                                filename_part[1..filename_part.len() - 1].to_string()
                            } else {
                                filename_part
                            };

                        let filename = AsciiString::from_ascii_str(&filename);
                        match cbm.load_file_ascii(device, &filename) {
                            Ok(file) => {
                                println!("Load of {filename} complete - length {}", file.len())
                            }
                            Err(e) => println!("Error: {}", e),
                        }
                    }

                    "print" | "p" => {
                        println!("Device number: {}", device);
                        println!("Verbosity:     {}", args.verbose);
                    }

                    "n" | "num" => {
                        device = if cmd.len() > 1 {
                            match cmd[1].parse::<u8>() {
                                Ok(num) if (DEVICE_MIN_NUM..=DEVICE_MAX_NUM).contains(&num) => {
                                    println!("Set device number to {}", num);
                                    num
                                }
                                _ => {
                                    println!("Invalid device number. Must be 8-15");
                                    continue;
                                }
                            }
                        } else {
                            println!("No device number supplied");
                            continue;
                        };
                    }

                    "help" | "h" | "?" => {
                        println!("Available commands:");
                        println!(
                            "  a|scan [min max]         - Scan for devices (optional range {}-{})",
                            DEVICE_MIN_NUM, DEVICE_MAX_NUM
                        );
                        println!("  i|id|identify            - Get device info");
                        println!("  s|status                 - Get device status");
                        println!(
                            "  d|dir [0|1]              - List directory (optional drive number)"
                        );
                        println!("  r|b|reset                - Reset the IEC bus");
                        println!("  u|usbreset               - Reset the USB device");
                        println!("  c|command <cmd>          - Send command to device");
                        println!("  f|format <name> <id>     - Format disk");
                        println!("  l|load <filename>        - Load file from disk");
                        println!("  p|print                  - Print config");
                        println!(
                            "  n|num {}-{}               - Change device number",
                            DEVICE_MIN_NUM, DEVICE_MAX_NUM
                        );
                        println!("  h|?|help                 - Show this help");
                        println!("  q|x|quit|exit            - Exit program");
                    }

                    _ => println!("Unknown command. Type 'help' for available commands."),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Ctrl-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Ctrl-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

//fn scan(cbm: &UsbCbm, min: u8, max: u8) {
fn scan<D: Device>(cbm: &Cbm<D>, min: u8, max: u8) {
    let devices = cbm.scan_bus_range(min..=max);
    if let Ok(devices) = devices {
        if devices.len() > 0 {
            for (id, info) in devices.iter() {
                println!(
                    "Found device {}: type: {} description: {}",
                    id, info.device_type, info.description
                );
            }
        } else {
            println!("No devices found with numbers {}-{}", min, max);
        }
    } else if let Err(e) = devices {
        println!("Hit fatal error scanning for devices: {e}");
    }
}
