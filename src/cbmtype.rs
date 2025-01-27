use crate::Error;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbmStatus {
    pub number: u8,
    pub error_number: CbmErrorNumber,
    pub message: String,
    pub track: u8,
    pub sector: u8,
    pub device: u8,
}

// Required trait implementations
impl Default for CbmStatus {
    fn default() -> Self {
        CbmStatus {
            number: 255,
            error_number: CbmErrorNumber::default(),
            message: "none".to_string(),
            track: 0,
            sector: 0,
            device: 0,
        }
    }
}

impl CbmStatus {
    pub fn new(status: &str, device: u8) -> Result<Self, Error> {
        trace!("Received device status from device {}: {}", device, status);
        trace!("Status bytes: {:?}", status.as_bytes());

        let clean_status = if let Some(pos) = status.find("\r") {
            &status[..pos]
        } else {
            status
        };

        debug!(
            "Received cleaned device status: {}, length {}",
            clean_status,
            clean_status.len()
        );

        if clean_status.len() == 0 {
            return Err(Error::Parse {
                message: format!("Device {device} provided zero length status string"),
            });
        }

        let parts: Vec<&str> = clean_status.split(',').collect();
        if parts.len() != 4 {
            return Err(Error::Parse {
                message: format!("Device {device} supplied status format: {clean_status}"),
            });
        }

        let number = parts[0].trim().parse::<u8>().map_err(|_| Error::Parse {
            message: format!(
                "Device {device}: Invalid error number: {} within status: {}",
                parts[0], clean_status
            ),
        })?;
        let error_number = number.into();
        if error_number == CbmErrorNumber::Unknown {
            warn!("Unknown Error Number (EN) returned by drive: {}", number);
        }

        let message = parts[1].trim().to_string();

        let track = parts[2].trim().parse::<u8>().map_err(|_| Error::Parse {
            message: format!(
                "Device {device}: Invalid track: {} within status: {}",
                parts[2], clean_status
            ),
        })?;

        let sector = parts[3]
            .trim()
            .trim_end_matches('\n')
            .trim()
            .parse::<u8>()
            .map_err(|_| Error::Parse {
                message: format!(
                    "Device {device}: Invalid sector: {} within status: {}",
                    parts[3], clean_status
                ),
            })?;

        Ok(Self {
            number,
            error_number,
            message,
            track,
            sector,
            device,
        })
    }

    pub fn is_ok(&self) -> CbmErrorNumberOk {
        if self.number < 20 {
            CbmErrorNumberOk::Ok
        } else if self.number == 73 {
            CbmErrorNumberOk::Number73
        } else {
            CbmErrorNumberOk::Err
        }
    }

    /// Useful for checking drive gave us any valid response
    /// This means it's working even if the disk isn't inserted, is corrupt, etc
    pub fn is_valid_cbm(&self) -> bool {
        self.error_number != CbmErrorNumber::Unknown
    }

    pub fn track(&self) -> Option<u8> {
        if matches!(self.number, 20..=29) {
            Some(self.track)
        } else {
            None
        }
    }

    pub fn sector(&self) -> Option<u8> {
        if matches!(self.number, 20..=29) {
            Some(self.sector)
        } else {
            None
        }
    }

    pub fn files_scratched(&self) -> Option<u8> {
        if self.error_number == CbmErrorNumber::FilesScratched {
            Some(self.track)
        } else {
            None
        }
    }

    pub fn as_short_str(&self) -> String {
        format!("{:02},{}", self.number, self.message)
    }

    pub fn as_str(&self) -> String {
        format!(
            "{:02},{},{:02},{:02}",
            self.number, self.message, self.track, self.sector
        )
    }
}

impl TryFrom<(&str, u8)> for CbmStatus {
    type Error = Error;

    fn try_from((s, device): (&str, u8)) -> Result<Self, Self::Error> {
        Self::new(s, device)
    }
}

