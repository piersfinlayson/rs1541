//! A Rust interface for interacting with Commodore disk drives.
//!
//! This module provides a wrapper around the xum1541 library,
//! allowing modern systems to interact with Commodore disk drives (like the 1541)
//! through XUM1541-compatible USB adapters.
//!
//! # Architecture
//!
//! The module is structured around several key abstractions:
//!
//! - [`Cbm`]: The main interface for interacting with drives. This struct provides
//!   high-level operations like reading files, writing files, and getting directory
//!   listings.
//!
//! - [`CbmDriveUnit`]: Represents a physical drive unit, managing its channels and state.
//!   A drive unit may contain one or two drives (like the 1541 vs 1571).
//!
//! - [`CbmChannel`]: Represents a communication channel to a drive. CBM drives use a
//!   channel-based communication system, with 16 channels (0-15) available per drive.
//!   Channel 15 is reserved for commands.
//!
//! # Safety and Threading
//!
//! The module is designed with safety and thread-safety in mind:
//!
//! - All operations that could fail return [`Result`]s with detailed error types
//! - The xum1541 handle is protected by a mutex to allow safe multi-threaded access
//! - Channel allocation is managed to prevent conflicts and ensure proper cleanup
//!
//! # Example Usage
//!
//! ```ignore
//! use your_crate_name::Cbm;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a new CBM instance
//!     let cbm = Cbm::new()?;
//!
//!     // Get directory listing from drive 8
//!     let dir = cbm.dir(8, None)?;
//!     println!("{}", dir);
//!
//!     // Read a file
//!     let data = cbm.read_file(8, "MYPROGRAM.PRG")?;
//!
//!     // Write a file
//!     cbm.write_file(8, "NEWFILE.PRG", &data)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Error Handling
//!
//! The module uses a custom error type [`Error`] that covers various failure modes:
//!
//! - Device errors (drive not responding, hardware issues)
//! - File operation errors (file not found, disk full)
//! - Command errors (invalid command, drive error status)
//! - USB communication errors
//!
//! # File System Operations
//!
//! Commodore drives use a different character set (PETSCII) and have specific file
//! naming conventions. This module handles the translation between ASCII and PETSCII
//! automatically. File operations support:
//!
//! - Reading and writing files
//! - Deleting files
//! - Getting directory listings
//! - Formatting disks
//! - Validating disk contents
//!
//! # Device Management
//!
//! The module supports:
//!
//! - Multiple drive types (1541, 1571, 1581, etc.)
//! - Device detection and identification
//! - Status monitoring
//! - Bus reset and device reset operations
//!
//! # Channel Management
//!
//! The module includes a sophisticated channel management system that:
//!
//! - Automatically allocates channels for operations
//! - Prevents channel conflicts
//! - Ensures proper cleanup after use
//! - Reserves channel 15 for command operations
//!
//! # Performance Considerations
//!
//! Communication with Commodore drives is inherently slow due to the bus design.
//! Operations are performed synchronously, and large file transfers can take
//! significant time. The module provides proper error handling and status updates
//! to help manage these limitations.
//!
//! # Limitations
//!
//! ⚠️ **Warning:** Some functions are not yet tested/implemented - use at your own
//! risk!
//!
//! - Requires compatible USB hardware (XUM1541)
//! - Operations are synchronous
//! - Some advanced 1571/1581 features may not be supported
//! - Drive/DOS commands are limited to standard CBM DOS operations
//!
use crate::channel::{CBM_CHANNEL_CTRL, CBM_CHANNEL_LOAD};
use crate::string::{AsciiString, PetsciiString};
use crate::validate::{validate_device, DeviceValidation};
use crate::{
    BusGuardMut, BusGuardRef, CbmDeviceInfo, CbmDirListing, CbmErrorNumberOk, CbmStatus, CbmString,
    DeviceError, Error, MAX_DEVICE_NUM,
};
use crate::disk::BYTES_PER_BLOCK;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;
use xum1541::constants::MIN_DEVICE_NUM;
use xum1541::Error as Xum1541Error;
use xum1541::{Bus, BusBuilder, CommunicationKind, DeviceChannel};

use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

/// The main interface for interacting with Commodore disk drives via an XUM1541.
///
/// `Cbm` provides a high-level interface to communicate with Commodore disk drives
/// through the xum1541 crate. It manages the driver connection and provides methods
/// for common disk operations like reading files, writing files, and getting directory
/// listings.
///
/// The struct uses interior mutability (via `Arc<Mutex<>>`) to allow safe concurrent
/// access to the XUM1541 driver while maintaining a clean API that doesn't require
/// explicit locking by the user.
///
/// # Example
///
/// ```ignore
/// use your_crate_name::Cbm;
///
/// let cbm = Cbm::new()?;
///
/// // Get directory listing
/// let dir = cbm.dir(8, None)?;
/// println!("{}", dir);
///
/// // Read a file
/// let data = cbm.read_file(8, "MYFILE.PRG")?;
/// ```
#[derive(Debug, Clone)]
pub struct Cbm {
    handle: Arc<Mutex<Option<Bus>>>,
}

