//! remote.rs
//!
//! A simple example connecting to a remote xum1541, over IP.  See
//! [examples/cli](examples/cli.rs) for an example which dynamically
//! creates either local or remote xum1541, via the Cbm object.
//!
//! From the xum1541 project run
//!
//! `cargo run --bin device-server`
//!
//! Then run this to do a remote directory listing over the network
use rs1541::{Cbm, DeviceError, Error, Xum1541Error};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::exit;

fn main() -> Result<(), Error> {
    env_logger::init();
    log::info!("Started logging");

    // Driver automatically opens on creation and closes on drop
    let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1541);
    let cbm = match Cbm::new(None, Some(remote)) {
        Err(Error::Xum1541(error)) => match error {
            Xum1541Error::DeviceAccess { .. } | Xum1541Error::Usb(_) => {
                println!("Failed to connect to xum1541 device\nError: {error}");
                exit(1);
            }
            _ => {
                println!("Unexpected error from xum1541\nError: {error}");
                exit(1);
            }
        },
        Err(error) => {
            println!("Unexpected error from rs1541\nError: {error}");
            exit(1);
        }
        Ok(cbm) => cbm,
    };

    // Get drive information
    let id = cbm.identify(8);
    let id = match id {
        Err(Error::Device {
            error: DeviceError::NoDevice,
            ..
        }) => {
            println!("Device 8 not detected");
            std::process::exit(1);
        }
        Err(e) => return Err(e),
        Ok(id) => id,
    };
    println!("Drive type at device 8: {}", id);

    // Check drive status
    let status = cbm.get_status(8)?;
    println!("Drive status: {}", status);

    // Read directory
    let dir = cbm.dir(8, None)?;
    println!("Directory listing:\n{}", dir);

    Ok(())
}
