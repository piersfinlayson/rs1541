//! Contains types and functions for working with Commodore files and
//! directories

use crate::error::Error;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::fmt;

const BYTES_PER_BLOCK: u64 = 254;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbmFileType {
    PRG,
    SEQ,
    USR,
    REL,
    Unknown,
}

impl CbmFileType {
    pub fn _to_suffix(&self) -> &'static str {
        match self {
            CbmFileType::PRG => ",P",
            CbmFileType::SEQ => ",S",
            CbmFileType::USR => ",U",
            CbmFileType::REL => ",R",
            CbmFileType::Unknown => "",
        }
    }
}

impl fmt::Display for CbmFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            CbmFileType::PRG => "prg",
            CbmFileType::SEQ => "seq",
            CbmFileType::USR => "usr",
            CbmFileType::REL => "rel",
            CbmFileType::Unknown => "",
        };
        write!(f, "{}", output)?;
        Ok(())
    }
}

impl From<&str> for CbmFileType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PRG" => CbmFileType::PRG,
            "SEQ" => CbmFileType::SEQ,
            "USR" => CbmFileType::USR,
            "REL" => CbmFileType::REL,
            _ => CbmFileType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbmFileMode {
    Read,
    Write,
    Append,
}

impl CbmFileMode {
    fn _to_suffix(&self) -> &'static str {
        match self {
            CbmFileMode::Read => "",
            CbmFileMode::Write => ",W",
            CbmFileMode::Append => ",A",
        }
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
#[derive(Debug, Clone)]
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

impl CbmFileEntry {
    pub fn max_size(&self) -> Option<u64> {
        match self {
            CbmFileEntry::ValidFile { blocks, .. } => Some((*blocks as u64) * BYTES_PER_BLOCK),
            CbmFileEntry::InvalidFile { partial_blocks, .. } => {
                if let Some(blocks) = partial_blocks {
                    Some((*blocks as u64) * BYTES_PER_BLOCK)
                } else {
                    None
                }
            }
        }
    }
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
#[derive(Debug, Clone)]
pub struct CbmDiskHeader {
    pub drive_number: u8,
    pub name: String,
    pub id: String,
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
#[derive(Debug, Clone)]
pub struct CbmDirListing {
    /// The drive number (0 or 1) where this disk is mounted
    pub header: CbmDiskHeader,

    /// The name of the disk (up to 16 characters)
    pub files: Vec<CbmFileEntry>,

    /// The two-character disk ID
    pub blocks_free: u16,
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
    /// * `Err(Error)` if the listing cannot be parsed
    ///
    /// # Errors
    ///
    /// Returns `Error::Parse` if:
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
    pub fn parse(input: &str) -> Result<Self, Error> {
        trace!("CbmDirListing::parse input.len() {}", input.len());
        trace!("Input:\n{}", input);
        let mut lines = input.lines();

        // Parse header
        let header = Self::parse_header(lines.next().ok_or_else(|| {
            debug!("CbmDirListing::parse Missing header line");
            Error::Parse {
                message: "Missing header line".to_string(),
            }
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

        let blocks_free = blocks_free.ok_or_else(|| {
            debug!("CbmDirListing::parse Missing blocks free line");
            Error::Parse {
                message: "Missing blocks free line".to_string(),
            }
        })?;

        Ok(CbmDirListing {
            header,
            files,
            blocks_free,
        })
    }

    fn parse_header(line: &str) -> Result<CbmDiskHeader, Error> {
        // Example: "   0 ."test/demo  1/85 " 8a 2a"
        let re =
            regex::Regex::new(r#"^\s*(\d+)\s+\."([^"]*)" ([a-zA-Z0-9]{2})"#).map_err(|_| {
                Error::Parse {
                    message: "Invalid header regex".to_string(),
                }
            })?;

        let caps = re.captures(line).ok_or_else(|| Error::Parse {
            message: format!("Invalid header format: {}", line),
        })?;

        Ok(CbmDiskHeader {
            drive_number: caps[1].parse().map_err(|_| Error::Parse {
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

    fn parse_blocks_free(line: &str) -> Result<u16, Error> {
        let re = regex::Regex::new(r"^\s*(\d+)\s+blocks free").map_err(|_| Error::Parse {
            message: "Invalid blocks free regex".to_string(),
        })?;

        let caps = re.captures(line).ok_or_else(|| Error::Parse {
            message: format!("Invalid blocks free format: {}", line),
        })?;

        caps[1].parse().map_err(|_| Error::Parse {
            message: format!("Invalid blocks free number: {}", &caps[1]),
        })
    }

    pub fn num_files(&self) -> usize {
        self.files.len()
    }

    pub fn num_blocks_used_valid(&self) -> u16 {
        self.files.iter()
            .map(|entry| match entry {
                CbmFileEntry::ValidFile { blocks, .. } => *blocks,
                _ => 0,
            })
            .sum()
    }

    pub fn total_blocks(&self) -> u16 {
        self.num_blocks_used_valid() + self.blocks_free
    }
}