/// Functions to manage this and the Bus object
impl Cbm {
    /// Creates a new CBM instance and opens the XUM1541 driver.
    ///
    /// This function attempts to initialize communication with the XUM1541 driver
    /// and returns a wrapped handle that can be used for further operations.
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The driver cannot be opened
    /// - No XUM1541 device is connected
    /// - The device is in use by another process
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// ```
    pub fn new() -> Result<Self, Error> {
        trace!("Cbm::new");
        let mut bus = BusBuilder::new().build()?;
        bus.initialize()?;

        Ok(Self {
            handle: Arc::new(Mutex::new(Some(bus))),
        })
    }

    /// Resets the USB device connection - by closing the driver then reopening
    /// which in turn will force a device reset
    ///
    /// This is a potentially risky operation that should be used with caution.
    /// If it returns `Error::DriverNotOpen`, the xum1541 driver may need to
    /// be reopened with a new `Cbm` instance.
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The driver is not open
    /// - The USB reset operation fails
    /// - The handle is invalid
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut cbm = Cbm::new()?;
    /// cbm.usb_device_reset()?;
    /// ```
    pub fn usb_device_reset(&mut self) -> Result<(), Error> {
        // Lock the old handle - will be unlocked when it goes out of scope
        let mut handle = self.handle.lock();

        // Drop the old Bus instance which will close the driver
        let old_bus = handle.take();
        drop(old_bus);

        // Create a new instance (can fail)
        let mut new_bus = BusBuilder::new().build()?;
        new_bus.initialize()?;

        // Set the stored handle to the new instance
        *handle = Some(new_bus);

        Ok(())
    }

    /// Resets the entire IEC bus.
    ///
    /// This operation affects all devices on the bus and should be used sparingly.
    /// It's primarily useful when devices have gotten into an inconsistent state.
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The driver is not open
    /// - The bus reset operation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// cbm.reset_bus()?;
    /// ```
    pub fn reset_bus(&self) -> Result<(), Error> {
        self.handle.lock().bus_mut_or_err()?.reset()?;
        Ok(())
    }
}

/// Simple high level drive-access functions
impl Cbm {
    /// Identifies a device on the IEC bus.
    ///
    /// Queries the specified device to determine its type (1541, 1571, etc.)
    /// and capabilities.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The device doesn't respond
    /// - The device returns invalid identification data
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// let info = cbm.identify(8)?;
    /// println!("Device type: {}", info.device_type);
    /// ```
    pub fn identify(&self, device: u8) -> Result<CbmDeviceInfo, Error> {
        // Issue a memory read of two bytes at address 0xff40
        // For compatibility with DOS1 drives we'll only read 1 byte at a time
        // (With DOS 2 we could pass in another byte to ask for 2 bytes)
        let mut buf = [0u8; 2];
        self.read_drive_memory(device, 0xff40, &mut buf)?;
        let magic: u16 = ((buf[1] as u16) << 8) | (buf[0] as u16);

        // Need to do some extra work for some drives
        let magic2 = match magic {
            0xaaaa => {
                // 1540 and 1541 variants
                let mut buf = [0u8; 2];
                self.read_drive_memory(device, 0xfffe, &mut buf)?;
                if buf[0] != 0x67 || buf[1] != 0xFE {
                    Some(((buf[1] as u16) << 8) | (buf[0] as u16))
                } else {
                    // Read another 2 bytes in order to diffentiate between
                    // the 1540 and 1541.  The 1540 has 0x56 then 0x31 at
                    // 0xE5C4 (V1 in ascii) and the 1541 has 0x31 then 0x35
                    // for 15 (both short for V170 and 1541 - the firmware
                    // version string exposed in status after reset)
                    //implement
                    let mut buf = [0u8; 2];
                    self.read_drive_memory(device, 0xe5c4, &mut buf)?;
                    Some(((buf[1] as u16) << 8) | (buf[0] as u16))
                }
            }
            0x01ba => {
                // 1581 and FDX000 (3.5" drives)
                let mut buf = [0u8; 2];
                self.read_drive_memory(device, 0xfffe, &mut buf)?;
                let magic2: u16 = ((buf[1] as u16) << 8) | (buf[0] as u16);
                Some(magic2)
            }
            _ => None,
        };

        let device_info = CbmDeviceInfo::from_magic(magic, magic2);

        // Generate the device type from the magic number(s)
        Ok(device_info)
    }

    /// Gets the status of a device.
    ///
    /// This function retrieves the current status message from the device,
    /// which includes error conditions and drive state.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The device doesn't respond
    /// - The status message cannot be read
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// let status = cbm.get_status(8)?;
    /// println!("Drive status: {}", status);
    /// ```
    pub fn get_status(&self, device: u8) -> Result<CbmStatus, Error> {
        let mut guard = self.handle.lock();
        let mut bus = (&mut guard).bus_mut_or_err()?;

        Self::get_status_locked(&mut bus, device)
    }

