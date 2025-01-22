//! Format Example
//!
//! ⚠️ **Warning:** Formats the disk in the drive, unless write protected
//!
//! Formats a disk and then returns the directory listing

use rs1541::{AsciiString, Cbm, CbmError};

fn main() -> Result<(), CbmError> {
    // Driver automatically opens on creation and closes on drop
    let cbm = Cbm::new()?;

    let disk_name = "formatted disk";
    let disk_id = "aa";
    println!("Formatting disk as: {},{}", disk_name, disk_id);

    // Returns Ok(CbmStatus) if the format command was successfully sent to
    // the disk, but the format itself may fail - in which case CbmStatus
    // will contain the error code reported by the drive.
    let ascii_disk_name = AsciiString::from_ascii_str(disk_name);
    let ascii_disk_id = AsciiString::from_ascii_str(disk_id);
    cbm.format_disk(8, &ascii_disk_name, &ascii_disk_id)?;
    let status = cbm.get_status(8)?; 
    println!("Drive status after formatting: {}", status);

    // Read directory
    let dir = cbm.dir(8, None)?;
    println!("Directory listing:\n{}", dir);

    Ok(())
}
