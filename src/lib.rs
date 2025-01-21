//! # rs1541
//!
//! Rust bindings and helper functions for accessing Commodore disk drives through OpenCBM.
//!
//! ## Overview
//! This crate provides idiomatic Rust interfaces to OpenCBM, allowing easy control of
//! Commodore disk drives (like the 1541) using modern USB devices such as the XUM1541.
//! Thread-safe access to OpenCBM through protected mutex handles enables safe usage in
//! multi-threaded and async applications.
//!
//! ## Features
//! * Safe Rust wrappers around OpenCBM's C interface
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
//!     let cbm = Cbm::new()?;
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
//! * OpenCBM installed and configured
//! * XUM1541 (or compatible) USB device
//! * Appropriate permissions to access the USB device
//!
//! ## Error Handling
//! All operations that could fail return a [`Result`] type. Specific error
//! conditions are represented by the [`CbmError`] type, which wraps both
//! OpenCBM errors and drive-specific error codes.

/// Define rs1541 modules
pub mod cbm;
pub mod cbmtype;
pub mod error;
pub mod string;
pub mod util;
pub mod validate;

pub use cbm::{
    Cbm, CbmChannel, CbmChannelManager, CbmChannelPurpose, CbmDirListing, CbmDiskHeader,
    CbmDriveUnit, CbmFileEntry,
};
pub use cbmtype::{
    CbmDeviceInfo, CbmDeviceType, CbmErrorNumber, CbmErrorNumberOk, CbmFileMode, CbmFileType,
    CbmOperation, CbmOperationType, CbmStatus,
};
/// Export the public API
pub use error::CbmError;
pub use string::{AsciiString, CbmString, PetsciiString};
pub use util::{ascii_str_to_petscii, ascii_to_petscii, petscii_str_to_ascii, petscii_to_ascii};
pub use validate::{validate_device, DeviceValidation};

/// Minimum device number supported by Commodore disk drives
pub const MIN_DEVICE_NUM: u8 = 8;

/// Maximum device number supported by Commodor disk drives
/// At least the later devices (such as the 1571) can be set to support up to
/// device 30, in software
pub const MAX_DEVICE_NUM: u8 = 30;

/// Default device number for Commodore disk drives
pub const DEFAULT_DEVICE_NUM: u8 = 8;

/// USB Vendor ID for an XUM1541 device
pub const XUM1541_VENDOR_ID: &str = "16d0";

/// USB Product ID for an XUM1541 dvice
pub const XUM1541_PRODUCT_ID: &str = "0504";
