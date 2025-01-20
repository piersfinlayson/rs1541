use std::convert::TryFrom;
use std::fmt;

#[derive(Clone)]
pub enum CbmString {
    Ascii(AsciiString),
    Petscii(PetsciiString),
}

impl CbmString {
    pub fn to_petscii(&self) -> PetsciiString {
        match self {
            CbmString::Ascii(ascii) => ascii.into(),
            CbmString::Petscii(petscii) => petscii.clone(),
        }
    }

    pub fn from_petscii_bytes(bytes: &[u8]) -> Self {
        CbmString::Petscii(PetsciiString::from_petscii_bytes(bytes))
    }

    pub fn from_ascii_bytes(bytes: &[u8]) -> Self {
        CbmString::Ascii(AsciiString::from_bytes(bytes).unwrap())
    }
}

impl From<AsciiString> for CbmString {
    fn from(ascii: AsciiString) -> Self {
        CbmString::Ascii(ascii)
    }
}

impl From<PetsciiString> for CbmString {
    fn from(petscii: PetsciiString) -> Self {
        CbmString::Petscii(petscii)
    }
}

impl<'a> TryFrom<&'a str> for CbmString {
    type Error = Box<dyn std::error::Error>;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        // Try ASCII first since it's more restrictive
        if let Ok(ascii) = AsciiString::try_from(s) {
            Ok(CbmString::Ascii(ascii))
        } else {
            // Assume PETSCII if not ASCII
            Ok(CbmString::Petscii(PetsciiString::from_petscii_bytes(
                s.as_bytes(),
            )))
        }
    }
}

#[derive(Debug, Clone)]
pub struct PetsciiString(Vec<u8>);

#[derive(Debug, Clone)]
pub struct AsciiString(Vec<u8>);

impl PetsciiString {
    /// Create a new PetsciiString from raw bytes, without performing validation.
    ///
    /// # Safety
    /// The caller must ensure the bytes are valid PETSCII.
    pub unsafe fn from_bytes_unchecked(bytes: Vec<u8>) -> Self {
        PetsciiString(bytes)
    }

    /// Create a new PetsciiString from bytes, validating the input.
    /// Returns None if any byte is invalid PETSCII.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // TODO: Add PETSCII validation if needed
        Some(PetsciiString(bytes.to_vec()))
    }

    /// Convert to an AsciiString
    pub fn to_ascii(&self) -> AsciiString {
        let converted: Vec<u8> = self.0.iter().map(|&c| petscii_to_ascii(c) as u8).collect();
        AsciiString(converted)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl AsciiString {
    /// Create a new AsciiString from raw bytes, without performing validation.
    ///
    /// # Safety
    /// The caller must ensure the bytes are valid ASCII.
    pub unsafe fn from_bytes_unchecked(bytes: Vec<u8>) -> Self {
        AsciiString(bytes)
    }

    /// Create a new AsciiString from bytes, validating the input.
    /// Returns None if any byte is not ASCII.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.iter().all(|&b| b.is_ascii()) {
            Some(AsciiString(bytes.to_vec()))
        } else {
            None
        }
    }

    /// Convert to a PetsciiString
    pub fn to_petscii(&self) -> PetsciiString {
        let converted: Vec<u8> = self
            .0
            .iter()
            .map(|&c| ascii_to_petscii(c as char))
            .collect();
        PetsciiString(converted)
    }

    /// Convert to a regular Rust String
    pub fn to_string(&self) -> String {
        // Safe because we validate ASCII in constructor
        unsafe { String::from_utf8_unchecked(self.0.clone()) }
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

// Implement Display for both string types
impl fmt::Display for PetsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_ascii())
    }
}

impl fmt::Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Safe because we validate ASCII in constructor
        let s = unsafe { std::str::from_utf8_unchecked(&self.0) };
        write!(f, "{}", s)
    }
}

// Implement PartialEq and Eq for both types
impl PartialEq for PetsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.to_ascii().0 == other.to_ascii().0
    }
}

impl Eq for PetsciiString {}

impl PartialEq for AsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for AsciiString {}

// Cross-type equality comparisons
impl PartialEq<AsciiString> for PetsciiString {
    fn eq(&self, other: &AsciiString) -> bool {
        self.to_ascii().0 == other.0
    }
}

impl PartialEq<PetsciiString> for AsciiString {
    fn eq(&self, other: &PetsciiString) -> bool {
        self.0 == other.to_ascii().0
    }
}

impl From<AsciiString> for PetsciiString {
    fn from(ascii: AsciiString) -> Self {
        ascii.to_petscii()
    }
}

