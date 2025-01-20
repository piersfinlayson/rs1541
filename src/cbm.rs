//! A Rust interface for interacting with Commodore disk drives via OpenCBM
//!
//! This module provides a safe, idiomatic Rust wrapper around the OpenCBM library,
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
//! - The OpenCBM handle is protected by a mutex to allow safe multi-threaded access
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
//! The module uses a custom error type [`CbmError`] that covers various failure modes:
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
pub use crate::cbmtype::CbmDeviceInfo;
use crate::cbmtype::{
    CbmDeviceType, CbmError, CbmErrorNumber, CbmErrorNumberOk, CbmFileType, CbmStatus,
};
use crate::{CbmString, AsciiString, PetsciiString};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;

use std::collections::HashMap;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The main interface for interacting with Commodore disk drives via OpenCBM.
///
/// `Cbm` provides a safe, high-level interface to communicate with Commodore disk drives
/// through the OpenCBM library. It manages the driver connection and provides methods
/// for common disk operations like reading files, writing files, and getting directory
/// listings.
///
/// The struct uses interior mutability (via `Arc<Mutex<>>`) to allow safe concurrent
/// access to the OpenCBM driver while maintaining a clean API that doesn't require
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
    handle: Arc<Mutex<Option<xum1541::Bus>>>,
}

impl Cbm {
    /// Creates a new CBM instance and opens the XUM1541 driver.
    ///
    /// This function attempts to initialize communication with the OpenCBM driver
    /// and returns a wrapped handle that can be used for further operations.
    ///
    /// # Errors
    ///
    /// Returns `CbmError` if:
    /// - The OpenCBM driver cannot be opened
    /// - No XUM1541 device is connected
    /// - The device is in use by another process
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// ```
    pub fn new() -> Result<Self, CbmError> {
        let mut bus = xum1541::BusBuilder::new().build()?;
        bus.initialize()?;

        Ok(Self {
            handle: Arc::new(Mutex::new(Some(bus))),
        })
    }

