use crate::cbm::Cbm;
use crate::cbmtype::{CbmErrorNumber, CbmErrorNumberOk, CbmStatus, CbmDeviceInfo};
use crate::channel::CbmChannelManager;
use crate::error::{DeviceError, Error};
use crate::CbmDirListing;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use parking_lot::Mutex;
use std::fmt;
use std::sync::Arc;

/// Represents a physical drive unit
///
/// Manages the channels and state for a single physical drive unit,
/// which may contain one or two drives.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CbmDriveUnit {
    pub device_number: u8,
    pub device_info: CbmDeviceInfo,
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
        write!(f, "Drive {} ({})", self.device_number, self.device_info)
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
    /// Tests whether a drive exists and if so, detects the type and creates
    /// a CbmDriveUnit object for it.
    pub fn try_from_bus(cbm: &Cbm, device: u8) -> Result<Self, Error> {
        if cbm.drive_exists(device)? {
            let info = cbm.identify(device)?;
            Ok(Self::new(device, info))
        } else {
            Err(Error::Device {
                device,
                error: DeviceError::NoDevice,
            })
        }
    }

    /// Creates a new drive unit instance.
    ///
    /// This function creates a new drive unit with the specified device number
    /// and type. It initializes the channel manager but does not perform any
    /// hardware communication.
    ///
    /// You may prefer [`CbmDriveUnit::try_from_bus`] as this will check the
    /// device exists and automatically get it's type before creating.
    ///
    /// # Arguments
    ///
    /// * `device_number` - The IEC device number
    /// * `device_info` - The [`crate::CbmDeviceInfo`]

    pub fn new(device_number: u8, device_info: CbmDeviceInfo) -> Self {
        // Test whether this device is actually attached
        Self {
            device_number,
            device_info,
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
    /// Returns `Error` if:
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
    pub fn get_status(&mut self, cbm: &Cbm) -> Result<CbmStatus, Error> {
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
    /// # Returns
    /// `Vec<Result<CbmStatus, Error>>` - A vector of status messages, or errors, one for each drive
    ///
    /// `Error` is used if:
    /// - Any drive fails to initialize (unless its error is in ignore_errors)
    /// - The command cannot be sent
    /// - The driver is not open
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rs1541::{Cbm, CbmDriveUnit, CbmDeviceType, CbmErrorNumber};
    /// let mut cbm = Cbm::new().unwrap();
    /// let mut drive = CbmDriveUnit::try_from_bus(&cbm, 8).unwrap();
    ///
    /// // Initialize all drives, ignoring "drive not ready" errors
    /// let status_vec = drive.send_init(&mut cbm, &vec![CbmErrorNumber::DriveNotReady]);
    /// // Now process the status_vec
    /// ```
    pub fn send_init(
        &mut self,
        cbm: &mut Cbm,
        ignore_errors: &Vec<CbmErrorNumber>,
    ) -> Vec<Result<CbmStatus, Error>> {
        self.busy = true;
        let mut results = Vec::new();

        for ii in self.num_disk_drives_iter() {
            let cmd = format!("i{}", ii);
            let status = match cbm.send_string_command_ascii(self.device_number, &cmd) {
                Ok(_) => match cbm.get_status(self.device_number) {
                    Ok(status) => {
                        if status.is_ok() != CbmErrorNumberOk::Ok
                            && !ignore_errors.contains(&status.error_number)
                        {
                            Err(status.into())
                        } else {
                            Ok(status)
                        }
                    }
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            };
            results.push(status);
        }

        self.busy = false;
        results
    }

    #[allow(dead_code)]
    fn reset(&mut self) -> Result<(), Error> {
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
        self.device_info.device_type.num_disk_drives()
    }

    pub fn device_info(&self) -> &CbmDeviceInfo {
        &self.device_info
    }

    pub fn device_type_str(&self) -> &str {
        self.device_info.device_type.as_str()
    }

    pub fn description(&self) -> &str {
        &self.device_info.description
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

    /// Does a directory for all disk units in this drive
    ///
    /// # Returns
    ///
    /// [`(Vec<CbmDirListing>, CbmStatus)`] - If at least partially successful.  The CbmStatus will be from the first failure if one occured.  Partial success means a directory listing was secured from at least one of the drives, and we didn't hit a fatal error during communication
    /// [`Error`] - if a serious error occurred.
    pub fn dir(&self, cbm: &mut Cbm) -> Result<(Vec<CbmDirListing>, CbmStatus), Error> {
        let mut results = Vec::new();

        // Single unit drives do not like to be asked to load $0 - so don't
        // pass in the drive unit number in that case
        // A single drive unit will respond 74 if asked to dir $0
        let single_drive_unit = self.num_disk_drives() == 1;
        let mut error_status = None;

        // Do dir for all drive units
        for ii in self.num_disk_drives_iter() {
            debug!("Doing dir of device {} drive {}", self.device_number, ii);
            let drive_unit_num = if single_drive_unit { None } else { Some(ii) };
            match cbm.dir(self.device_number, drive_unit_num) {
                Err(e @ Error::Device { .. })=>
                {
                    debug!(
                        "Got error trying to dir device {} drive {}: {}",
                        self.device_number, ii, e
                    );
                    continue;
                }
                Err(Error::Status { status, .. }) => {
                    debug!(
                        "Got error status trying to dir device {} drive {}: {}",
                        self.device_number, ii, status
                    );
                    error_status = Some(status.clone());
                    continue;
                }
                Err(e) => {
                    info!(
                        "Hit unrecoverable error trying to dir device {} drive {}: {}",
                        self.device_number, ii, e
                    );
                    return Err(e);
                }
                Ok(result) => {
                    results.push(result);
                }
            }
        }

        // If we have an error status return that.  Otherwise do a final status check now and return that 
        let status = error_status.unwrap_or(cbm.get_status(self.device_number)?);

        Ok((results, status))
    }
}
