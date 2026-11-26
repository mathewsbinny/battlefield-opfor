//! EA `BIGF` archive reader & writer (used by Frostbite `SwfMovie` `.res` files).

/// One entry inside a `BIGF` archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigfEntry {
    pub name: String,
    pub data: Vec<u8>,
}

/// Parse an EA `BIGF` archive (`"BIGF"`, u32 LE total size, u32 BE count, u32 BE header size).
pub fn parse(raw: &[u8]) -> Result<Vec<BigfEntry>, String> {
    if raw.len() < 16 || &raw[0..4] != b"BIGF" {
        return Err("not a BIGF archive".to_string());
    }
    let count = u32::from_be_bytes([raw[8], raw[9], raw[10], raw[11]]) as usize;
    let mut pos = 16usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if pos + 8 > raw.len() {
            return Err("truncated BIGF directory".to_string());
        }
        let off = u32::from_be_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]]) as usize;
        let len =
            u32::from_be_bytes([raw[pos + 4], raw[pos + 5], raw[pos + 6], raw[pos + 7]]) as usize;
        pos += 8;
        let Some(nul) = raw[pos..].iter().position(|&b| b == 0) else {
            return Err("unterminated BIGF entry name".to_string());
        };
        let name = String::from_utf8_lossy(&raw[pos..pos + nul]).into_owned();
        pos += nul + 1;
        if off + len > raw.len() {
            return Err(format!("BIGF entry {name} runs past EOF"));
        }
        out.push(BigfEntry {
            name,
            data: raw[off..off + len].to_vec(),
        });
    }
    Ok(out)
}

/// Pack entries back into a valid 16-byte-aligned EA `BIGF` archive.
#[must_use]
pub fn write(entries: &[BigfEntry]) -> Vec<u8> {
    let mut dir_len = 16usize;
    for e in entries {
        dir_len += 8 + e.name.len() + 1;
    }
    let hdr_size = align16(dir_len);
    let mut offsets = Vec::with_capacity(entries.len());
    let mut cur = hdr_size;
    for e in entries {
        offsets.push(cur);
        cur = align16(cur + e.data.len());
    }
    let total_size = cur;
    let mut out = vec![0u8; total_size];
    out[0..4].copy_from_slice(b"BIGF");
    out[4..8].copy_from_slice(&(total_size as u32).to_le_bytes());
    out[8..12].copy_from_slice(&(entries.len() as u32).to_be_bytes());
    out[12..16].copy_from_slice(&(hdr_size as u32).to_be_bytes());

    let mut pos = 16usize;
    for (e, &off) in entries.iter().zip(&offsets) {
        out[pos..pos + 4].copy_from_slice(&(off as u32).to_be_bytes());
        out[pos + 4..pos + 8].copy_from_slice(&(e.data.len() as u32).to_be_bytes());
        pos += 8;
        out[pos..pos + e.name.len()].copy_from_slice(e.name.as_bytes());
        pos += e.name.len();
        out[pos] = 0;
        pos += 1;
        out[off..off + e.data.len()].copy_from_slice(&e.data);
    }
    out
}

const fn align16(v: usize) -> usize {
    (v + 15) & !15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigf_roundtrip() {
        let entries = vec![
            BigfEntry {
                name: "test.apt".to_string(),
                data: b"Apt Data:test".to_vec(),
            },
            BigfEntry {
                name: "test.const".to_string(),
                data: vec![1, 2, 3, 4, 5],
            },
        ];
        let packed = write(&entries);
        let parsed = parse(&packed).unwrap();
        assert_eq!(parsed, entries);
    }
}