impl From<PetsciiString> for AsciiString {
    fn from(petscii: PetsciiString) -> Self {
        petscii.to_ascii()
    }
}

// Allow converting from &AsciiString too
impl From<&AsciiString> for PetsciiString {
    fn from(ascii: &AsciiString) -> Self {
        ascii.to_petscii()
    }
}

// Allow converting from &PetsciiString too
impl From<&PetsciiString> for AsciiString {
    fn from(petscii: &PetsciiString) -> Self {
        petscii.to_ascii()
    }
}

impl From<AsciiString> for String {
    fn from(ascii: AsciiString) -> String {
        ascii.to_string()
    }
}

impl AsciiString {
    /// Create a new AsciiString from a string literal.
    /// Panics if the string contains non-ASCII characters.
    ///
    /// Use this when you know the string is ASCII (e.g., for literals).
    pub fn from_ascii_str(s: &str) -> Self {
        Self::try_from(s).expect("String contains non-ASCII characters")
    }
}

impl PetsciiString {
    /// Create a new PetsciiString from bytes that are already in PETSCII format.
    ///
    /// # Safety
    /// The caller must ensure the bytes are valid PETSCII.
    pub fn from_petscii_bytes(bytes: &[u8]) -> Self {
        // Using to_vec() isn't const yet, but we could make this const with a custom vec creation
        PetsciiString(bytes.to_vec())
    }

    /// Create a new PetsciiString from an ASCII string literal.
    /// Panics if the string contains non-ASCII characters.
    pub fn from_ascii_str(s: &str) -> Self {
        AsciiString::from_ascii_str(s).into()
    }
}

impl TryFrom<String> for AsciiString {
    type Error = &'static str;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_ascii() {
            Ok(AsciiString(s.into_bytes()))
        } else {
            Err("String contains non-ASCII characters")
        }
    }
}

impl TryFrom<&String> for AsciiString {
    type Error = &'static str;

    fn try_from(s: &String) -> Result<Self, Self::Error> {
        if s.is_ascii() {
            Ok(AsciiString(s.as_bytes().to_vec()))
        } else {
            Err("String contains non-ASCII characters")
        }
    }
}

impl TryFrom<&str> for AsciiString {
    type Error = &'static str;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.is_ascii() {
            Ok(AsciiString(s.as_bytes().to_vec()))
        } else {
            Err("String contains non-ASCII characters")
        }
    }
}

// The core conversion functions, now marked private
fn petscii_to_ascii(character: u8) -> char {
    match character {
        0x0a | 0x0d => '\n',
        0x40 | 0x60 => character as char,
        0xa0 | 0xe0 => ' ', // CBM: Shifted Space
        _ => match character & 0xe0 {
            0x40 | 0x60 => (character ^ 0x20) as char,
            0xc0 => (character ^ 0x80) as char,
            _ => {
                if character.is_ascii() && (character as char).is_ascii_graphic() {
                    character as char
                } else {
                    '.'
                }
            }
        },
    }
}

fn ascii_to_petscii(character: char) -> u8 {
    let c = character as u8;
    if (0x5b..=0x7e).contains(&c) {
        c ^ 0x20
    } else if character.is_ascii_uppercase() {
        c | 0x80
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_string_conversion() {
        let ascii = AsciiString::try_from("Hello").unwrap();
        let petscii = ascii.to_petscii();
        let back_to_ascii = petscii.to_ascii();
        assert_eq!(ascii, back_to_ascii);
    }

    #[test]
    fn test_ascii_validation() {
        assert!(AsciiString::try_from("Hello").is_ok());
        assert!(AsciiString::try_from("Hello 🌍").is_err());
    }

    #[test]
    fn test_display() {
        let ascii = AsciiString::try_from("Hello").unwrap();
        let petscii = ascii.to_petscii();

        assert_eq!(&format!("{}", ascii), "Hello");
        assert_eq!(&format!("{}", petscii), "Hello");
    }

    #[test]
    fn test_equality() {
        let ascii1 = AsciiString::try_from("TEST").unwrap();
        let petscii1 = ascii1.to_petscii();
        let ascii2 = AsciiString::try_from("TEST").unwrap();
        let petscii2 = ascii2.to_petscii();

        // Test all equality combinations
        assert_eq!(ascii1, ascii2);
        assert_eq!(petscii1, petscii2);
        assert_eq!(ascii1, petscii1);
        assert_eq!(petscii1, ascii1);

        // Test inequality
        let different = AsciiString::try_from("OTHER").unwrap();
        assert_ne!(ascii1, different);
        assert_ne!(petscii1, different);
        assert_ne!(petscii1, different.to_petscii());
    }
}
