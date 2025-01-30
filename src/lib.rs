//! # rs1541
//!
//! Rust bindings and helper functions for accessing Commodore disk drives.
//!
//! ## Overview
//! This crate provides idiomatic Rust interfaces allowing easy control of
//! Commodore disk drives (like the 1541) using modern USB devices such as the XUM1541.
//! Thread-safe access is provided through protected mutex handles enables safe usage in
//! multi-threaded and async applications.
//!
//! The primary object is [`Cbm`].
//!
//! ## Features
//! * RAII-based driver management - no manual open/close needed
//! * Thread-safe access for multi-threaded and async applications
//! * Ergonomic error handling using Rust's Result type
//! * Directory parsing with structured data types
//! * Strong typing for CBM-specific concepts (error codes, status messages, etc.)
//!
//! ## Quick Start
//! ```ignore
//! use rs1541::Cbm;
//! use std::error::Error;
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     // Driver automatically opens on creation and closes on drop
//!     let cbm = Cbm::new_usb(None)?;
//!
//!     // Get drive information
//!     let id = cbm.identify(8)?;
//!     println!("Drive type at device 8: {}", id);
//!
//!     // Check drive status
//!     let status = cbm.get_status(8)?;
//!     println!("Drive status: {}", status);
//!
//!     // Read directory
//!     let dir = cbm.dir(8, None)?;
//!     println!("Directory listing:\n{}", dir);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Requirements
//! * Rust installed on your system
//! * XUM1541 (or compatible) USB device
//! * Appropriate permissions to access the USB device
//!
//! ## Error Handling
//! All operations that could fail return a [`Result`] type. Specific error
//! conditions are represented by the [`Error`] type, which wraps both
//! XUM1541 errors and drive-specific error codes.

// Define rs1541 modules
pub mod cbm;
pub mod cbmtype;
pub mod channel;
pub mod disk;
pub mod drive;
pub mod error;
pub mod string;
pub mod util;
pub mod validate;

/// Export the public API
pub use cbm::Cbm;
/// USB implementation of the Cbm object, used to create and use a USB connected XUM1541
pub type UsbCbm = Cbm<UsbDevice>;
pub type RemoteUsbCbm = Cbm<RemoteUsbDevice>;
pub use cbmtype::{
    CbmDeviceInfo, CbmDeviceType, CbmErrorNumber, CbmErrorNumberOk, CbmOperation, CbmOperationType,
    CbmStatus, DosVersion,
};
pub use channel::{CbmChannel, CbmChannelManager, CbmChannelPurpose};
pub use channel::{CBM_CHANNEL_CTRL, CBM_CHANNEL_LOAD};
pub use disk::{CbmDirListing, CbmDiskHeader, CbmFileEntry, CbmFileType};
pub use drive::CbmDriveUnit;
pub use error::{DeviceError, Error};
pub use string::{AsciiString, CbmString, PetsciiString};
pub use util::{ascii_str_to_petscii, ascii_to_petscii, petscii_str_to_ascii, petscii_to_ascii};
pub use validate::{validate_device, DeviceValidation};

// Export DeviceChannel as we use in our API
pub use xum1541::DeviceAccessError;
pub use xum1541::DeviceChannel;
pub use xum1541::Error as Xum1541Error;
pub use xum1541::{Device, RemoteUsbDevice, UsbDevice};

/// A trait to allow us to get the Bus as a reference from a MutexGuard and
/// automatically convert the None case to a Error
trait BusGuardRef<D>
where
    D: Device,
{
    fn bus_ref_or_err(&self) -> Result<&xum1541::Bus<D>, Error>;
}

impl<D: Device> BusGuardRef<D> for parking_lot::MutexGuard<'_, Option<xum1541::Bus<D>>> {
    fn bus_ref_or_err(&self) -> Result<&xum1541::Bus<D>, Error> {
        self.as_ref()
            .ok_or(Error::Xum1541(xum1541::Error::DeviceAccess {
                kind: xum1541::DeviceAccessError::NoDevice,
            }))
    }
}

/// A trait to allow us to get the Bus as a mutable reference from a
/// MutexGuard and automatically convert the None case to a Error
trait BusGuardMut<D>
where
    D: Device,
{
    fn bus_mut_or_err(&mut self) -> Result<&mut xum1541::Bus<D>, Error>;
}

impl<'a, D: Device> BusGuardMut<D> for parking_lot::MutexGuard<'_, Option<xum1541::Bus<D>>> {
    fn bus_mut_or_err(&mut self) -> Result<&mut xum1541::Bus<D>, Error> {
        self.as_mut()
            .ok_or(Error::Xum1541(xum1541::Error::DeviceAccess {
                kind: xum1541::DeviceAccessError::NoDevice,
            }))
    }
}

pub use xum1541::constants::{DEVICE_MAX_NUM, DEVICE_MIN_NUM};

/// Default device number for Commodore disk drives
pub const DEFAULT_DEVICE_NUM: u8 = 8;

/// USB Vendor ID for an XUM1541 device
pub const XUM1541_VENDOR_ID: &str = "16d0";

/// USB Product ID for an XUM1541 dvice
pub const XUM1541_PRODUCT_ID: &str = "0504";
