/// Hex string ↔ bytes conversion and hex dump parsing

/// Parse a hex string (with optional whitespace) into bytes.
/// Returns None on invalid input (odd length or non-hex chars).
pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let s: String = hex.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if s.len() % 2 != 0 || s.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks(2) {
        let hi = hex_val(chunk[0])?;
        let lo = hex_val(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Convert bytes to a hex string with spaces between bytes.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse a hex dump of the format commonly output by hex editors.
/// Lines look like: `00000000h: 48 65 6C 6C 6F 20 57 6F 72 6C 64 00  ...`
/// We extract the hex bytes after the colon and before the ASCII section
/// (separated by two spaces or more).
pub fn parse_hex_dump(dump: &str) -> String {
    let mut result = String::new();
    for line in dump.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Try to detect if this is a hex dump with address prefix or just hex bytes
        let hex_source = if let Some(colon_pos) = trimmed.find(':') {
            // Check for address prefix before colon
            let before = trimmed[..colon_pos].trim_end_matches('h').trim_end_matches('H');
            if !before.is_empty()
                && before.chars().all(|c| c.is_ascii_hexdigit())
                && before.len() <= 8
            {
                // Hex dump format: extract after colon, before ASCII section
                let after = &trimmed[colon_pos + 1..];
                if let Some(ds) = after.find("  ") {
                    &after[..ds]
                } else {
                    after
                }
            } else {
                // Colon present but not a hex dump address - treat whole line as hex source
                trimmed
            }
        } else {
            // No colon - plain hex bytes
            trimmed
        };
        for token in hex_source.split_whitespace() {
            if token.len() == 2 && token.chars().all(|c| c.is_ascii_hexdigit()) {
                result.push_str(token);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("48656C6C6F"), Some(vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]));
        assert_eq!(hex_to_bytes("48 65 6C 6C 6F"), Some(vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]));
        assert_eq!(hex_to_bytes(""), None);
        assert_eq!(hex_to_bytes("ABC"), None);
        assert_eq!(hex_to_bytes("XX"), None);
    }

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(&[0x48, 0x65, 0x6C]), "48 65 6C");
    }

    #[test]
    fn test_parse_hex_dump() {
        let dump = "00000000h: 48 65 6C 6C 6F 20 57 6F 72 6C 64 00              Hello World.";
        assert_eq!(parse_hex_dump(dump), "48656C6C6F20576F726C6400");

        // With colon only (no 'h')
        let dump2 = "00000000: 48 65 6C 6C 6F";
        assert_eq!(parse_hex_dump(dump2), "48656C6C6F");
    }
}