    /// Scan the bus for any devices
    ///
    /// This function will scan the entire possible range of devices,
    /// from 8 to 30.  If a device doesn't exist it takes about 1s to
    /// detect this.
    ///
    /// If you want to reduce the amount of time taken and scan a partial
    /// range, use [`Cbm::scan_bus_range`] instead.
    ///
    /// # Returns
    /// - `HashMap<u8, CbmDevInfo>` - if successful
    /// - `Error`` - if a serious error occured.  If a Error::Device error occurs, this is treated as non fatal and the query will continue
    ///
    /// # Examples
    /// ```ignore
    /// let devices = cbm.scan_bus()?;
    /// ```
    pub fn scan_bus(&self) -> Result<HashMap<u8, CbmDeviceInfo>, Error> {
        self.scan_bus_range(MIN_DEVICE_NUM..=MAX_DEVICE_NUM)
    }

    /// Scans the bus for devices within the range specified
    ///
    /// # Arguments
    /// - `range` - The inclusive range to scan for
    ///
    /// - `HashMap<u8, CbmDevInfo>` - if successful
    /// - `Error`` - if a serious error occured.  If a Error::Device error occurs, this is treated as non fatal and the query will continue
    ///
    /// # Examples
    /// ```ignore
    /// let devices = cbm.scan_bus()?;
    /// ```
    ///
    /// # Note
    /// See [`Cbm::scan_bus`] for more
    pub fn scan_bus_range(
        &self,
        range: RangeInclusive<u8>,
    ) -> Result<HashMap<u8, CbmDeviceInfo>, Error> {
        // Make return variable
        let mut devices = HashMap::new();

        // Check each device number
        for device in range {
            if self.drive_exists(device)? {
                debug!("Device {device} exists - identify it");
                let info = match self.identify(device) {
                    // If hit a device error continue, otherwise propagate
                    Err(Error::Device { .. }) => {
                        warn!("Hit error identifying device {device} status");
                        continue;
                    }
                    Err(e) => return Err(e),
                    Ok(info) => info,
                };
                devices.insert(device, info);
            } else {
                trace!("Device {device} doesn't exist");
            }
        }

        Ok(devices)
    }

    /// Detects whether the specified device actually exists
    pub fn drive_exists(&self, device: u8) -> Result<bool, Error> {
        let dc = DeviceChannel::new(device, CBM_CHANNEL_CTRL)?;
        match self.bus_listen(dc) {
            Err(Error::Xum1541(Xum1541Error::Communication {
                kind: CommunicationKind::StatusValue { .. },
            })) => {
                debug!("Device {device} doesn't exist");
                Ok(false)
            }
            Err(e) => Err(e.into()),
            Ok(_) => {
                self.bus_unlisten()?;
                Ok(true)
            }
        }
    }

    /// Gets a directory listing from the device.
    ///
    /// Returns a structured representation of the disk directory, including
    /// disk name, file entries, and blocks free.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `drive_num` - Optional drive number (0 or 1) for dual drives
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The device doesn't respond
    /// - The directory cannot be read
    /// - The directory format is invalid
    /// - The driver is not open
    /// - The drive number is invalid (>1)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    ///
    /// // Get directory from first drive
    /// let dir = cbm.dir(8, Some(0))?;
    ///
    /// // Print directory contents
    /// println!("Disk name: {}", dir.header.name);
    /// for file in &dir.files {
    ///     println!("{}", file);
    /// }
    /// println!("Blocks free: {}", dir.blocks_free);
    /// ```
    /// Get directory listing, converting filenames from PETSCII to ASCII
    pub fn dir(&self, device: u8, drive_num: Option<u8>) -> Result<CbmDirListing, Error> {
        // Validate drive_num
        if let Some(drive_num) = drive_num {
            if drive_num > 1 {
                return Err(DeviceError::invalid_drive_num(device, drive_num));
            }
        }

        // Construct directory command ("$" or "$0" or "$1")
        let filename = match drive_num {
            Some(num) => PetsciiString::from_petscii_bytes(&[b'$', num]),
            None => PetsciiString::from_petscii_bytes(&[b'$']),
        };

        let dir_data = self.load_file_petscii(device, &filename)?;

        // Process it
        let mut cursor = 0;

        // Skip the load address (first two bytes)
        cursor += 2;

        let mut output = String::new();

        if dir_data.len() >= 2 {
            // Read directory entries
            while cursor + 4 <= dir_data.len() {
                // Need at least 4 bytes for link and size
                // Read link address (2 bytes)
                cursor += 2; // Skip link address as we don't use it

                // Read file size (2 bytes, little endian)
                let size = (dir_data[cursor] as u16) | ((dir_data[cursor + 1] as u16) << 8);
                cursor += 2;

                output.push_str(&format!("{:4} ", size));

                // Read filename characters until 0 byte or end of data
                let mut filename = Vec::new();
                while cursor < dir_data.len() && dir_data[cursor] != 0 {
                    filename.push(dir_data[cursor]);
                    cursor += 1;
                }
                cursor += 1; // Skip the terminating 0 byte

                let petscii_filename = PetsciiString::from_petscii_bytes(&filename);
                let ascii_filename: AsciiString = petscii_filename.into();
                let str_filename = ascii_filename.to_string();
                output.push_str(&str_filename);
                output.push('\n');

                // Break if we've reached the end of the data
                if cursor >= dir_data.len() {
                    break;
                }
            }
        }

        CbmDirListing::parse(&output)
    }