impl fmt::Display for CbmStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02},{},{:02},{:02}",
            self.number, self.message, self.track, self.sector
        )
    }
}

impl Into<Result<(), Error>> for CbmStatus {
    fn into(self) -> Result<(), Error> {
        match self.is_ok() {
            CbmErrorNumberOk::Ok => Ok(()),
            CbmErrorNumberOk::Number73 => Err(self.into()),
            CbmErrorNumberOk::Err => Err(self.into()),
        }
    }
}

impl CbmStatus {
    pub fn into_73_ok(self) -> Result<(), Error> {
        if self.is_ok() == CbmErrorNumberOk::Number73 {
            Ok(())
        } else {
            Err(self.into())
        }
    }
}

#[derive(Debug, Clone)]
pub struct CbmDeviceInfo {
    pub device_type: CbmDeviceType,
    pub description: String,
}

impl Default for CbmDeviceInfo {
    fn default() -> Self {
        CbmDeviceInfo {
            device_type: CbmDeviceType::default(),
            description: "unknown".to_string(),
        }
    }
}

impl fmt::Display for CbmDeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.device_type, self.description)
    }
}

impl CbmDeviceInfo {
    pub fn from_magic(magic: u16, magic2: Option<u16>) -> Self {
        let (device_type, description) = match magic {
            0xfeb6 => (CbmDeviceType::Cbm2031, String::from("2031")),
            0xaaaa => match magic2 {
                Some(magic2) => match magic2 {
                    0x3156 => (CbmDeviceType::Cbm1540, String::from("1540")),
                    0xfeb6 => (CbmDeviceType::Cbm2031, String::from("2031")),
                    _ => (CbmDeviceType::Cbm1541, String::from("1541")),
                },
                None => (CbmDeviceType::Cbm1541, String::from("1541")),
            },
            0xf00f => (CbmDeviceType::Cbm1541, String::from("1541-II")),
            0xcd18 => (CbmDeviceType::Cbm1541, String::from("1541C")),
            0x10ca => (CbmDeviceType::Cbm1541, String::from("DolphinDOS 1541")),
            0x6f10 => (CbmDeviceType::Cbm1541, String::from("SpeedDOS 1541")),
            0x2710 => (CbmDeviceType::Cbm1541, String::from("ProfessionalDOS 1541")),
            0x8085 => (CbmDeviceType::Cbm1541, String::from("JiffyDOS 1541")),
            0xaeea => (CbmDeviceType::Cbm1541, String::from("64'er DOS 1541")),
            0x180d => (
                CbmDeviceType::Cbm1541,
                String::from("Turbo Access / Turbo Trans"),
            ),
            0x094c => (CbmDeviceType::Cbm1541, String::from("Prologic DOS")),
            0xfed7 => (CbmDeviceType::Cbm1570, String::from("1570")),
            0x02ac => (CbmDeviceType::Cbm1571, String::from("1571")),
            0x01ba => match magic2 {
                Some(0x4446) => (CbmDeviceType::FdX000, String::from("FD2000/FD4000")),
                _ => (CbmDeviceType::Cbm1581, String::from("1581")),
            },
            0x32f0 => (CbmDeviceType::Cbm3040, String::from("3040")),
            0xc320 | 0x20f8 => (CbmDeviceType::Cbm4040, String::from("4040")),
            0xf2e9 => (CbmDeviceType::Cbm8050, String::from("8050 dos2.5")),
            0xc866 | 0xc611 => (CbmDeviceType::Cbm8250, String::from("8250 dos2.7")),
            _ => match magic2 {
                Some(m2) => (
                    CbmDeviceType::Unknown,
                    format!("Unknown device: {:04x} {:04x}", magic, m2),
                ),
                None => (
                    CbmDeviceType::Unknown,
                    format!("Unknown device: {:04x}", magic),
                ),
            },
        };

        CbmDeviceInfo {
            device_type,
            description,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(into = "i32", from = "i32")]
pub enum CbmDeviceType {
    Unknown = -1,
    Cbm1540 = 0,
    Cbm1541 = 1,
    Cbm1570 = 2,
    Cbm1571 = 3,
    Cbm1581 = 4,
    Cbm2040 = 5,
    Cbm2031 = 6,
    Cbm3040 = 7,
    Cbm4040 = 8,
    Cbm4031 = 9,
    Cbm8050 = 10,
    Cbm8250 = 11,
    Sfd1001 = 12,
    FdX000 = 13,
}

impl Default for CbmDeviceType {
    fn default() -> Self {
        CbmDeviceType::Unknown
    }
}

impl fmt::Display for CbmDeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CbmDeviceType::Unknown => write!(f, "Unknown"),
            CbmDeviceType::Cbm1540 => write!(f, "CBM 1540"),
            CbmDeviceType::Cbm1541 => write!(f, "CBM 1541"),
            CbmDeviceType::Cbm1570 => write!(f, "CBM 1570"),
            CbmDeviceType::Cbm1571 => write!(f, "CBM 1571"),
            CbmDeviceType::Cbm1581 => write!(f, "CBM 1581"),
            CbmDeviceType::Cbm2040 => write!(f, "CBM 2040"),
            CbmDeviceType::Cbm2031 => write!(f, "CBM 2031"),
            CbmDeviceType::Cbm3040 => write!(f, "CBM 3040"),
            CbmDeviceType::Cbm4040 => write!(f, "CBM 4040"),
            CbmDeviceType::Cbm4031 => write!(f, "CBM 4031"),
            CbmDeviceType::Cbm8050 => write!(f, "CBM 8050"),
            CbmDeviceType::Cbm8250 => write!(f, "CBM 8250"),
            CbmDeviceType::Sfd1001 => write!(f, "SFD 1001"),
            CbmDeviceType::FdX000 => write!(f, "FD X000"),
        }
    }
}

