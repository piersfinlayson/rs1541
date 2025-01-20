/// Convert a PETSCII character to ASCII.
/// Returns the ASCII equivalent if it can be displayed, or '.' otherwise.
pub fn petscii_to_ascii(character: u8) -> char {
    // First handle the special cases
    match character {
        0x0a | 0x0d => '\n',
        0x40 | 0x60 => character as char,
        0xa0 | 0xe0 => ' ', // CBM: Shifted Space
        _ => {
            // Then handle the character ranges
            match character & 0xe0 {
                0x40 | 0x60 => (character ^ 0x20) as char, // 41-7E
                0xc0 => (character ^ 0x80) as char,        // C0-DF
                _ => {
                    // For all other characters, return as-is if printable, '.' if not
                    if character.is_ascii() && (character as char).is_ascii_graphic() {
                        character as char
                    } else {
                        '.'
                    }
                }
            }
        }
    }
}

/// Convert an ASCII character to PETSCII.
/// Returns the PETSCII equivalent of the input character.
pub fn ascii_to_petscii(character: char) -> u8 {
    let c = character as u8;

    if (0x5b..=0x7e).contains(&c) {
        c ^ 0x20
    } else if character.is_ascii_uppercase() {
        c | 0x80
    } else {
        c
    }
}

pub fn petscii_str_to_ascii(input: &[u8]) -> String {
    input.iter().map(|&c| petscii_to_ascii(c)).collect()
}

pub fn ascii_str_to_petscii(input: &str) -> Vec<u8> {
    input.chars().map(ascii_to_petscii).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_petscii_special_chars() {
        assert_eq!(petscii_to_ascii(0x0a), '\n');
        assert_eq!(petscii_to_ascii(0x0d), '\n');
        assert_eq!(petscii_to_ascii(0x40), '@');
        assert_eq!(petscii_to_ascii(0x60), '`');
        assert_eq!(petscii_to_ascii(0xa0), ' ');
        assert_eq!(petscii_to_ascii(0xe0), ' ');
    }

    #[test]
    fn test_ascii_conversion() {
        // Test uppercase letters
        assert_eq!(ascii_to_petscii('A'), 0xc1);
        assert_eq!(ascii_to_petscii('Z'), 0xda);

        // Test special characters
        assert_eq!(ascii_to_petscii('['), 0x7b);
        assert_eq!(ascii_to_petscii(']'), 0x7d);

        // Test unchanged characters
        assert_eq!(ascii_to_petscii('a'), b'A');
        assert_eq!(ascii_to_petscii('1'), b'1');
    }
}
