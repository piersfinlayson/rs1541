use crate::CbmStatus;
use libc::{EINVAL, EIO, ENODEV, ETIMEDOUT};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use xum1541::Error as Xum1541Error;
use xum1541::DeviceChannel;

#[derive(Debug, Error, PartialEq, Serialize, Deserialize)]
pub enum Error {
    /// Error from the XUM1541 device
    #[error("{0}")]
    Xum1541(#[from] Xum1541Error),

    /// Hit an error when operating with a device
    #[error("Device {device} error: {error}")]
    Device { device: u8, error: DeviceError },

    /// Hit an error when manipulating a file
    #[error("Device {}: File error: {message}", device)]
    File { device: u8, message: String },

    /// The drive responded with a status error
    #[error("Device {}: Status error: {status}", status.device)]
    Status { status: CbmStatus },

    /// This is distinct from Xum11541Error::Timeout, and is used to indicate
    /// a timeout error in this higher-level API.
    #[error("Timeout error, duration: {dur:?}")]
    Timeout { dur: std::time::Duration },

    /// Argument validation failed
    #[error("Validation error: {message}")]
    Validation { message: String },

    /// Parsing error, most likely on data received from the device
    #[error("Parse error: {message}")]
    Parse { message: String },
}

/// (CBM) Device errors
#[derive(Error, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum DeviceError {
    /// failed to get status from this device
    #[error("Failed to get status: {message}")]
    GetStatusFailure { message: String },

    /// Attmepted to read from an invalid drive number for this drive
    #[error("Invalid drive number: {drive_num}")]
    InvalidDrive { drive_num: u8 },

    /// Error reading from a channel
    #[error("Read error: Channel: {channel}, Error: {message}")]
    Read { channel: u8, message: String },

    /// Error writing to a channel
    #[error("Write error: Channel: {channel}, Error: {message}")]
    Write { channel: u8, message: String },

    /// Our best bet is that the device doesn't exist.  This is based on
    /// an attempt to retrieve status by putting the device into talk mode
    /// on channel 15 and failing to read a single byte
    #[error("Device does not exist (or at least isn't talking on channel 15)")]
    NoDevice,
}

impl From<CbmStatus> for Error {
    fn from(status: CbmStatus) -> Self {
        Error::Status { status }
    }
}

impl Error {
    /// Convert the error to a an errno
    pub fn to_errno(&self) -> i32 {
        match self {
            xum @ Error::Xum1541(_) => xum.to_errno(),
            e @ Error::Device { .. } => e.to_errno(),
            Error::File { .. } => EIO,
            Error::Timeout { .. } => ETIMEDOUT,
            Error::Validation { .. } => EINVAL,
            Error::Status { .. } => EIO,
            Error::Parse { message: _ } => EINVAL,
        }
    }
}

impl DeviceError {
    pub fn to_errno(&self) -> i32 {
        match self {
            DeviceError::GetStatusFailure { .. } => EIO,
            DeviceError::InvalidDrive { .. } => EINVAL,
            DeviceError::Read { .. } => EIO,
            DeviceError::Write { .. } => EIO,
            DeviceError::NoDevice { .. } => ENODEV,
        }
    }

    fn with_device(&self, device: u8) -> Error {
        Error::Device {
            device,
            error: self.clone(),
        }
    }

    pub fn invalid_drive_num(device: u8, drive_num: u8) -> Error {
        DeviceError::InvalidDrive { drive_num }.with_device(device)
    }

    pub fn read_error(dc: DeviceChannel, message: String) -> Error {
        DeviceError::Read {
            channel: dc.channel(),
            message,
        }
        .with_device(dc.device())
    }

    pub fn write_error(dc: DeviceChannel, message: String) -> Error {
        DeviceError::Write {
            channel: dc.channel(),
            message,
        }
        .with_device(dc.device())
    }

    pub fn get_status_failure(device: u8, message: String) -> Error {
        DeviceError::GetStatusFailure { message }.with_device(device)
    }

    pub fn no_device(device: u8) -> Error {
        DeviceError::NoDevice.with_device(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_errno() {
        let error = Error::Validation {
            message: "Test error".to_string(),
        };
        assert_eq!(error.to_errno(), EINVAL);
    }
}
