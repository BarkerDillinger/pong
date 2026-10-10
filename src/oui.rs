//! Offline IEEE MA-L OUI registry lookup (organization, not device model).
use std::{collections::HashMap, env, fs, path::PathBuf};

pub fn parse_mac(mac: &str) -> Option<[u8; 6]> {
    let mut parts = mac.split(':');
    let mut bytes = [0u8; 6];
    for b in &mut bytes {
        let p = parts.next()?;
        if p.len() != 2 {
            return None;
        }
        *b = u8::from_str_radix(p, 16).ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(bytes)
}

// Enough CSV decoding for the registry's Registry,Assignment,Organization Name columns.
// Organization Name may be quoted and contain commas or doubled quotes.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        if c == '"' {
            if quoted && chars.peek() == Some(&'"') {
                chars.next();
                field.push('"');
            } else {
                quoted = !quoted;
            }
        } else if c == ',' && !quoted {
            fields.push(std::mem::take(&mut field));
            if fields.len() == 3 {
                break;
            }
        } else {
            field.push(c);
        }
    }
    if fields.len() < 3 {
        fields.push(field);
    }
    fields
}

#[derive(Default)]
pub struct VendorDb {
    entries: HashMap<[u8; 3], String>,
}
impl VendorDb {
    fn from_csv(csv: &str) -> Self {
        let mut result = Self::default();
        for line in csv.lines() {
            let cols = csv_fields(line);
            if cols.len() < 3 || cols[0].trim() != "MA-L" {
                continue;
            }
            let assignment = cols[1].trim();
            if assignment.len() != 6 {
                continue;
            }
            if let Ok(n) = u32::from_str_radix(assignment, 16) {
                result.entries.insert(
                    [(n >> 16) as u8, (n >> 8) as u8, n as u8],
                    cols[2].trim().to_string(),
                );
            }
        }
        result
    }
    pub fn load() -> Self {
        let path = env::var_os("PONG_OUI_FILE").map(PathBuf::from).or_else(|| {
            env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share/pong/oui.csv"))
        });
        path.and_then(|p| fs::read_to_string(p).ok())
            .map(|contents| Self::from_csv(&contents))
            .unwrap_or_default()
    }
    pub fn lookup(&self, mac: &str) -> String {
        let Some(bytes) = parse_mac(mac) else {
            return "Unknown".into();
        };
        if bytes[0] & 1 != 0 {
            return "Multicast/group".into();
        }
        if bytes[0] & 2 != 0 {
            return "Locally administered".into();
        }
        self.entries
            .get(&[bytes[0], bytes[1], bytes[2]])
            .cloned()
            .unwrap_or_else(|| "Unknown".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_registry_and_quoted_names() {
        let db = VendorDb::from_csv(
            "Registry,Assignment,Organization Name,Organization Address\nMA-L,7C0A3F,\"Example, Incorporated\",Somewhere\nMA-L,9C6B00,Another Vendor,Somewhere\n",
        );
        assert_eq!(db.lookup("7c:0a:3f:75:30:dc"), "Example, Incorporated");
        assert_eq!(db.lookup("9c:6b:00:97:bd:59"), "Another Vendor");
    }
    #[test]
    fn locally_administered_and_bad_mac() {
        let db = VendorDb::default();
        assert_eq!(db.lookup("02:11:22:33:44:55"), "Locally administered");
        assert_eq!(db.lookup("zz:11:22:33:44:55"), "Unknown");
        assert!(parse_mac("01:02:03:04:05:06:07").is_none());
    }
}
