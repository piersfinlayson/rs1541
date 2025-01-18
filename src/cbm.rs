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
//! ```rust
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
use crate::opencbm::{ascii_to_petscii, petscii_to_ascii, OpenCbm};

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
/// ```rust
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
    handle: Arc<Mutex<Option<OpenCbm>>>,
}

impl Cbm {
    /// Creates a new CBM instance and opens the OpenCBM XUM1541 driver.
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// ```
    pub fn new() -> Result<Self, CbmError> {
        let opencbm = OpenCbm::new()?;

        Ok(Self {
            handle: Arc::new(Mutex::new(Some(opencbm))),
        })
    }

    /// Resets the USB device connection.
    ///
    /// This is a potentially risky operation that should be used with caution.
    /// If it returns `CbmError::DriverNotOpen`, the OpenCBM driver may need to
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
    /// ```rust
    /// let mut cbm = Cbm::new()?;
    /// cbm.usb_device_reset()?;
    /// ```
    pub fn usb_device_reset(&mut self) -> Result<(), CbmError> {
        let mut handle = self.handle.lock();
        if let Some(h) = handle.as_mut() {
            h.usb_device_reset().map_err(|e| e.into())
        } else {
            Err(CbmError::UsbError("No CBM handle".to_string()))
        }
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// cbm.reset_bus()?;
    /// ```
    pub fn reset_bus(&self) -> Result<(), CbmError> {
        self.handle
            .lock()
            .as_ref() // Convert Option<OpenCbm> to Option<&OpenCbm>
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// let info = cbm.identify(8)?;
    /// println!("Device type: {}", info.device_type);
    /// ```
    pub fn identify(&self, device: u8) -> Result<CbmDeviceInfo, CbmError> {
        self.handle
            .lock()
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?
            .identify(device)
            .map_err(|e| CbmError::DeviceError {
                device,
                message: e.to_string(),
            })
    }

    fn get_status_already_locked(cbm: &OpenCbm, device: u8) -> Result<CbmStatus, CbmError> {
        let (buf, result) = cbm
            .device_status(device)
            .map_err(|e| CbmError::DeviceError {
                device,
                message: e.to_string(),
            })?;

        if result < 0 {
            return Err(CbmError::DeviceError {
                device,
                message: format!("Failed to get device status error {}", result),
            });
        }

        let status = String::from_utf8_lossy(&buf)
            .split("\r")
            .next()
            .unwrap_or(&String::from_utf8_lossy(&buf))
            .trim()
            .to_string();

        CbmStatus::new(&status, device)
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// let status = cbm.get_status(8)?;
    /// println!("Drive status: {}", status);
    /// ```
    pub fn get_status(&self, device: u8) -> Result<CbmStatus, CbmError> {
        let guard = self.handle.lock();
        let cbm = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;
        Self::get_status_already_locked(cbm, device)
    }

    /// Sends a command to a device.
    ///
    /// Commands are sent over channel 15 and allow direct communication
    /// with the drive's DOS.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `command` - The command string to send (in ASCII, will be converted to PETSCII)
    ///
    /// # Errors
    ///
    /// Returns `CbmError` if:
    /// - The device doesn't respond
    /// - The command cannot be sent
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// cbm.send_command(8, "i0")?; // Initialize drive 0
    /// ```
    pub fn send_command(&self, device: u8, command: &str) -> Result<(), CbmError> {
        let guard = self.handle.lock();
        let cbm = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        debug!("Send command: {}", command);

        // Allocate channel 15 for commands
        cbm.listen(device, 15).map_err(|e| CbmError::CommandError {
            device,
            message: format!("Listen failed: {}", e),
        })?;

        // Convert command to PETSCII and send
        let cmd_bytes = ascii_to_petscii(command);
        let result = cbm
            .raw_write(&cmd_bytes)
            .map_err(|e| CbmError::CommandError {
                device,
                message: format!("Write failed: {}", e),
            })?;

        if result != cmd_bytes.len() as i32 {
            return Err(CbmError::CommandError {
                device,
                message: "Failed to write full command".into(),
            });
        }

        // Cleanup
        cbm.unlisten().map_err(|e| CbmError::CommandError {
            device,
            message: format!("Unlisten failed: {}", e),
        })?;

        Ok(())
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// cbm.format_disk(8, "MY DISK", "01")?;
    /// ```
    pub fn format_disk(&self, device: u8, name: &str, id: &str) -> Result<CbmStatus, CbmError> {
        // Validate ID length
        if id.len() != 2 {
            return Err(CbmError::InvalidOperation {
                device,
                message: "Disk ID must be 2 characters".into(),
            });
        }

        // Construct format command (N:name,id)
        let cmd = format!("n0:{},{}", name, id);
        self.send_command(device, &cmd)?;

        self.get_status(device)
    }

    /// Reads a file from the disk.
    ///
    /// Reads the entire contents of the specified file into a vector of bytes.
    ///
    /// # Arguments
    ///
    /// * `device` - Device number (typically 8-11 for disk drives)
    /// * `filename` - Name of the file to read
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// let data = cbm.read_file(8, "MYPROGRAM.PRG")?;
    /// ```
    pub fn read_file(&self, device: u8, filename: &str) -> Result<Vec<u8>, CbmError> {
        let guard = self.handle.lock();
        let _cbm = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;
        let mut data = Vec::new();

        // Find a free channel (0-14)
        // In a real implementation, we'd use the CbmChannelManager here
        let channel = 2; // For demonstration

        // Open file for reading
        drop(guard); // Drop guard temporarily for send_command
        self.send_command(device, &format!("{}", filename))?;

        // Check status after open
        let status = self.get_status(device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        // Re-acquire guard for file operations
        let guard = self.handle.lock();
        let cbm = guard.as_ref().ok_or(CbmError::FileError {
            device,
            message: "No CBM handle".to_string(),
        })?;

        // Now read the file data
        cbm.talk(device, channel).map_err(|e| CbmError::FileError {
            device,
            message: format!("Talk failed: {}", e),
        })?;

        loop {
            let (buf, count) = cbm.raw_read(256).map_err(|e| CbmError::FileError {
                device,
                message: format!("Read failed: {}", e),
            })?;

            if count <= 0 {
                break;
            }

            data.extend_from_slice(&buf[..count as usize]);
        }

        // Cleanup
        cbm.untalk().map_err(|e| CbmError::FileError {
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
    /// * `filename` - Name for the file
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// let data = vec![0x01, 0x08, 0x0C, 0x08, 0x0A, 0x00];
    /// cbm.write_file(8, "NEWFILE.PRG", &data)?;
    /// ```
    pub fn write_file(&self, device: u8, filename: &str, data: &[u8]) -> Result<(), CbmError> {
        let guard = self.handle.lock();
        let _cbm = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        // Find a free channel (0-14)
        // In a real implementation, we'd use the CbmChannelManager here
        let channel = 2; // For demonstration

        // Drop guard for nested operations that need the mutex
        drop(guard);

        // Open file for writing with overwrite if exists
        self.send_command(device, &format!("@:{}", filename))?;

        // Check status after open
        let status = self.get_status(device)?;
        if status.is_ok() != CbmErrorNumberOk::Ok {
            return Err(status.into());
        }

        // Reacquire guard for file operations
        let guard = self.handle.lock();
        let cbm = guard.as_ref().ok_or(CbmError::FileError {
            device,
            message: "No CBM handle".to_string(),
        })?;

        // Now write the file data
        cbm.listen(device, channel)
            .map_err(|e| CbmError::FileError {
                device,
                message: format!("Listen failed: {}", e),
            })?;

        // Write data in chunks
        for chunk in data.chunks(256) {
            let result = cbm.raw_write(chunk).map_err(|e| CbmError::FileError {
                device,
                message: format!("Write failed: {}", e),
            })?;

            if result != chunk.len() as i32 {
                return Err(CbmError::FileError {
                    device,
                    message: "Failed to write complete chunk".into(),
                });
            }
        }

        // Cleanup
        cbm.unlisten().map_err(|e| CbmError::FileError {
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
    /// * `filename` - Name of the file to delete
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// cbm.delete_file(8, "OLDFILE.PRG")?;
    /// ```
    pub fn delete_file(&self, device: u8, filename: &str) -> Result<(), CbmError> {
        // Construct scratch command (S:filename)
        let cmd = format!("s0:{}", filename);
        self.send_command(device, &cmd)?;

        // Check status after delete
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
    /// ```rust
    /// let cbm = Cbm::new()?;
    /// cbm.validate_disk(8)?;
    /// ```
    pub fn validate_disk(&self, device: u8) -> Result<(), CbmError> {
        // Send validate command (V)
        self.send_command(device, "v")?;

        // Check status after validation
        self.get_status(device)?.into()
    }

    fn error_untalk_and_close_file(cbm: &OpenCbm, device: u8, channel_num: u8) {
        trace!("Cbm: Entered error_untalk_and_close_file");
        let _ = cbm
            .untalk()
            .inspect_err(|_| debug!("Untalk failed {} {}", device, channel_num));

        let _ = cbm
            .close_file(device, channel_num)
            .inspect_err(|_| debug!("Close file failed {} {}", device, channel_num));
        trace!("Cbm: Exited error_untalk_and_close_file");
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
    /// ```rust
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
    pub fn dir(&self, device: u8, drive_num: Option<u8>) -> Result<CbmDirListing, CbmError> {
        // Validate drive_num - must be None, Some(0) or Some(1)
        if let Some(drive_num) = drive_num {
            if drive_num > 1 {
                return Err(CbmError::InvalidOperation {
                    device,
                    message: format!("Invalid drive number {} - must be 0 or 1", drive_num),
                });
            }
        }

        trace!("Lock cbm");
        let guard = self.handle.lock();
        let cbm = guard
            .as_ref()
            .ok_or(CbmError::UsbError("No CBM handle".to_string()))?;

        // Construct directory command ("$" or "$0" or "$1")
        let dir_cmd = match drive_num {
            Some(num) => format!("${}", num),
            None => "$".to_string(),
        };
        trace!("Construct dir command {}", dir_cmd);

        trace!("Open file");
        let channel_num = 0;
        cbm.open_file(device, channel_num, &dir_cmd)
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Failed to open directory {}: {}", dir_cmd, e),
            })?;

        let mut output = String::new();

        // Check that open succeeded
        Self::get_status_already_locked(cbm, device).and_then(|status| {
            if status.is_ok() != CbmErrorNumberOk::Ok {
                Err(CbmError::CommandError {
                    device,
                    message: format!("Got error status after dir open {}", status),
                })
            } else {
                debug!("status value after dir open {}", status);
                Ok(())
            }
        })?;

        // Read the directory data
        cbm.talk(device, channel_num)
            .inspect_err(|_| {
                debug!("Talk command failed {} {}", device, channel_num);
                let _ = cbm.close_file(device, channel_num);
            })
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Talk failed: {}", e),
            })?;

        // Skip the load address (first two bytes)
        trace!("Read 2 bytes");
        let (_buf, result) = cbm
            .raw_read(2)
            .inspect_err(|_| Self::error_untalk_and_close_file(cbm, device, channel_num))
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Failed to read load address: {}", e),
            })?;

        if result == 2 {
            // Read directory entries
            loop {
                trace!("In read loop");
                // Read link address
                let (_, count) = cbm
                    .raw_read(2)
                    .inspect_err(|_| Self::error_untalk_and_close_file(cbm, device, channel_num))
                    .map_err(|e| CbmError::DeviceError {
                        device,
                        message: format!("Failed to read link address: {}", e),
                    })?;

                if count != 2 {
                    break;
                }

                // Read file size
                let (size_buf, size_count) = cbm
                    .raw_read(2)
                    .inspect_err(|_| Self::error_untalk_and_close_file(cbm, device, channel_num))
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
                    let (char_buf, char_count) = cbm
                        .raw_read(1)
                        .inspect_err(|_| {
                            Self::error_untalk_and_close_file(cbm, device, channel_num)
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
                output.push_str(&petscii_to_ascii(&filename));
                output.push('\n');
            }
        }

        // Cleanup
        cbm.untalk()
            .inspect_err(|_| Self::error_untalk_and_close_file(cbm, device, channel_num))
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Untalk failed: {}", e),
            })?;

        cbm.close_file(device, 0)
            .map_err(|e| CbmError::DeviceError {
                device,
                message: format!("Failed to close directory: {}", e),
            })?;

        // Get final status
        let status = Self::get_status_already_locked(cbm, device)?;
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
    /// ```rust
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
/// ```rust
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
    /// ```rust
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
    /// ```rust
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
    /// ```rust
    /// let mut drive = CbmDriveUnit::new(8, CbmDeviceType::Cbm4040);
    /// let cbm = Cbm::new()?;
    /// 
    /// // Initialize both drives, ignoring "drive not ready" errors
    /// let status = drive.send_init(cbm, &vec![CbmErrorNumber::DriveNotReady])?;
    /// ```
    pub fn send_init(
        &mut self,
        cbm: Cbm,
        ignore_errors: &Vec<CbmErrorNumber>,
    ) -> Result<Vec<CbmStatus>, CbmError> {
        self.busy = true;

        // First ? catches panic and maps to CbmError
        // Second > propagates CbmError (from first, or from within {})
        let mut status_vec: Vec<CbmStatus> = Vec::new();
        catch_unwind(AssertUnwindSafe(|| {
            self.num_disk_drives_iter().try_for_each(|ii| {
                let cmd = format!("i{}", ii);
                cbm.send_command(self.device_number, &cmd)
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
    /// ```rust
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
    /// ```rust
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
    /// ```rust
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
    /// ```rust
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
/// ```rust
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
    /// ```rust
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
/// ```rust
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
    /// ```rust
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