    /// Resets the USB device connection - by closing the driver then reopening
    /// which in turn will force a device reset
    ///
    /// This is a potentially risky operation that should be used with caution.
    /// If it returns `CbmError::DriverNotOpen`, the xum1541 driver may need to
    /// be reopened with a new `Cbm` instance.
    ///
    /// # Errors
    ///
    /// Returns `CbmError` if:
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
    pub fn usb_device_reset(&mut self) -> Result<(), CbmError> {
        // Lock the old handle - will be unlocked when it goes out of scope
        let mut handle = self.handle.lock();

        // Drop the old xum1541::Bus instance which will close the driver
        let old_bus = handle.take();
        drop(old_bus);

        // Create a new instance (can fail)
        let new_bus = xum1541::BusBuilder::new().build()?;

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
    /// Returns `CbmError` if:
    /// - The driver is not open
    /// - The bus reset operation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cbm = Cbm::new()?;
    /// cbm.reset_bus()?;
    /// ```
    pub fn reset_bus(&self) -> Result<(), CbmError> {
        self.handle
            .lock()
            .as_mut() // Convert Option<OpenCbm> to Option<&OpenCbm>
            .ok_or(CbmError::UsbError(
                // Convert None to Err
                "No CBM handle".to_string(),
            ))? // Propagate error if None
            .reset() // Call reset() on the OpenCbm
            .map_err(|e| CbmError::DeviceError {
                device: 0,
                message: e.to_string(),
            }) // Convert the reset error if it occurs
    }

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
    /// Returns `CbmError` if:
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
    pub fn identify(&self, device: u8) -> Result<CbmDeviceInfo, CbmError> {
        // Issue a memory read of two bytes at address 0xff40
        // For compatibility with DOS1 drives we'll only read 1 byte at a time
        // (With DOS 2 we could pass in another byte to ask for 2 bytes)
        let buf = self.read_drive_memory(device, 0xff40, 2)?;
        let magic: u16 = ((buf[1] as u16) << 8) | (buf[0] as u16);

        // Need to do some extra work for some drives
        let (magic, magic2) = match magic {
            0xaaaa => {
                // Replace magic - not quite sure which drives this
                // differentiates between
                let buf = self.read_drive_memory(device, 0xfffe, 2)?;
                let magic: u16 = ((buf[1] as u16) << 8) | (buf[0] as u16);
                (magic, None)
            }
            0x01ba => {
                // Leave magic as is, and add a second magic, to differentiate
                // between 1581 and FDX000 drives
                let buf = self.read_drive_memory(device, 0x8008, 2)?;
                let magic2: u16 = ((buf[1] as u16) << 8) | (buf[0] as u16);
                (magic, Some(magic2))
            }
            _ => (magic, None),
        };

        // Generate the device type from the magic number(s)
        Ok(CbmDeviceInfo::from_magic(magic, magic2))
    }

    /// Function to read a number of consecutive bytes from a drive
    /// Currently only reads one byte at a time for DOS1 compatibility
    pub fn read_drive_memory(
        &self,
        device: u8,
        addr: u16,
        size: usize,
    ) -> Result<Vec<u8>, CbmError> {
        let mut buf = vec![0u8; size];

        // Split address into low and high bytes
        let addr_low = (addr & 0xFF) as u8;
        let addr_high = ((addr >> 8) & 0xFF) as u8;

        // Read one byte at a time for DOS1 compatibility
        for i in 0..size {
            let cmd = vec![b'M', b'-', b'R', addr_low.wrapping_add(i as u8), addr_high];
            self.send_command_petscii(device, &PetsciiString::from_petscii_bytes(&cmd))?;

            let mut guard = self.handle.lock();
            let bus = guard
                .as_mut()
                .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

            bus.talk(device, 15)?;
            bus.read(&mut buf[i..i + 1], 1).inspect_err(|_| {
                let _ = bus.untalk();
            })?;
            bus.untalk()?;
        }

        Ok(buf)
    }

    pub fn write_drive_memory(&self, device: u8, addr: u16, data: &[u8]) -> Result<(), CbmError> {
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
            let bus = guard
                .as_mut()
                .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

            bus.talk(device, 15)?;
            bus.untalk()?;
        }

        Ok(())
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
    /// Returns `CbmError` if:
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
    pub fn get_status(&self, device: u8) -> Result<CbmStatus, CbmError> {
        let mut guard = self.handle.lock();
        let mut bus = guard
            .as_mut()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;
        Self::get_status_already_locked(&mut bus, device)
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
    pub fn send_command(&self, device: u8, cmd: &CbmString) -> Result<(), CbmError> {
        self.send_command_petscii(device, &cmd.to_petscii())
    }

    /// Send a command on a specific drive
    /// The command must be provided as a PetsciiString
    pub fn send_command_petscii(&self, device: u8, cmd: &PetsciiString) -> Result<(), CbmError> {
        let mut guard = self.handle.lock();
        let bus = guard
            .as_mut()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        bus.listen(device, 15)?;
        bus.write(cmd.as_bytes()).inspect_err(|_| {
            let _ = bus.unlisten();
        })?;
        bus.unlisten().map_err(|e| e.into())
    }

    /// Sends a command string to a device after converting from ASCII to PETSCII
    pub fn send_command_ascii(&self, device: u8, command: &AsciiString) -> Result<(), CbmError> {
        self.send_command_petscii(device, &command.into())
    }

    /// Sends a string command to a device, converting from ASCII to PETSCII.
    /// The input string must be ASCII-compatible.
    ///
    /// # Errors
    /// Returns `CbmError` if:
    /// - The string contains non-ASCII characters
    /// - The device command fails
    pub fn send_string_command_ascii(&self, device: u8, command: &str) -> Result<(), CbmError> {
        let ascii = AsciiString::try_from(command).map_err(|e| CbmError::InvalidOperation {
            device,
            message: e.to_string(),
        })?;
        self.send_command_ascii(device, &ascii)
    }

    /// Sends a string command that is already in PETSCII format.
    /// The input string bytes must be valid PETSCII.
    ///
    /// # Errors
    /// Returns `CbmError` if the device command fails
    pub fn send_string_command_petscii(&self, device: u8, command: &str) -> Result<(), CbmError> {
        self.send_command_petscii(
            device,
            &PetsciiString::from_petscii_bytes(command.as_bytes()),
        )
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
    /// Returns `CbmError` if:
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
    ) -> Result<CbmStatus, CbmError> {
        // Validate ID length
        if id.as_bytes().len() != 2 {
            return Err(CbmError::InvalidOperation {
                device,
                message: "Disk ID must be 2 characters".into(),
            });
        }

        // Construct format command (N:name,id)
        let cmd = format!("n0:{},{}", name, id);

        self.send_string_command_ascii(device, &cmd)?;
        self.get_status(device)
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
    /// Returns `CbmError` if:
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
    pub fn read_file(&self, device: u8, filename: &AsciiString) -> Result<Vec<u8>, CbmError> {
        let guard = self.handle.lock();
        let _bus = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        let channel = 2; // For demonstration

        drop(guard);

        self.send_command_ascii(device, filename)?;

        // Check status after open
        let status = self.get_status(device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        // Re-acquire guard for file operations
        let mut guard = self.handle.lock();
        let bus = guard.as_mut().ok_or(CbmError::FileError {
            device,
            message: "No CBM handle".to_string(),
        })?;

        // Now read the file data
        bus.talk(device, channel).map_err(|e| CbmError::FileError {
            device,
            message: format!("Talk failed: {}", e),
        })?;

        let mut data = Vec::new();
        loop {
            let buf = &mut [0u8; 256];
            let count = bus.read(buf, 256).map_err(|e| CbmError::FileError {
                device,
                message: format!("Read failed: {}", e),
            })?;

            data.extend_from_slice(&buf[..count as usize]);
            if count < 256 {
                debug!("Finished reading file");
                break;
            }
        }

        // Cleanup
        bus.untalk().map_err(|e| CbmError::FileError {
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
    /// Returns `CbmError` if:
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
    pub fn write_file(
        &self,
        device: u8,
        filename: &AsciiString,
        data: &[u8],
    ) -> Result<(), CbmError> {
        let guard = self.handle.lock();
        let _bus = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        let channel = 2; // For demonstration

        drop(guard);

        // Open file for writing with overwrite if exists
        self.send_string_command_ascii(device, &format!("@:{}", filename))?;

        // Check status after open
        let status = self.get_status(device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        // Reacquire guard for file operations
        let mut guard = self.handle.lock();
        let bus = guard.as_mut().ok_or(CbmError::FileError {
            device,
            message: "No CBM handle".to_string(),
        })?;

        // Now write the file data
        bus.listen(device, channel)
            .map_err(|e| CbmError::FileError {
                device,
                message: format!("Listen failed: {}", e),
            })?;

        // Write data in chunks
        for chunk in data.chunks(256) {
            let result = bus.write(chunk).map_err(|e| CbmError::FileError {
                device,
                message: format!("Write failed: {}", e),
            })?;

            if result != chunk.len() {
                return Err(CbmError::FileError {
                    device,
                    message: "Failed to write complete chunk".into(),
                });
            }
        }

        // Cleanup
        bus.unlisten().map_err(|e| CbmError::FileError {
            device,
            message: format!("Unlisten failed: {}", e),
        })?;

        Ok(())
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
    /// Returns `CbmError` if:
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
    pub fn delete_file(&self, device: u8, filename: &AsciiString) -> Result<(), CbmError> {
        // Construct scratch command (S:filename)
        let cmd = format!("s0:{}", filename);
        self.send_string_command_ascii(device, &cmd)?;
        self.get_status(device)?.into()
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
    /// Returns `CbmError` if:
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
    pub fn validate_disk(&self, device: u8) -> Result<(), CbmError> {
        // Send validate command (V)
        self.send_command_petscii(device, &PetsciiString::from_ascii_str("v"))?;

        // Check status after validation
        self.get_status(device)?.into()
    }

    fn error_untalk_and_close_file_locked(bus: &mut xum1541::Bus, device: u8, channel_num: u8) {
        trace!("Cbm: Entered error_untalk_and_close_file_locked");
        let _ = bus
            .untalk()
            .inspect_err(|_| debug!("Untalk failed {} {}", device, channel_num));

        let _ = Self::close_file_locked(bus, device, channel_num)
            .inspect_err(|_| debug!("Close file failed {} {}", device, channel_num));
        trace!("Cbm: Exited error_untalk_and_close_file_locked");
    }

    /// Open a file using an ASCII filename
    pub fn open_file(
        &self,
        device: u8,
        channel_num: u8,
        filename: &AsciiString,
    ) -> Result<(), CbmError> {
        let mut guard = self.handle.lock();
        let bus = guard
            .as_mut()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        let petscii_name: PetsciiString = filename.into();

        bus.open(device, channel_num)?;
        bus.write(petscii_name.as_bytes()).inspect_err(|_| {
            let _ = bus.close(device, channel_num);
        })?;
        bus.unlisten().map_err(|e| e.into())
    }

    pub fn close_file(&self, device: u8, channel_num: u8) -> Result<(), CbmError> {
        let mut guard = self.handle.lock();
        let bus = guard
            .as_mut()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        Self::close_file_locked(bus, device, channel_num)
    }

    fn close_file_locked(
        bus: &mut xum1541::Bus,
        device: u8,
        channel_num: u8,
    ) -> Result<(), CbmError> {
        bus.close(device, channel_num).map_err(|e| e.into())
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
    /// Returns `CbmError` if:
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
    pub fn dir(&self, device: u8, drive_num: Option<u8>) -> Result<CbmDirListing, CbmError> {
        // Validate drive_num
        if let Some(drive_num) = drive_num {
            if drive_num > 1 {
                return Err(CbmError::InvalidOperation {
                    device,
                    message: format!("Invalid drive number {} - must be 0 or 1", drive_num),
                });
            }
        }

        // Construct directory command ("$" or "$0" or "$1")
        let filename = AsciiString::try_from(match drive_num {
            Some(num) => format!("${}", num),
            None => "$".to_string(),
        })
        .map_err(|e| CbmError::InvalidOperation {
            device,
            message: e.to_string(),
        })?;

        let channel_num = 0;
        self.open_file(device, channel_num, &filename)
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Failed to open directory {}: {}", filename, e),
            })?;

        let mut output = String::new();

        // Check that open succeeded
        match self.get_status(device) {
            Ok(status) => {
                if status.is_ok() != CbmErrorNumberOk::Ok {
                    let _ = self.close_file(device, channel_num);
                    return Err(CbmError::CommandError {
                        device,
                        message: format!("Got error status after dir open {}", status),
                    });
                } else {
                    debug!("status value after dir open {}", status);
                }
            }
            Err(e) => {
                let _ = self.close_file(device, channel_num);
                return Err(CbmError::CommandError {
                    device,
                    message: format!("Failed to open directory: {}", e),
                });
            }
        }

        let mut guard = self.handle.lock();
        let mut bus = guard
            .as_mut()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        // Read the directory data
        match bus.talk(device, channel_num) {
            Ok(_) => {}
            Err(e) => {
                let _ = Self::close_file_locked(bus, device, channel_num);
                return Err(CbmError::DeviceError {
                    device,
                    message: format!("Talk failed: {}", e),
                });
            }
        };

        // Skip the load address (first two bytes)
        trace!("Read 2 bytes");
        let buf = &mut [0u8; 2];
        let result = bus
            .read(buf, 2)
            .inspect_err(|_| {
                Self::error_untalk_and_close_file_locked(&mut bus, device, channel_num)
            })
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Failed to read load address: {}", e),
            })?;

        if result == 2 {
            // Read directory entries
            loop {
                trace!("In read loop");
                // Read link address
                let buf = &mut [0u8; 2];
                let count = bus
                    .read(buf, 2)
                    .inspect_err(|_| {
                        Self::error_untalk_and_close_file_locked(&mut bus, device, channel_num)
                    })
                    .map_err(|e| CbmError::DeviceError {
                        device,
                        message: format!("Failed to read link address: {}", e),
                    })?;

                if count != 2 {
                    break;
                }

                // Read file size
                let size_buf = &mut [0u8; 2];
                let size_count = bus
                    .read(size_buf, 2)
                    .inspect_err(|_| {
                        Self::error_untalk_and_close_file_locked(&mut bus, device, channel_num)
                    })
                    .map_err(|e| CbmError::DeviceError {
                        device,
                        message: format!("Failed to read file size: {}", e),
                    })?;

                if size_count != 2 {
                    break;
                }

                // Calculate file size (little endian)
                let size = (size_buf[0] as u16) | ((size_buf[1] as u16) << 8);
                output.push_str(&format!("{:4} ", size));

                // Read filename characters until 0 byte
                let mut filename = Vec::new();
                loop {
                    let char_buf = &mut [0u8; 1];
                    let char_count = bus
                        .read(char_buf, 1)
                        .inspect_err(|_| {
                            Self::error_untalk_and_close_file_locked(&mut bus, device, channel_num)
                        })
                        .map_err(|e| CbmError::DeviceError {
                            device,
                            message: format!("Failed to read filename: {}", e),
                        })?;

                    if char_count != 1 || char_buf[0] == 0 {
                        break;
                    }

                    filename.push(char_buf[0]);
                }
                let petscii_filename = PetsciiString::from_petscii_bytes(&filename);
                let ascii_filename: AsciiString = petscii_filename.into();
                let str_filename = ascii_filename.to_string();
                output.push_str(&str_filename);
                output.push('\n');
            }
        }

        // Cleanup
        bus.untalk()
            .inspect_err(|_| {
                Self::error_untalk_and_close_file_locked(&mut bus, device, channel_num)
            })
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Untalk failed: {}", e),
            })?;

        Self::close_file_locked(bus, device, 0).map_err(|e| CbmError::DeviceError {
            device,
            message: format!("Failed to close directory: {}", e),
        })?;

        // Get final status
        let status = Self::get_status_already_locked(&mut bus, device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        let result = if let Ok(directory) = CbmDirListing::parse(&output) {
            // Directory is now parsed into a structured format
            Ok(directory)
        } else {
            Err(CbmError::DeviceError {
                device,
                message: "Failed to parse directory listing".to_string(),
            })
        }?;

        trace!("Dir success: {:?}", result);

        Ok(result)
    }
}

impl Cbm {
    fn get_status_already_locked(
        bus: &mut xum1541::Bus,
        device: u8,
    ) -> Result<CbmStatus, CbmError> {
        let mut buf = vec![0u8; 64];

        // Put the drive into talk mode
        bus.talk(device, 15)?;

        // Read up to 256 bytes of data, stopping when we hit \r (or hit 64
        // bytes). \r will be included if found
        bus.read_until(&mut buf, 64, &vec![b'\r'])
            .inspect_err(|_| {
                let _ = bus.untalk();
            })?;

        // Tell the drive to stop talking
        bus.untalk()?;

        // Create the status from the buf
        let status_str = String::from_utf8_lossy(&buf).to_string();
        CbmStatus::new(&status_str, device)
    }
}

/// Represents a channel to a CBM drive
///
/// Channels are the primary means of communication with CBM drives. Each drive
/// supports 16 channels (0-15), with channel 15 reserved for control operations.
#[derive(Debug, Clone)]
pub struct CbmChannel {
    _number: u8,
    _purpose: CbmChannelPurpose,
}

/// Purpose for which a channel is being used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbmChannelPurpose {
    Reset,     // Channel 15 - reserved for reset commands
    Directory, // Reading directory
    FileRead,  // Reading a file
    FileWrite, // Writing a file
    Command,   // Other command channel operations
}

/// Manages channel allocation for a drive unit
///
/// Ensures proper allocation and deallocation of channels, maintaining
/// the invariant that channel 15 is only used for reset operations.
#[derive(Debug)]
pub struct CbmChannelManager {
    channels: HashMap<u8, Option<CbmChannel>>,
    next_sequence: AtomicU64,
}

impl CbmChannelManager {
    pub fn new() -> Self {
        let mut channels = HashMap::new();
        for i in 0..=15 {
            channels.insert(i, None);
        }
        Self {
            channels,
            next_sequence: AtomicU64::new(1), // Start at 1 to avoid handle 0
        }
    }

    /// Allocates a channel for a specific purpose
    ///
    /// Returns (channel_number, handle) if successful, None if no channels available
    /// or if attempting to allocate channel 15 for non-reset purposes
    pub fn allocate(
        &mut self,
        _device_number: u8,
        _drive_id: u8,
        purpose: CbmChannelPurpose,
    ) -> Option<u8> {
        // Channel 15 handling
        if purpose == CbmChannelPurpose::Reset {
            if let Some(slot) = self.channels.get_mut(&15) {
                if slot.is_none() {
                    let _sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst);
                    *slot = Some(CbmChannel {
                        _number: 15,
                        _purpose: purpose,
                    });
                    return Some(15);
                }
            }
            return None;
        }

        // Regular channel allocation
        for i in 0..15 {
            if let Some(slot) = self.channels.get_mut(&i) {
                if slot.is_none() {
                    let _sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst);
                    *slot = Some(CbmChannel {
                        _number: i,
                        _purpose: purpose,
                    });
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn reset(&mut self) {
        for i in 0..=15 {
            self.channels.insert(i, None);
        }
    }
}

/// Represents a physical drive unit
///
/// Manages the channels and state for a single physical drive unit,
/// which may contain one or two drives.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CbmDriveUnit {
    pub device_number: u8,
    pub device_type: CbmDeviceType,
    channel_manager: Arc<Mutex<CbmChannelManager>>,
    busy: bool,
}

impl fmt::Display for CbmDriveUnit {
    /// Provides a string representation of the drive unit.
    ///
    /// Returns a string containing the device number and type.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm1541);
    /// println!("{}", drive); // Outputs: "Drive 8 (1541)"
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Drive {} ({})", self.device_number, self.device_type)
    }
}

/// Represents a physical Commodore disk drive unit connected to the system.
///
/// A `CbmDriveUnit` manages the state and operations for a single physical drive unit.
/// This can be a single-drive unit (like the 1541) or a dual-drive unit (4040, etc).
/// The struct handles channel allocation, device status tracking, and drive-specific
/// operations.
///
/// The drive unit maintains its own channel manager to ensure proper allocation and
/// deallocation of communication channels. Channel 15 is reserved for commands and
/// status operations.
///
/// # Examples
///
/// ```ignore
/// use your_crate_name::{CbmDriveUnit, CbmDeviceType};
///
/// // Create a new 1541 drive unit
/// let mut drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm1541);
///
/// // Initialize both drives if this is a dual unit
/// let cbm = Cbm::new()?;
/// let status = drive.send_init(cbm, &vec![])?;
/// ```
///
impl CbmDriveUnit {
    /// Creates a new drive unit instance.
    ///
    /// This function creates a new drive unit with the specified device number
    /// and type. It initializes the channel manager but does not perform any
    /// hardware communication.
    ///
    /// # Arguments
    ///
    /// * `device_number` - The IEC device number
    /// * `device_type` - The type of drive (e.g., Cbm1541, Cbm1571)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm1541);
    /// ```
    pub fn new(device_number: u8, device_type: CbmDeviceType) -> Self {
        // Test whether this device is actually attached
        Self {
            device_number,
            device_type,
            channel_manager: Arc::new(Mutex::new(CbmChannelManager::new())),
            busy: false,
        }
    }

    /// Gets the current status of the drive unit.
    ///
    /// Retrieves the status message from the drive, which may include error conditions,
    /// drive state, or the result of the last operation.
    ///
    /// # Arguments
    ///
    /// * `cbm` - The Cbm instance to use for communication
    ///
    /// # Errors
    ///
    /// Returns `CbmError` if:
    /// - The drive doesn't respond
    /// - The status cannot be read
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm1541);
    /// let cbm = Cbm::new()?;
    /// let status = drive.get_status(&cbm)?;
    /// println!("Drive status: {}", status);
    /// ```
    pub fn get_status(&mut self, cbm: &Cbm) -> Result<CbmStatus, CbmError> {
        self.busy = true;
        cbm.get_status(self.device_number)
            .inspect(|_| self.busy = false)
            .inspect_err(|_| self.busy = false)
    }

    /// Sends initialization commands to all drives in the unit.
    ///
    /// For dual drive units, this will initialize both drive 0 and drive 1.
    /// The function returns a vector of status messages, one for each drive
    /// that was initialized.
    ///
    /// # Arguments
    ///
    /// * `cbm` - The Cbm instance to use for communication
    /// * `ignore_errors` - Vector of error numbers that should not cause the operation to fail
    ///
    /// # Errors
    ///
    /// Returns `CbmError` if:
    /// - Any drive fails to initialize (unless its error is in ignore_errors)
    /// - The command cannot be sent
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm4040);
    /// let cbm = Cbm::new()?;
    ///
    /// // Initialize both drives, ignoring "drive not ready" errors
    /// let status = drive.send_init(cbm, &vec![CbmErrorNumber::DriveNotReady])?;
    /// ```
    pub fn send_init(
        &mut self,
        cbm: &mut Cbm,
        ignore_errors: &Vec<CbmErrorNumber>,
    ) -> Result<Vec<CbmStatus>, CbmError> {
        self.busy = true;

        // First ? catches panic and maps to CbmError
        // Second > propagates CbmError (from first, or from within {})
        let mut status_vec: Vec<CbmStatus> = Vec::new();
        catch_unwind(AssertUnwindSafe(|| {
            self.num_disk_drives_iter().try_for_each(|ii| {
                let cmd = format!("i{}", ii);
                cbm.send_string_command_ascii(self.device_number, &cmd)
                    .inspect_err(|_| self.busy = false)?;
                let status = cbm
                    .get_status(self.device_number)
                    .inspect_err(|_| self.busy = false)?;
                if status.is_ok() != CbmErrorNumberOk::Ok {
                    if !ignore_errors.contains(&status.error_number) {
                        self.busy = false;
                        return Err(CbmError::CommandError {
                            device: self.device_number,
                            message: format!("{} {}", cmd, status),
                        });
                    } else {
                        debug!("Ignoring error {}", status.error_number);
                    }
                }
                status_vec.push(status);
                Ok(())
            })
        }))
        .inspect_err(|_| self.busy = false)?
        .inspect_err(|_| self.busy = false)?;

        self.busy = false;
        Ok(status_vec)
    }

    #[allow(dead_code)]
    fn reset(&mut self) -> Result<(), CbmError> {
        self.busy = true;
        self.channel_manager.lock().reset();
        self.busy = true;
        Ok(())
    }

    /// Gets the number of disk drives in this unit.
    ///
    /// Returns 1 for single drive units (like the 1541) and 2 for
    /// dual drive units (like the 4040).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm4040);
    /// assert_eq!(drive.num_disk_drives(), 2);
    /// ```
    pub fn num_disk_drives(&self) -> u8 {
        self.device_type.num_disk_drives()
    }

    /// Returns an iterator over the drive numbers in this unit.
    ///
    /// For a single drive unit, yields only 0.
    /// For a dual drive unit, yields 0 and 1.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm4040);
    /// for drive_num in drive.num_disk_drives_iter() {
    ///     println!("Initializing drive {}", drive_num);
    ///     // ... initialize drive ...
    /// }
    /// ```
    pub fn num_disk_drives_iter(&self) -> impl Iterator<Item = u8> {
        0..self.num_disk_drives()
    }

    /// Returns an iterator over the drive numbers in this unit.
    ///
    /// For a single drive unit, yields only 0.
    /// For a dual drive unit, yields 0 and 1.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm1571);
    /// for drive_num in drive.num_disk_drives_iter() {
    ///     println!("Initializing drive {}", drive_num);
    ///     // ... initialize drive ...
    /// }
    /// ```
    pub fn is_responding(&self) -> bool {
        true
    }

    /// Checks if the drive unit is currently busy with an operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm1541);
    /// if !drive.is_busy() {
    ///     // Safe to send new commands
    /// }
    /// ```
    pub fn is_busy(&self) -> bool {
        self.busy
    }
}

/// Represents an entry in a Commodore disk directory.
///
/// This enum handles both valid and invalid directory entries. Valid entries contain
/// complete file information including size, name, and type. Invalid entries retain
/// as much information as could be parsed along with error details, allowing for
/// diagnostic and recovery operations.
///
/// Directory entries on Commodore drives follow a specific format:
/// ```text
/// BLOCKS   "FILENAME"   TYPE   
///    10    "MYFILE"    PRG
/// ```
///
/// # Examples
///
/// ```ignore
/// match file_entry {
///     CbmFileEntry::ValidFile { blocks, filename, file_type } => {
///         println!("{} blocks: {} ({})", blocks, filename, file_type);
///     },
///     CbmFileEntry::InvalidFile { raw_line, error, .. } => {
///         println!("Error parsing entry: {} - {}", raw_line, error);
///     }
/// }
/// ```
#[derive(Debug)]
pub enum CbmFileEntry {
    /// Represents a successfully parsed directory entry.
    ///
    /// Contains all information about a file as stored in the directory.
    ///
    /// # Fields
    ///
    /// * `blocks` - Size of the file in disk blocks (1 block = 254 bytes of user data)
    /// * `filename` - Name of the file as stored on disk (may include shifted characters)
    /// * `file_type` - Type of the file (PRG, SEQ, USR, etc.)
    ValidFile {
        blocks: u16,
        filename: String,
        file_type: CbmFileType,
    },
    /// Represents a directory entry that could not be fully parsed.
    ///
    /// This variant retains the raw directory line and any partial information
    /// that could be extracted, along with details about what went wrong during parsing.
    ///
    /// # Fields
    ///
    /// * `raw_line` - The original directory line that failed to parse
    /// * `error` - Description of what went wrong during parsing
    /// * `partial_blocks` - Block count if it could be parsed
    /// * `partial_filename` - Filename if it could be parsed
    InvalidFile {
        raw_line: String,
        error: String,                    // Description of what went wrong
        partial_blocks: Option<u16>,      // In case we at least got the blocks
        partial_filename: Option<String>, // In case we at least got the filename
    },
}

impl fmt::Display for CbmFileEntry {
    /// Formats the file entry for display.
    ///
    /// # Format
    ///
    /// For valid files:
    /// - Shows filename with type suffix (e.g., "PROGRAM.PRG")
    /// - Shows block count right-aligned
    /// - Pads with spaces to align multiple entries
    ///
    /// For invalid files:
    /// - Shows the error message
    /// - Includes any partial information that was successfully parsed
    /// - Includes the raw directory line for debugging
    ///
    /// # Examples
    ///
    /// Valid file:
    /// ```text
    /// Filename: "MYPROG.PRG"          Blocks: 10
    /// ```
    ///
    /// Invalid file:
    /// ```text
    /// Invalid entry: "   10  MYPROG*" (Invalid character in filename) [Blocks: 10]
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CbmFileEntry::ValidFile {
                blocks,
                filename,
                file_type,
            } => {
                write!(
                    f,
                    "Filename: \"{}.{}\"{:width$}Blocks: {:>3}",
                    filename,
                    file_type,
                    "", // empty string for padding
                    blocks,
                    width = 25 - (filename.len() + 3 + 1) // +1 for the dot, +3 for suffix
                )
            }
            CbmFileEntry::InvalidFile {
                raw_line,
                error,
                partial_blocks,
                partial_filename,
            } => {
                write!(f, "Invalid entry: {} ({})", raw_line, error)?;
                if let Some(filename) = partial_filename {
                    write!(f, " [Filename: \"{}\"]", filename)?;
                }
                if let Some(blocks) = partial_blocks {
                    write!(f, " [Blocks: {}]", blocks)?;
                }
                Ok(())
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct CbmDiskHeader {
    drive_number: u8,
    name: String,
    id: String,
}

/// Common disk header constants
impl CbmDiskHeader {
    /// Maximum length of a disk name (16 characters)
    pub const MAX_NAME_LENGTH: usize = 16;

    /// Required length of a disk ID (2 characters)
    pub const ID_LENGTH: usize = 2;
}

impl fmt::Display for CbmDiskHeader {
    /// Formats the disk header for display.
    ///
    /// Produces output in the format:
    /// ```text
    /// Drive 0 Header: "MY DISK" ID: 01
    /// ```
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let header = CbmDiskHeader::parse_header("0 .\"MY DISK     01\"")?;
    /// println!("{}", header); // "Drive 0 Header: "MY DISK" ID: 01"
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Drive {} Header: \"{}\" ID: {}",
            self.drive_number, self.name, self.id
        )
    }
}

/// Represents the header information of a Commodore disk.
///
/// The disk header on Commodore drives contains metadata about the disk, including
/// its name, ID, and which drive it was formatted in. This information appears at
/// the start of every directory listing in a specific format:
///
/// ```text
/// "0 .DISKNAME     ID."
///  ^ ^  ^          ^
///  | |  |          |
///  | |  |          Two-character disk ID
///  | |  16-character disk name (padded with shifted spaces)
///  | Leading dot indicating header line
///  Drive number (0 or 1)
/// ```
///
/// # Examples
///
/// ```ignore
/// use your_crate_name::CbmDiskHeader;
///
/// // Parse a header line from a directory listing
/// let header = CbmDiskHeader::parse_header("0 .\"MY DISK     01\"")?;
/// assert_eq!(header.drive_number, 0);
/// assert_eq!(header.name, "MY DISK");
/// assert_eq!(header.id, "01");
/// ```
///
/// # Header Format Details
///
/// - The drive number is 0 for the first drive or 1 for the second drive in dual units
/// - The disk name can be up to 16 characters, padded with shifted spaces if shorter
/// - The ID is always exactly 2 characters
/// - Special characters in the name are stored in PETSCII but converted to ASCII for display
#[allow(dead_code)]
#[derive(Debug)]
pub struct CbmDirListing {
    /// The drive number (0 or 1) where this disk is mounted
    header: CbmDiskHeader,

    /// The name of the disk (up to 16 characters)
    files: Vec<CbmFileEntry>,

    /// The two-character disk ID
    blocks_free: u16,
}

impl fmt::Display for CbmDirListing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.header)?;
        for entry in &self.files {
            writeln!(f, "{}", entry)?;
        }
        writeln!(f, "Free blocks: {}", self.blocks_free)
    }
}

