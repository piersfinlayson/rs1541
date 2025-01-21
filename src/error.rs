use xum1541::{DeviceAccessKind, Xum1541Error};
use crate::CbmStatus;
use libc::{ENODEV, EACCES, EBUSY, EINVAL, EIO, ENOENT, ENOTSUP, ENXIO, EPERM, ETIME};
use std::any::Any;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum CbmError {
    #[error("CbmError: {message}")]
    OtherError {
        message: String,
    },
    #[error("{0}")]
    Xum1541Error(#[from] Xum1541Error),
    #[error("{}: Device error: {message}", Self::format_device(Some(*device)))]
    DeviceError {
        device: u8,
        message: String,
    },
    #[error("{}: Channel error: {message}", Self::format_device(Some(*device)))]
    ChannelError {
        device: u8,
        message: String,
    },
    #[error("{}: File error: {message}", Self::format_device(Some(*device)))]
    FileError {
        device: u8,
        message: String,
    },
    #[error("{}: Command error: {message}", Self::format_device(Some(*device)))]
    CommandError {
        device: u8,
        message: String,
    },
    #[error("{}: Status error: {status}", Self::format_device(Some(*device)))]
    StatusError {
        device: u8,
        status: CbmStatus,
    },
    #[error("{}: Timeout error", Self::format_device(Some(*device)))]
    TimeoutError {
        device: u8,
    },
    #[error("{}: Invalid operation: {message}", Self::format_device(Some(*device)))]
    InvalidOperation {
        device: u8,
        message: String,
    },
    #[error("System error: {}", std::io::Error::from_raw_os_error(0))]
    Errno(i32),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("USB error: {0}")]
    UsbError(String),
    #[error("Parse error: {message}")]
    ParseError {
        message: String,
    },
    #[error("Driver not open")]
    DriverNotOpen,
}

impl From<CbmStatus> for CbmError {
    fn from(status: CbmStatus) -> Self {
        CbmError::StatusError {
            device: status.device,
            status,
        }
    }
}

impl CbmError {
    /// Convert the error to a an errno
    pub fn to_errno(&self) -> i32 {
        match self {
            CbmError::OtherError { .. } => EPERM,
            CbmError::Xum1541Error(error) => match error {
                Xum1541Error::Usb { .. } => EIO,
                Xum1541Error::Init { .. } => EPERM,
                Xum1541Error::Communication { .. } => EIO,
                Xum1541Error::Timeout { .. } => ETIME,
                Xum1541Error::DeviceAccess { kind } => match kind {
                    DeviceAccessKind::NotFound { .. } => ENOENT,
                    DeviceAccessKind::SerialMismatch { .. } => ENOENT,
                    DeviceAccessKind::FirmwareVersion { .. } => ENODEV,
                    DeviceAccessKind::Permission { .. } => EACCES,

                },
                Xum1541Error::Args { .. } => EINVAL,
            },
            CbmError::DeviceError { .. } => EIO,
            CbmError::ChannelError { .. } => EBUSY,
            CbmError::FileError { .. } => ENOENT,
            CbmError::CommandError { .. } => EIO,
            CbmError::TimeoutError { .. } => EIO,
            CbmError::InvalidOperation { .. } => ENOTSUP,
            CbmError::Errno(errno) => *errno,
            CbmError::ValidationError { .. } => EINVAL,
            CbmError::StatusError { .. } => EPERM,
            CbmError::UsbError(_msg) => ENXIO,
            CbmError::ParseError { message: _ } => EINVAL,
            CbmError::DriverNotOpen => ENXIO,
        }
    }

    /// Helper function to format device number for display
    fn format_device(device: Option<u8>) -> String {
        match device {
            Some(dev) => format!("Device {}", dev),
            None => "n/a".to_string(),
        }
    }
}

impl From<Box<dyn Any + Send>> for CbmError {
    fn from(error: Box<dyn Any + Send>) -> Self {
        let msg = if let Some(s) = error.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = error.downcast_ref::<&str>() {
            s.to_string()
        } else {
            "Unknown panic".to_string()
        };

        CbmError::DeviceError {
            device: 0,
            message: format!("Panic in opencbm: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_errno() {
        let error = CbmError::DeviceError {
            device: 8,
            message: "Test error".to_string(),
        };
        assert_eq!(error.to_errno(), EIO);

        let error = CbmError::Errno(ENOENT);
        assert_eq!(error.to_errno(), ENOENT);
    }
}