    /// Validates the disk contents.
    ///
    /// This operation checks the Block Availability Map (BAM) and can recover
    /// some types of disk errors.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The device doesn't respond
    /// - The validation fails
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// cbm.validate_disk(8)?;
    /// ```
    pub fn validate_disk(&self, device: u8) -> Result<(), Error> {
        // Send validate command (V)
        self.send_command_petscii(device, &PetsciiString::from_ascii_str("v"))?;

        // Check status after validation
        self.get_status(device)?.into()
    }

    /// Deletes a file from the disk.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `filename` - Name of the file to delete as ascii
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The file doesn't exist
    /// - The file cannot be deleted
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// cbm.delete_file(8, "OLDFILE.PRG")?;
    /// ```
    pub fn delete_file(&self, device: u8, filename: &AsciiString) -> Result<(), Error> {
        // Construct scratch command (S:filename)
        let cmd = format!("s0:{}", filename);
        self.send_string_command_ascii(device, &cmd)?;
        self.get_status(device)?.into()
    }

    /// Formats a disk.
    ///
    /// Formats the disk in the specified drive with the given name and ID.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `name` - Name for the disk (max 16 characters)
    /// * `id` - Two-character disk ID
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The device doesn't respond
    /// - The format operation fails
    /// - The ID is not exactly 2 characters
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// cbm.format_disk(8, "MY DISK", "01")?;
    /// ```
    /// Format a disk with ASCII name and ID
    pub fn format_disk(
        &self,
        device: u8,
        name: &AsciiString,
        id: &AsciiString,
    ) -> Result<(), Error> {
        // Validate ID length
        let id_len = id.as_bytes().len();
        if id_len != 2 {
            return Err(Error::Validation {
                message: format!(
                    "Device {device} format error: Disk ID must be 2 characters, not {id_len}"
                ),
            });
        }

        // Construct format command (N:name,id)
        let cmd = format!("n0:{},{}", name, id);

        trace!("Send format command in ascii {}", cmd);
        self.send_string_command_ascii(device, &cmd)?;
        self.get_status(device)?.into()
    }
}

/// Lower level public API
impl Cbm {
    /// Function to read a number of consecutive bytes from a drive
    ///
    /// Currently only reads one byte at a time for DOS1 compatibility
    /// Returns an error if couldn't read the requested number of bytes
    /// Will wrap around from 0xffff to 0x0000 and continue if necessary
    ///
    /// # Arguments
    /// - `device` - Device number to read from
    /// - `addr` - [`u16`]` indicating which address to read from
    /// - `buf` - buffer to read into, and the size of this buffer controls how many bytes will be read (one at a time)
    ///
    /// # Returns
    /// - `()` - is successful
    /// - `Error` - if an error occurs
    ///
    /// Returns an error if couldn't read all request bytes.
    ///
    /// Note that the M-R command leaves the drive in a bad state.  The 1571
    /// manual states, page 75, "Any #INPUT from the error channel will give
    /// peculiar results when you're using this command.  This can be cleared
    /// up by sending any other command to the disk, except another memory
    /// command".
    ///
    /// As there are few commands which don't cause some sort of physical
    /// actions or state change on the drive, immediately after doing an M-R
    /// we retrieve the status, expecting it to fail (it will likely return)
    /// a single byte - lik `\r`.
    pub fn read_drive_memory(&self, device: u8, addr: u16, buf: &mut [u8]) -> Result<(), Error> {
        let size = buf.len();
        trace!("Cbm::read_drive_memory: device {device} addr 0x{addr:04x} size {size}");

        // Validate arguments
        Self::validate_read_args(
            size,
            format!("Asked to read 0 bytes from device {device} memory address 0x{addr:04x}"),
        )?;

        // Split address into low and high bytes
        let mut addr_low = (addr & 0xFF) as u8;
        let mut addr_high = ((addr >> 8) & 0xFF) as u8;

        // We need to get the Bus lock for the whole time we're doing stuff
        // as the disk drive will be in a "peculiar" state, during and after
        // our memory read.
        {
            let mut guard = self.handle.lock();
            let bus = (&mut guard).bus_mut_or_err()?;

            let result = (|| {
                // Read one byte at a time for DOS1 compatibility
                let mut cmd = [b'M', b'-', b'R', addr_low, addr_high];
                let dc = DeviceChannel::new(device, CBM_CHANNEL_CTRL)?;
                for ii in 0..size {
                    debug!("Read from memory address 0x{addr_high:02x}{addr_low:02x}");
                    Self::send_command_petscii_locked(
                        bus,
                        dc,
                        &PetsciiString::from_petscii_bytes(&cmd),
                    )?;

                    Self::read_from_drive_locked(bus, dc, &mut buf[ii..ii + 1], true)?;

                    debug!("Read data: 0x{:02x}", buf[ii]);

                    // Increment and handle 16-bit address wraparound
                    if ii < size - 1 {
                        addr_low = addr_low.wrapping_add(1);
                        if addr_low == 0 {
                            addr_high = addr_high.wrapping_add(1);
                        }
                        cmd[3] = addr_low;
                        cmd[4] = addr_high;
                    }
                }
                Ok(())
            })();

            // Always perform cleanup regardless of the operation result
            trace!("Read status in order to clear effects of M-R command");
            match Self::get_status_locked(bus, device) {
                Ok(status) => debug!("Unexpectedly got status OK after M-R command {status} "),
                Err(Error::Parse { message }) => {
                    trace!("Got expectedly bad status when reading status after M-R: {message}")
                }
                Err(e) => {
                    let default_error = DeviceError::get_status_failure(
                        device,
                        format!("Failed to get status after identify: {e}"),
                    );
                    return Err(match e {
                        Error::Device { device, error } => match error {
                            DeviceError::NoDevice => DeviceError::no_device(device),
                            _ => default_error,
                        },
                        _ => default_error,
                    });
                }
            }

            // Now propagate the result from the main operation
            result
        }
    }