impl CbmDirListing {
    /// Parses a raw directory listing string into a structured format.
    ///
    /// This function takes the raw text output from a directory command and
    /// converts it into a structured `CbmDirListing` containing the header,
    /// file entries, and free space information.
    ///
    /// # Arguments
    ///
    /// * `input` - Raw directory listing string from the disk
    ///
    /// # Returns
    ///
    /// * `Ok(CbmDirListing)` if parsing succeeds
    /// * `Err(CbmError)` if the listing cannot be parsed
    ///
    /// # Errors
    ///
    /// Returns `CbmError::ParseError` if:
    /// - The header line is missing or invalid
    /// - The blocks free line is missing or invalid
    /// - The listing format doesn't match expectations
    ///
    /// Note that invalid file entries do not cause the parse to fail;
    /// they are stored as `CbmFileEntry::InvalidFile` variants.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let raw_dir = r#"
    /// 0 "MY DISK     01" 2A
    /// 10   "PROGRAM"     PRG
    /// 5    "DATA"        SEQ
    /// 664 BLOCKS FREE.
    /// "#;
    ///
    /// let dir = CbmDirListing::parse(raw_dir)?;
    /// assert_eq!(dir.header.name, "MY DISK");
    /// assert_eq!(dir.files.len(), 2);
    /// assert_eq!(dir.blocks_free, 664);
    /// ```
    pub fn parse(input: &str) -> Result<Self, CbmError> {
        let mut lines = input.lines();

        // Parse header
        let header = Self::parse_header(lines.next().ok_or_else(|| CbmError::ParseError {
            message: "Missing header line".to_string(),
        })?)?;

        // Parse files
        let mut files = Vec::new();
        let mut blocks_free = None;

        for line in lines {
            if line.contains("blocks free") {
                blocks_free = Some(Self::parse_blocks_free(line)?);
                break;
            } else {
                files.push(Self::parse_file_entry(line));
            }
        }

        let blocks_free = blocks_free.ok_or_else(|| CbmError::ParseError {
            message: "Missing blocks free line".to_string(),
        })?;

        Ok(CbmDirListing {
            header,
            files,
            blocks_free,
        })
    }

