//! Format Example
//!
//! ⚠️ **Warning:** Formats the disk in the drive, unless write protected
//!
//! Formats a disk and then returns the directory listing

use rs1541::Cbm;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Driver automatically opens on creation and closes on drop
    let cbm = Cbm::new()?;

    let disk_name = "formatted disk";
    let disk_id = "aa";
    println!("Formatting disk as: {},{}", disk_name, disk_id);

    // Returns Ok(CbmStatus) if the format command was successfully sent to
    // the disk, but the format itself may fail - in which case CbmStatus
    // will contain the error code reported by the drive.
    let status = cbm.format_disk(8, disk_name, disk_id)?;
    println!("Drive status after formatting: {}", status);

    // Read directory
    let dir = cbm.dir(8, None)?;
    println!("Directory listing:\n{}", dir);

    Ok(())
}