impl From<i32> for CbmDeviceType {
    fn from(value: i32) -> Self {
        match value {
            -1 => Self::Unknown,
            0 => Self::Cbm1541,
            1 => Self::Cbm1541,
            2 => Self::Cbm1570,
            3 => Self::Cbm1571,
            4 => Self::Cbm1581,
            5 => Self::Cbm2040,
            6 => Self::Cbm2031,
            7 => Self::Cbm3040,
            8 => Self::Cbm4040,
            9 => Self::Cbm4031,
            10 => Self::Cbm8050,
            11 => Self::Cbm8250,
            12 => Self::Sfd1001,
            13 => Self::FdX000,
            _ => Self::Unknown,
        }
    }
}

impl From<CbmDeviceType> for i32 {
    fn from(value: CbmDeviceType) -> Self {
        value as i32
    }
}

impl CbmDeviceType {
    pub fn to_fs_name(&self) -> String {
        match self {
            Self::Unknown => "Unknown".to_string(),
            Self::FdX000 => self.as_str().to_string(),
            _ => format!("CBM_{}", self.as_str()),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown Device",
            Self::Cbm1540 => "1540",
            Self::Cbm1541 => "1541",
            Self::Cbm1570 => "1570",
            Self::Cbm1571 => "1571",
            Self::Cbm1581 => "1581",
            Self::Cbm2040 => "2040",
            Self::Cbm2031 => "2031",
            Self::Cbm3040 => "3040",
            Self::Cbm4040 => "4040",
            Self::Cbm4031 => "4031",
            Self::Cbm8050 => "8050",
            Self::Cbm8250 => "8250",
            Self::Sfd1001 => "SFD-1001",
            Self::FdX000 => "FDX000",
        }
    }

