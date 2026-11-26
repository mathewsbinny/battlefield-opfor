//! Xbox 360 big-endian -> PC little-endian inspection & conversion for EA APT `.const` and `.geo`.

/// Summary metadata of an EA APT `.const` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstInfo {
    pub big_endian: bool,
    pub movie_offset: u32,
    pub count: u32,
    pub entries_offset: u32,
}

const CONST_MAGIC: &[u8; 20] = b"Apt constant file\x1A\0\0";

/// Inspect an EA APT `.const` file and detect endianness from `movie_offset` vs `apt_len`.
pub fn inspect_const(raw: &[u8], apt_len: usize) -> Result<ConstInfo, String> {
    if raw.len() < 32 || &raw[0..20] != CONST_MAGIC {
        return Err("invalid Apt constant file header".to_string());
    }
    let off_le = u32::from_le_bytes([raw[20], raw[21], raw[22], raw[23]]);
    let off_be = u32::from_be_bytes([raw[20], raw[21], raw[22], raw[23]]);
    let big_endian = (off_be as usize) < apt_len && (off_le as usize) >= apt_len;
    let r32 = |at: usize| {
        let b = [raw[at], raw[at + 1], raw[at + 2], raw[at + 3]];
        if big_endian {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        }
    };
    Ok(ConstInfo {
        big_endian,
        movie_offset: r32(20),
        count: r32(24),
        entries_offset: r32(28),
    })
}

/// Convert a big-endian Xbox 360 `.const` file to little-endian PC `.const`.
pub fn const_be_to_le(raw: &[u8], apt_len: usize) -> Result<Vec<u8>, String> {
    let info = inspect_const(raw, apt_len)?;
    if !info.big_endian {
        return Ok(raw.to_vec());
    }
    let mut out = raw.to_vec();
    out[20..24].copy_from_slice(&info.movie_offset.to_le_bytes());
    out[24..28].copy_from_slice(&info.count.to_le_bytes());
    out[28..32].copy_from_slice(&info.entries_offset.to_le_bytes());

    let table = info.entries_offset as usize;
    let end = table
        .checked_add((info.count as usize) * 8)
        .ok_or("const table overflow")?;
    if end > out.len() {
        return Err("const table runs past EOF".to_string());
    }
    for i in 0..info.count as usize {
        let at = table + i * 8;
        let typ = u32::from_be_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]);
        let val = u32::from_be_bytes([raw[at + 4], raw[at + 5], raw[at + 6], raw[at + 7]]);
        out[at..at + 4].copy_from_slice(&typ.to_le_bytes());
        out[at + 4..at + 8].copy_from_slice(&val.to_le_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_be_const_header_and_entries() {
        let mut raw = Vec::new();
        raw.extend_from_slice(CONST_MAGIC);
        raw.extend_from_slice(&0x100u32.to_be_bytes()); // movie_offset
        raw.extend_from_slice(&1u32.to_be_bytes()); // count
        raw.extend_from_slice(&32u32.to_be_bytes()); // entries_offset
        raw.extend_from_slice(&1u32.to_be_bytes()); // type = string
        raw.extend_from_slice(&40u32.to_be_bytes()); // string offset
        raw.extend_from_slice(b"hello\0");

        let info = inspect_const(&raw, 4096).unwrap();
        assert!(info.big_endian);
        assert_eq!(info.movie_offset, 0x100);

        let pc = const_be_to_le(&raw, 4096).unwrap();
        let info_pc = inspect_const(&pc, 4096).unwrap();
        assert!(!info_pc.big_endian);
        assert_eq!(info_pc.movie_offset, 0x100);
        assert_eq!(u32::from_le_bytes([pc[36], pc[37], pc[38], pc[39]]), 40);
    }
}