    /// Writes the required number of bytes to the device's memory
    pub fn write_drive_memory(&self, device: u8, addr: u16, data: &[u8]) -> Result<(), Error> {
        // Split address into low and high bytes
        let addr_low = (addr & 0xFF) as u8;
        let addr_high = ((addr >> 8) & 0xFF) as u8;

        // Write one byte at a time for DOS1 compatibility
        for (i, &byte) in data.iter().enumerate() {
            let cmd = vec![
                b'M',
                b'-',
                b'W',
                addr_low.wrapping_add(i as u8),
                addr_high,
                byte,
            ];
            self.send_command_petscii(device, &PetsciiString::from_petscii_bytes(&cmd))?;

            let mut guard = self.handle.lock();
            let bus = (&mut guard).bus_mut_or_err()?;

            let dc = DeviceChannel::new(device, CBM_CHANNEL_CTRL)?;
            bus.talk(dc)?;

            // TODO - actually write the byte

            bus.untalk()?;
        }

        Ok(())
    }

    /// Send a command on a specific drive
    ///
    /// There are a number of different variants of this function that allow
    /// different types of strings to be passed in.  send_command() is likely
    /// to be the easiest to use, as it accepts a CbmString, which is an enum
    /// allowing ASCII or PETSCII strings to be passed in.
    ///
    /// # Example
    /// ```ignore
    /// let cmd_str = String("n0:formatted,aa");
    /// cbm.send_command(device, &CbmString::from_ascii_bytes(cmd_str.as_bytes()));
    /// ```
    pub fn send_command(&self, device: u8, cmd: &CbmString) -> Result<(), Error> {
        self.send_command_petscii(device, &cmd.to_petscii())
    }

    /// Send a command on a specific drive
    /// The command must be provided as a PetsciiString
    pub fn send_command_petscii(&self, device: u8, cmd: &PetsciiString) -> Result<(), Error> {
        trace!("Cbm::send_command_petscii device {device} cmd {cmd}");
        let dc = DeviceChannel::new(device, CBM_CHANNEL_CTRL)?;

        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        Self::send_command_petscii_locked(bus, dc, cmd)
    }

    /// Sends a command string to a device after converting from ASCII to PETSCII
    pub fn send_command_ascii(&self, device: u8, command: &AsciiString) -> Result<(), Error> {
        let petscii: PetsciiString = command.into();
        trace!("Send string command in petscii {}", petscii);
        self.send_command_petscii(device, &petscii)
    }

    /// Sends a string command to a device, converting from ASCII to PETSCII.
    /// The input string must be ASCII-compatible.
    ///
    /// # Errors
    /// Returns `Error` if:
    /// - The string contains non-ASCII characters
    /// - The device command fails
    pub fn send_string_command_ascii(&self, device: u8, command: &str) -> Result<(), Error> {
        let ascii = AsciiString::try_from(command).map_err(|e| Error::Validation {
            message: format!("Unable to parse requested command as ASCII {command}: {e}"),
        })?;
        trace!("Send string command in ascii {}", ascii);
        self.send_command_ascii(device, &ascii)
    }

    /// Sends a string command that is already in PETSCII format.
    /// The input string bytes must be valid PETSCII.
    ///
    /// # Errors
    /// Returns `Error` if the device command fails
    pub fn send_string_command_petscii(&self, device: u8, command: &str) -> Result<(), Error> {
        self.send_command_petscii(
            device,
            &PetsciiString::from_petscii_bytes(command.as_bytes()),
        )
    }

    fn validate_read_args(size: usize, message: String) -> Result<(), Error> {
        if size == 0 {
            warn!("Asked to read {size} bytes: {message}");
            Err(Error::Validation { message })
        } else {
            Ok(())
        }
    }

