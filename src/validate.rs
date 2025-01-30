use log::trace;

use crate::Error;
use crate::{DEFAULT_DEVICE_NUM, DEVICE_MAX_NUM, DEVICE_MIN_NUM};

pub enum DeviceValidation {
    Required, // Cannot be None
    Optional, // Can be None, None returned if so
    Default,  // Can be None, default returned if so
}

/// Validate a device Option, making sure the device is in the support range.
///
/// Args
/// * device - the device_num Option to validate
/// * type   - the device validation type
pub fn validate_device(
    device: Option<u8>,
    validation: DeviceValidation,
) -> Result<Option<u8>, Error> {
    match (device, validation) {
        (Some(nm), _) => {
            if nm < DEVICE_MIN_NUM || nm > DEVICE_MAX_NUM {
                trace!("Device num out of allowed range {}", nm);
                Err(Error::Validation {
                    message: format!(
                        "Device num must be between {} and {}",
                        DEVICE_MIN_NUM, DEVICE_MAX_NUM
                    ),
                })
            } else {
                trace!("Device num in allowed range {}", nm);
                Ok(device)
            }
        }
        (None, DeviceValidation::Required) => {
            trace!("Error - no device num supplied");
            Err(Error::Validation {
                message: format!("No device num supplied"),
            })
        }
        (None, DeviceValidation::Optional) => Ok(None),
        (None, DeviceValidation::Default) => Ok(Some(DEFAULT_DEVICE_NUM)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod device_validation {
        use super::*;

        #[test]
        fn test_valid_numbers() {
            // Valid numbers should work with any validation mode
            assert!(matches!(
                validate_device(Some(8), DeviceValidation::Required),
                Ok(Some(8))
            ));
            assert!(matches!(
                validate_device(Some(15), DeviceValidation::Optional),
                Ok(Some(15))
            ));
            assert!(matches!(
                validate_device(Some(10), DeviceValidation::Default),
                Ok(Some(10))
            ));
        }

        #[test]
        fn test_invalid_numbers() {
            // Invalid numbers should fail regardless of validation mode
            assert!(validate_device(Some(7), DeviceValidation::Required).is_err());
            assert!(validate_device(Some(31), DeviceValidation::Optional).is_err());
            assert!(validate_device(Some(255), DeviceValidation::Default).is_err());
        }

        #[test]
        fn test_none_handling() {
            // Required - None not allowed
            assert!(validate_device(None, DeviceValidation::Required).is_err());

            // Optional - None is allowed and returns None
            assert!(matches!(
                validate_device(None, DeviceValidation::Optional),
                Ok(None)
            ));

            // Default - None returns the default device number
            assert!(matches!(
                validate_device(None, DeviceValidation::Default),
                Ok(Some(DEFAULT_DEVICE_NUM))
            ));
        }
    }
}
