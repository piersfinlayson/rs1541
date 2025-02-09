use rs1541::{Cbm, Error};

fn main() -> Result<(), Error> {
    // Driver automatically opens on creation and closes on drop
    let cbm = Cbm::new(None, None)?;

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