    /// Instructs the device to talk, reads the requested number of bytes
    /// then sets the device to untalk.
    /// In case of a failure, sets the device to untalk (if possible) before
    /// returning
    /// read_all - if set to True, returns an Error if all requested bytes
    /// not read
    pub fn read_from_drive(
        &self,
        dc: DeviceChannel,
        buf: &mut [u8],
        read_all: bool,
    ) -> Result<usize, Error> {
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        Self::read_from_drive_locked(bus, dc, buf, read_all)
    }

    /// Reads a file from the disk.
    ///
    /// Reads the entire contents of the specified file into a vector of bytes.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `filename` - Name of the file to read in ascii
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The file doesn't exist
    /// - The file cannot be opened
    /// - A read error occurs
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// let data = cbm.read_file(8, "MYPROGRAM.PRG")?;
    /// ```
    /// Read a file with ASCII filename
    pub fn read_file(&self, device: u8, filename: &AsciiString) -> Result<Vec<u8>, Error> {
        let dc = {
            let _bus = self.handle.lock().bus_ref_or_err()?;

            // TO DO properly alllocate channels
            DeviceChannel::new(device, 2)?
        };

        self.send_command_ascii(device, filename)?;

        // Check status after open
        let status = self.get_status(device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        // Re-acquire guard for file operations
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        // Now read the file data
        bus.talk(dc).map_err(|e| Error::File {
            device,
            message: format!("Talk failed: {}", e),
        })?;

        let mut data = Vec::new();
        loop {
            let buf = &mut [0u8; BYTES_PER_BLOCK];
            let count = Self::bus_read_locked(bus, dc, buf).map_err(|e| Error::File {
                device,
                message: format!("Read failed: {}", e),
            })?;

            data.extend_from_slice(&buf[..count as usize]);
            if count < BYTES_PER_BLOCK {
                debug!("Finished reading file");
                break;
            }
        }

        // Cleanup
        bus.untalk().map_err(|e| Error::File {
            device,
            message: format!("Untalk failed: {}", e),
        })?;

        Ok(data)
    }

    /// Writes a file to the disk.
    ///
    /// Creates or overwrites a file with the specified data.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `filename` - Name for the file in ascii
    /// * `data` - The data to write
    ///
    /// # Errors
    ///
    /// Returns `Error` if:
    /// - The disk is full
    /// - The file cannot be created
    /// - A write error occurs
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// let data = vec![0x01, 0x08, 0x0C, 0x08, 0x0A, 0x00];
    /// cbm.write_file(8, "NEWFILE.PRG", &data)?;
    /// ```
    pub fn write_file(&self, device: u8, filename: &AsciiString, data: &[u8]) -> Result<(), Error> {
        let dc = {
            let _bus = self.handle.lock().bus_ref_or_err()?;

            // TO DO properly allocate channels
            DeviceChannel::new(device, 2)?
        };

        // Open file for writing with overwrite if exists
        self.send_string_command_ascii(device, &format!("@:{}", filename))?;

        // Check status after open
        let status = self.get_status(device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        // Reacquire guard for file operations
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        // Now write the file data
        bus.listen(dc).map_err(|e| Error::File {
            device,
            message: format!("Listen failed: {}", e),
        })?;

        // Write data in chunks
        for chunk in data.chunks(BYTES_PER_BLOCK) {
            let result = bus.write(chunk).map_err(|e| Error::File {
                device,
                message: format!("Write failed: {}", e),
            })?;

            if result != chunk.len() {
                return Err(Error::File {
                    device,
                    message: "Failed to write complete chunk".into(),
                });
            }
        }

        // Cleanup
        bus.unlisten().map_err(|e| Error::File {
            device,
            message: format!("Unlisten failed: {}", e),
        })?;

        Ok(())
    }

    /// Open a file using an ASCII filename
    ///
    /// This function will convert the ASCII filename to PETSCII, and will
    /// check that the status of the drive is OK after the sequence.  If
    /// an error occurs during opening this function will clean-up.
    ///
    /// # Arguments
    /// `dc` - The device and channel to open the file on
    /// `filename` - The filename to open in ASCII format
    ///
    /// # Returns
    /// `()` - if successful
    /// `Error` - if an error occurs
    ///
    /// Note this function must be folllowed by the close for this device
    /// and channel
    pub fn open_file(&self, dc: DeviceChannel, filename: &AsciiString) -> Result<(), Error> {
        let petscii_name: PetsciiString = filename.into();

        {
            let mut guard = self.handle.lock();
            let bus = (&mut guard).bus_mut_or_err()?;

            Self::open_file_petscii_locked(bus, dc, &petscii_name)
        }
    }

    /// Close a file that was previously opened
    ///
    /// # Arguments
    ///
    /// `dc` - The device and channel to open the file on
    /// `filename` - The filename to open in ASCII format
    ///
    /// # Returns
    /// `()` - if successful
    /// `Error` - if an error occurs
    ///
    /// Note that this function must have been preceeded by an open_file()
    /// call for this device and channel
    pub fn close_file(&self, dc: DeviceChannel) -> Result<(), Error> {
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        Self::close_file_locked(bus, dc)
    }

    /// This function opens a file, reads in the entire contents and closes
    /// the file.
    ///
    /// The contents are read into a buffer allocated by this file, and
    /// ownership is passed to the caller.
    ///
    /// # Arguments
    /// * `device` - Device number
    /// * `filename` - Filename to open in PETSCII format (lower case characters for regular character-based filenames).  Does not include suffix or file type
    pub fn load_file_petscii(
        &self,
        device: u8,
        filename: &PetsciiString,
    ) -> Result<Vec<u8>, Error> {
        // Validate device
        validate_device(Some(device), DeviceValidation::Required)?;

        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        Self::load_file_petscii_locked(bus, device, &filename)
    }

    /// This function opens a file, reads in the entire contents and closes
    /// the file.
    ///
    /// The contents are read into a buffer allocated by this file, and
    /// ownership is passed to the caller.
    ///
    /// # Arguments
    /// * `device` - Device number
    /// * `filename` - Filename to open in ASCII format (lower case characters for regular character-based filenames).  Does not include suffix or file type
    pub fn load_file_ascii(&self, device: u8, filename: &AsciiString) -> Result<Vec<u8>, Error> {
        trace!("Cbm::load_file device: {device} filename: {filename}");

        // Convert filename to petscii
        let filename: PetsciiString = filename.into();

        // Load the file
        self.load_file_petscii(device, &filename)
    }

    fn load_file_petscii_locked(
        bus: &mut Bus,
        device: u8,
        filename: &PetsciiString,
    ) -> Result<Vec<u8>, Error> {
        debug!("Load file device: {device} filename: {filename}");

        // Open the file
        let dc = DeviceChannel::new(device, CBM_CHANNEL_LOAD)?;
        Self::open_file_petscii_locked(bus, dc, filename)?;

        // Talk
        bus.talk(dc).inspect_err(|_| {
            // Clean-up
            let _ = Self::close_file_locked(bus, dc);
        })?;

        // Read in BYTES_PER_BLOCK byte chunks
        let mut buffer = Vec::new();
        let mut read_buf = [0u8; BYTES_PER_BLOCK];
        let read_result = loop {
            match Self::bus_read_locked(bus, dc, &mut read_buf) {
                Ok(bytes_read) if bytes_read == 0 => break Ok(buffer),
                Ok(bytes_read) => buffer.extend_from_slice(&read_buf[..bytes_read]),
                Err(e) => {
                    // Clean-up
                    let _ = bus.untalk();
                    let _ = Self::close_file_locked(bus, dc);
                    break Err(e);
                }
            }
        }?;

        // Untalk
        bus.untalk().inspect_err(|_| {
            // Clean-up
            let _ = Self::close_file_locked(bus, dc);
        })?;

        // Close file
        Self::close_file_locked(bus, dc)?;

        Ok(read_result)
    }
}

/// Internal functions
impl Cbm {
    fn bus_listen(&self, dc: DeviceChannel) -> Result<(), Error> {
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        bus.listen(dc).map_err(|e| e.into())
    }

    #[allow(dead_code)]
    fn bus_unlisten(&self) -> Result<(), Error> {
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        bus.unlisten().map_err(|e| e.into())
    }

    #[allow(dead_code)]
    fn bus_talk(&self, dc: DeviceChannel) -> Result<(), Error> {
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        bus.talk(dc).map_err(|e| e.into())
    }

    #[allow(dead_code)]
    fn bus_untalk(&self) -> Result<(), Error> {
        let mut guard = self.handle.lock();
        let bus = (&mut guard).bus_mut_or_err()?;

        bus.untalk().map_err(|e| e.into())
    }

    // Handles figuring out if the read response means that we actually
    // have no device.
    //
    // Our current way of figuring this out is if the bus is talking on
    // channel 15 and if a read response gave 0 bytes
    //
    // We could also check whether the listener is the DeviceChannel
    // passed into us, but we won't bother with that.
    fn handle_read_result(
        result: Result<usize, Xum1541Error>,
        bus: &Bus,
        dc: DeviceChannel,
    ) -> Result<usize, Error> {
        match result {
            Ok(0) => {
                if let Some(talking_dc) = bus.is_talking() {
                    if talking_dc.channel() == CBM_CHANNEL_CTRL {
                        debug!("Bus is in Talking mode, device=15, and got a 0 byte read response");
                        Err(DeviceError::no_device(dc.device()))
                    } else {
                        Ok(0)
                    }
                } else {
                    Ok(0)
                }
            }
            Ok(n) => Ok(n),
            Err(e) => Err(e.into()),
        }
    }

    fn bus_read_locked(bus: &mut Bus, dc: DeviceChannel, buf: &mut [u8]) -> Result<usize, Error> {
        Self::handle_read_result(bus.read(buf), bus, dc)
    }

    fn bus_read_until_locked(
        bus: &mut Bus,
        dc: DeviceChannel,
        buf: &mut Vec<u8>,
        pattern: &[u8],
    ) -> Result<usize, Error> {
        Self::handle_read_result(bus.read_until(buf, pattern), bus, dc)
    }

    #[allow(dead_code)]
    fn bus_read_until_any_locked(
        bus: &mut Bus,
        dc: DeviceChannel,
        buf: &mut Vec<u8>,
        pattern: &[u8],
    ) -> Result<usize, Error> {
        Self::handle_read_result(bus.read_until_any(buf, pattern), bus, dc)
    }

    fn check_for_status_ok(bus: &mut Bus, device: u8, accept_73: bool) -> Result<(), Error> {
        Self::get_status_locked(bus, device)
            .map_err(|e| {
                let default_error =
                    DeviceError::get_status_failure(device, format!("Failed to get status: {e}"));
                match e {
                    Error::Device { device, error } => match error {
                        DeviceError::NoDevice => DeviceError::no_device(device),
                        _ => default_error,
                    },
                    _ => default_error,
                }
            })
            .and_then(|status| {
                trace!("Status value {}", status);
                if accept_73 {
                    status.into_73_ok()
                } else {
                    status.into()
                }
            })
    }

    fn send_command_petscii_locked(
        bus: &mut Bus,
        dc: DeviceChannel,
        cmd: &PetsciiString,
    ) -> Result<(), Error> {
        bus.listen(dc)?;
        bus.write(cmd.as_bytes()).inspect_err(|_| {
            let _ = bus.unlisten();
        })?;
        bus.unlisten().map_err(|e| e.into())
    }

    fn get_status_locked(bus: &mut Bus, device: u8) -> Result<CbmStatus, Error> {
        trace!("Cbm::get_status_locked device: {device}");

        // Set up DeviceChannel to read the status
        let dc = DeviceChannel::new(device, CBM_CHANNEL_CTRL)?;

        // Put the drive into talk mode
        bus.talk(dc)?;

        // Read up to 64 bytes of data, stopping when we hit \r (or hit 64
        // bytes). \r will be included if found
        let mut buf = vec![0u8; 64];
        let pattern = vec![b'\r'];
        let bytes_read =
            Self::bus_read_until_locked(bus, dc, &mut buf, &pattern).inspect_err(|e| {
                debug!("Hit error while in read_until() loop: {}", e);
                let _ = bus.untalk();
            })?;
        trace!("Read {} bytes of status", bytes_read);

        // Tell the drive to stop talking
        bus.untalk()?;

        // Create the status from the buf
        let status_str = String::from_utf8_lossy(&buf[..bytes_read]).to_string();
        CbmStatus::new(&status_str, device)
    }

    fn read_from_drive_locked(
        bus: &mut Bus,
        dc: DeviceChannel,
        buf: &mut [u8],
        read_all: bool,
    ) -> Result<usize, Error> {
        let size = buf.len();
        trace!("Cbm::read_from_drive_locked {dc} buf.len(): {size} read_all: {read_all}");

        // Validate arguments
        Self::validate_read_args(size, format!("Asked to read {size} bytes from {dc}"))?;

        let mut read_total = 0;
        {
            // Lock the bus

            bus.talk(dc)?;

            // Main reading loop
            loop {
                let read_len =
                    Self::bus_read_locked(bus, dc, &mut buf[read_total..]).inspect_err(|_| {
                        let _ = bus.untalk();
                    })?;

                if read_len == 0 {
                    break;
                } else {
                    read_total += read_len;
                    if read_len != size {
                        continue;
                    } else {
                        break;
                    }
                }
            }

            bus.untalk()?;
        }

        if read_total != size && read_all {
            // This can be normal behaviour is a device doesn't exist
            debug!("Failed to read {size} bytes from {dc}, read {read_total} bytes");
            Err(DeviceError::read_error(
                dc,
                format!("Failed to read {size} bytes, read {read_total}"),
            ))
        } else {
            trace!("Successfully read {size} bytes from {dc}");
            Ok(read_total)
        }
    }

    fn open_file_petscii_locked(
        bus: &mut Bus,
        dc: DeviceChannel,
        filename: &PetsciiString,
    ) -> Result<(), Error> {
        // The sequence for open is:
        // Bus::open
        // Bus::write the filename (no file type required)
        // Bus::unlisten
        bus.open(dc)?;
        bus.write(filename.as_bytes()).inspect_err(|_| {
            // Clean up
            let _ = bus.unlisten();
            let _ = bus.close(dc);
        })?;
        bus.unlisten().inspect_err(|_| {
            // Clean up
            let _ = bus.close(dc);
        })?;

        // Check for status OK
        Self::check_for_status_ok(bus, dc.device(), false).inspect_err(|_| {
            // Clean up
            let _ = bus.close(dc);
        })
    }

    fn close_file_locked(bus: &mut Bus, dc: DeviceChannel) -> Result<(), Error> {
        bus.close(dc).map_err(|e| e.into())
    }
}
