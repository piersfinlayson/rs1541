use clap::Parser;
use log::{info, LevelFilter};
use rs1541::{AsciiString, Cbm, Rs1541Error, CbmString, MIN_DEVICE_NUM, MAX_DEVICE_NUM};
use rustyline::{error::ReadlineError, DefaultEditor};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Device number to test (8-15)
    #[arg(short, long, default_value_t = 8)]
    device: u8,

    /// Verbosity level (-v for Info, -vv for Debug, -vvv for Trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() -> Result<(), Rs1541Error> {
    let args = Args::parse();
    let mut device = args.device;

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

    // Create CBM interface
    let mut cbm = Cbm::new()?;

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

                    "scan" | "a" => for id in MIN_DEVICE_NUM..=MAX_DEVICE_NUM {
                        match cbm.identify(id) {
                            Ok(info) => {
                                println!(
                                    "Found device {}: type: {} description: {}",
                                    id, info.device_type, info.description);
                            },
                            Err(e) => match e {
                                Rs1541Error::Device{ .. } => {
                                    // We this error if the device doesn't exist
                                },
                                e => {
                                    println!("Error: {}", e);
                                },
                            },  
                        }
                    },

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

                    "u" | "usbreset" | "resetusb" => match cbm.usb_device_reset() {
                        Ok(()) => println!("USB reset complete"),
                        Err(e) => println!("Error: {}", e),
                    },

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
                                Ok(num) if (8..=15).contains(&num) => {
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
                        println!("  a|scan                   - Scan for devices");
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
                        println!("  n|num 8-15               - Change device number");
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
