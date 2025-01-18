//! Basic Example
//!
//! Performs the following operations:
//! * Create Cbm object (which will also reset the bus)
//! * Identify the disk drive at device 8
//! * Get the status of device 8
//! * Read the directory listing of the inserted disk.
//! 
//! When run with a 1541, with the standard demo disk inserted, will output:
//! 
//! Drive type at device 8: CBM 1541: 1540 or 1541
//! Drive status: 00,OK,00,00
//! Directory listing:
//! Drive 0 Header: "test/demo  1/85" ID: 84
//! Filename: "how to use.prg"           Blocks:  14
//! Filename: "how part two.prg"         Blocks:   8
//! Filename: "how part three.prg"       Blocks:   7
//! Filename: "vic-20 wedge.prg"         Blocks:   4
//! Filename: "c-64 wedge.prg"           Blocks:   1
//! Filename: "dos 5.1.prg"              Blocks:   4
//! Filename: "printer test.prg"         Blocks:   9
//! Filename: "disk addr change.prg"     Blocks:   6
//! Filename: "view bam.prg"             Blocks:  12
//! Filename: "display t&s.prg"          Blocks:  15
//! Filename: "check disk.prg"           Blocks:   4
//! Filename: "performance test.prg"     Blocks:  11
//! Filename: "seq.file.demo.prg"        Blocks:   5
//! Filename: "rel.file.demo.prg"        Blocks:  18
//! Filename: "sd.backup.c16.prg"        Blocks:   7
//! Filename: "sd.backup.plus4.prg"      Blocks:   7
//! Filename: "sd.backup.c64.prg"        Blocks:  10
//! Filename: "print.64.util.prg"        Blocks:   7
//! Filename: "print.c16.util.prg"       Blocks:   7
//! Filename: "print.+4.util.prg"        Blocks:   7
//! Filename: "uni-copy.prg"             Blocks:  13
//! Filename: "c64 basic demo.prg"       Blocks:  30
//! Filename: "+4 basic demo.prg"        Blocks:  35
//! Filename: "load address.prg"         Blocks:   8
//! Filename: "unscratch.prg"            Blocks:   7
//! Filename: "header change.prg"        Blocks:   5
//! Free blocks: 403 

use rs1541::Cbm;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Driver automatically opens on creation and closes on drop
    let cbm = Cbm::new()?;

    // Get drive information
    let id = cbm.identify(8)?;
    println!("Drive type at device 8: {}", id);

    // Check drive status
    let status = cbm.get_status(8)?;
    println!("Drive status: {}", status);

    // Read directory
    let dir = cbm.dir(8, None)?;
    println!("Directory listing:\n{}", dir);

    Ok(())
}