    fn parse_header(line: &str) -> Result<CbmDiskHeader, CbmError> {
        // Example: "   0 ."test/demo  1/85 " 8a 2a"
        let re =
            regex::Regex::new(r#"^\s*(\d+)\s+\."([^"]*)" ([a-zA-Z0-9]{2})"#).map_err(|_| {
                CbmError::ParseError {
                    message: "Invalid header regex".to_string(),
                }
            })?;

        let caps = re.captures(line).ok_or_else(|| CbmError::ParseError {
            message: format!("Invalid header format: {}", line),
        })?;

        Ok(CbmDiskHeader {
            drive_number: caps[1].parse().map_err(|_| CbmError::ParseError {
                message: format!("Invalid drive number: {}", &caps[1]),
            })?,
            name: caps[2].trim_end().to_string(), // Keep leading spaces, trim trailing
            id: caps[3].to_string(),
        })
    }

    fn parse_file_entry(line: &str) -> CbmFileEntry {
        let re = regex::Regex::new(r#"^\s*(\d+)\s+"([^"]+)"\s+(\w+)\s*$"#).expect("Invalid regex");

        match re.captures(line) {
            Some(caps) => {
                let blocks = match caps[1].trim().parse() {
                    Ok(b) => b,
                    Err(_) => {
                        return CbmFileEntry::InvalidFile {
                            raw_line: line.to_string(),
                            error: "Invalid block count".to_string(),
                            partial_blocks: None,
                            partial_filename: Some(caps[2].to_string()),
                        }
                    }
                };

                let filetype = CbmFileType::from(&caps[3]);

                CbmFileEntry::ValidFile {
                    blocks,
                    filename: caps[2].to_string(), // Keep all spaces
                    file_type: filetype,
                }
            }
            None => CbmFileEntry::InvalidFile {
                raw_line: line.to_string(),
                error: "Could not parse line format".to_string(),
                partial_blocks: None,
                partial_filename: None,
            },
        }
    }

    fn parse_blocks_free(line: &str) -> Result<u16, CbmError> {
        let re =
            regex::Regex::new(r"^\s*(\d+)\s+blocks free").map_err(|_| CbmError::ParseError {
                message: "Invalid blocks free regex".to_string(),
            })?;

        let caps = re.captures(line).ok_or_else(|| CbmError::ParseError {
            message: format!("Invalid blocks free format: {}", line),
        })?;

        caps[1].parse().map_err(|_| CbmError::ParseError {
            message: format!("Invalid blocks free number: {}", &caps[1]),
        })
    }
}