    pub fn num_disk_drives(&self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Cbm1540 => 1,
            Self::Cbm1541 => 1,
            Self::Cbm1570 => 1,
            Self::Cbm1571 => 1,
            Self::Cbm1581 => 1,
            Self::Cbm2040 => 2,
            Self::Cbm2031 => 1,
            Self::Cbm3040 => 2,
            Self::Cbm4040 => 2,
            Self::Cbm4031 => 1,
            Self::Cbm8050 => 2,
            Self::Cbm8250 => 2,
            Self::Sfd1001 => 1,
            Self::FdX000 => 1,
        }
    }

    pub fn dos_version(&self) -> DosVersion {
        match self {
            Self::Cbm1540 => DosVersion::Dos2,
            Self::Cbm1541 => DosVersion::Dos2,
            Self::Cbm1570 => DosVersion::Dos2,
            Self::Cbm1571 => DosVersion::Dos3,
            Self::Cbm1581 => DosVersion::Dos3,
            Self::Cbm2040 => DosVersion::Dos1,
            Self::Cbm2031 => DosVersion::Dos2,
            Self::Cbm3040 => DosVersion::Dos1,
            Self::Cbm4040 => DosVersion::Dos2,
            Self::Cbm4031 => DosVersion::Dos2,
            Self::Cbm8050 => DosVersion::Dos2,
            Self::Cbm8250 => DosVersion::Dos2,
            Self::Sfd1001 => DosVersion::Dos2,
            Self::FdX000 => DosVersion::Dos3,
            Self::Unknown => DosVersion::Dos1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DosVersion {
    Dos1,
    Dos2,
    Dos3,
}

impl fmt::Display for DosVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DosVersion::Dos1 => "DOS1",
            DosVersion::Dos2 => "DOS2",
            DosVersion::Dos3 => "DOS3",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CbmErrorNumber {
    Ok = 0,
    FilesScratched = 1,
    ReadErrorBlockHeaderNotFound = 20,
    ReadErrorNoSyncCharacter = 21,
    ReadErrorDataBlockNotPresent = 22,
    ReadErrorChecksumErrorInDataBlock = 23,
    ReadErrorByteDecodingError = 24,
    WriteErrorWriteVerifyError = 25,
    WriteProtectOn = 26,
    ReadErrorChecksumErrorInHeader = 27,
    WriteErrorLongDataBlock = 28,
    DiskIdMismatch = 29,
    SyntaxErrorGeneralSyntax = 30,
    SyntaxErrorInvalidCommand = 31,
    SyntaxErrorLongLine = 32,
    SyntaxErrorInvalidFileName = 33,
    SyntaxErrorNoFileGiven = 34,
    SyntaxErrorInvalidCommandChannel15 = 39,
    RecordNotPresent = 50,
    OverflowInRecord = 51,
    FileTooLarge = 52,
    WriteFileOpen = 60,
    FileNotOpen = 61,
    FileNotFound = 62,
    FileExists = 63,
    FileTypeMismatch = 64,
    NoBlock = 65,
    IllegalTrackAndSector = 66,
    IllegalSystemTOrS = 67,
    NoChannel = 70,
    DirectoryError = 71,
    DiskFull = 72,
    DosMismatch = 73,
    DriveNotReady = 74,
    Unknown = 255,
}

impl Default for CbmErrorNumber {
    fn default() -> Self {
        CbmErrorNumber::Unknown
    }
}

impl From<u8> for CbmErrorNumber {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Ok,
            1 => Self::FilesScratched,
            20 => Self::ReadErrorBlockHeaderNotFound,
            21 => Self::ReadErrorNoSyncCharacter,
            22 => Self::ReadErrorDataBlockNotPresent,
            23 => Self::ReadErrorChecksumErrorInDataBlock,
            24 => Self::ReadErrorByteDecodingError,
            25 => Self::WriteErrorWriteVerifyError,
            26 => Self::WriteProtectOn,
            27 => Self::ReadErrorChecksumErrorInHeader,
            28 => Self::WriteErrorLongDataBlock,
            29 => Self::DiskIdMismatch,
            30 => Self::SyntaxErrorGeneralSyntax,
            31 => Self::SyntaxErrorInvalidCommand,
            32 => Self::SyntaxErrorLongLine,
            33 => Self::SyntaxErrorInvalidFileName,
            34 => Self::SyntaxErrorNoFileGiven,
            39 => Self::SyntaxErrorInvalidCommandChannel15,
            50 => Self::RecordNotPresent,
            51 => Self::OverflowInRecord,
            52 => Self::FileTooLarge,
            60 => Self::WriteFileOpen,
            61 => Self::FileNotOpen,
            62 => Self::FileNotFound,
            63 => Self::FileExists,
            64 => Self::FileTypeMismatch,
            65 => Self::NoBlock,
            66 => Self::IllegalTrackAndSector,
            67 => Self::IllegalSystemTOrS,
            70 => Self::NoChannel,
            71 => Self::DirectoryError,
            72 => Self::DiskFull,
            73 => Self::DosMismatch,
            74 => Self::DriveNotReady,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for CbmErrorNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CbmErrorNumber::Ok => "OK",
            CbmErrorNumber::FilesScratched => "FILES SCRATCHED",
            CbmErrorNumber::ReadErrorBlockHeaderNotFound => "READ ERROR (block header not found)",
            CbmErrorNumber::ReadErrorNoSyncCharacter => "READ ERROR (no sync character)",
            CbmErrorNumber::ReadErrorDataBlockNotPresent => "READ ERROR (data block not present)",
            CbmErrorNumber::ReadErrorChecksumErrorInDataBlock => {
                "READ ERROR (checksum error in data block)"
            }
            CbmErrorNumber::ReadErrorByteDecodingError => "READ ERROR (byte decoding error)",
            CbmErrorNumber::WriteErrorWriteVerifyError => "WRITE ERROR (write verify error)",
            CbmErrorNumber::WriteProtectOn => "WRITE PROTECT ON",
            CbmErrorNumber::ReadErrorChecksumErrorInHeader => {
                "READ ERROR (checksum error in header)"
            }
            CbmErrorNumber::WriteErrorLongDataBlock => "WRITE ERROR (long data block)",
            CbmErrorNumber::DiskIdMismatch => "DISK ID MISMATCH",
            CbmErrorNumber::SyntaxErrorGeneralSyntax => "SYNTAX ERROR (general syntax)",
            CbmErrorNumber::SyntaxErrorInvalidCommand => "SYNTAX ERROR (invalid command)",
            CbmErrorNumber::SyntaxErrorLongLine => "SYNTAX ERROR (long line)",
            CbmErrorNumber::SyntaxErrorInvalidFileName => "SYNTAX ERROR (invalid file name)",
            CbmErrorNumber::SyntaxErrorNoFileGiven => "SYNTAX ERROR (no file given))",
            CbmErrorNumber::SyntaxErrorInvalidCommandChannel15 => {
                "SYNTAX ERROR (invalid command on channel 15)"
            }
            CbmErrorNumber::RecordNotPresent => "RECORD NOT PRESENT",
            CbmErrorNumber::OverflowInRecord => "OVERFLOW IN RECORD",
            CbmErrorNumber::FileTooLarge => "FILE TOO LARGE",
            CbmErrorNumber::WriteFileOpen => "WRITE FILE OPEN",
            CbmErrorNumber::FileNotOpen => "FILE NOT OPEN",
            CbmErrorNumber::FileNotFound => "FILE NOT FOUND",
            CbmErrorNumber::FileExists => "FILE EXISTS",
            CbmErrorNumber::FileTypeMismatch => "FILE TYPE MISMATCH",
            CbmErrorNumber::NoBlock => "NO BLOCK",
            CbmErrorNumber::IllegalTrackAndSector => "ILLEGAL TRACK AND SECTOR",
            CbmErrorNumber::IllegalSystemTOrS => "ILLEGAL SYSTEM T OR S",
            CbmErrorNumber::NoChannel => "NO CHANNEL",
            CbmErrorNumber::DirectoryError => "DIRECTORY ERROR",
            CbmErrorNumber::DiskFull => "DISK FULL",
            CbmErrorNumber::DosMismatch => "DOS MISMATCH",
            CbmErrorNumber::DriveNotReady => "DRIVE NOT READY",
            CbmErrorNumber::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CbmErrorNumberOk {
    Ok,
    Err,
    Number73,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbmOperationType {
    Read,
    Write,
    Directory,
    Control,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct CbmOperation {
    op_type: CbmOperationType,
    count: usize,
    has_write: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_bad_status() {
        let result = CbmStatus::new("bibble bobble flibble flobble", 8);
        assert_eq!(
            result,
            Err(Error::Parse {
                message: "Device 8 supplied status format: bibble bobble flibble flobble"
                    .to_string()
            })
        );
    }

    #[test]
    fn test_status_parsing() {
        let status = CbmStatus::try_from(("21,READ ERROR,18,00", 8)).unwrap();
        assert_eq!(status.number, 21);
        assert_eq!(
            status.error_number,
            CbmErrorNumber::ReadErrorNoSyncCharacter
        );
        assert_eq!(status.message, "READ ERROR");
        assert_eq!(status.track, 18);
        assert_eq!(status.sector, 0);
        assert_eq!(status.device, 8);
        assert_eq!(status.is_ok(), CbmErrorNumberOk::Err);
    }

    #[test]
    fn test_ok_status() {
        let status = CbmStatus::try_from(("00,OK,00,00", 8)).unwrap();
        assert_eq!(status.is_ok(), CbmErrorNumberOk::Ok);
        assert_eq!(status.device, 8);
        assert_eq!(status.to_string(), "00,OK,00,00");
    }

    #[test]
    fn test_73_status() {
        let status = CbmStatus::try_from(("73,DOS MISMATCH,00,00", 8)).unwrap();
        assert_eq!(status.error_number, CbmErrorNumber::DosMismatch);
        assert_eq!(status.is_ok(), CbmErrorNumberOk::Number73);
        assert_eq!(status.to_string(), "73,DOS MISMATCH,00,00");
        assert_eq!(status.message, "DOS MISMATCH");
        assert_eq!(status.device, 8);
    }

    #[test]
    fn test_files_scratched() {
        let status = CbmStatus::try_from(("01,FILES SCRATCHED,03,00", 8)).unwrap();
        assert_eq!(status.files_scratched(), Some(3));
        assert_eq!(status.message, "FILES SCRATCHED");
        assert_eq!(status.is_ok(), CbmErrorNumberOk::Ok);
        assert_eq!(status.track, 3);
        assert_eq!(status.sector, 0);
        assert_eq!(status.device, 8);
    }

    #[test]
    fn test_read_error_display() {
        let status = CbmStatus::try_from(("21,READ ERROR,18,04", 8)).unwrap();
        assert_eq!(status.files_scratched(), None);
        assert_eq!(status.to_string(), "21,READ ERROR,18,04");
        assert_eq!(status.is_ok(), CbmErrorNumberOk::Err);
        assert_eq!(status.track, 18);
        assert_eq!(status.sector, 4);
        assert_eq!(status.device, 8);
    }

    #[test]
    fn test_error_display() {
        let error = Error::Validation {
            message: "Test error".to_string(),
        };
        assert_eq!(error.to_string(), "Validation error: Test error");

        let status = CbmStatus {
            number: 21,
            error_number: CbmErrorNumber::ReadErrorNoSyncCharacter,
            message: "READ ERROR".to_string(),
            track: 18,
            sector: 0,
            device: 8,
        };
        let error = Error::Status { status };
        assert_eq!(
            error.to_string(),
            "Device 8: Status error: 21,READ ERROR,18,00"
        );
    }
}
