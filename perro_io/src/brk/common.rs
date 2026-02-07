use std::io::{self, Read, Write};

pub const BRK_MAGIC: [u8; 4] = *b"BRK1";

pub const FLAG_COMPRESSED: u32 = 1; // Bit 0: Data is ZSTD compressed
pub const FLAG_ENCRYPTED: u32 = 2; // Bit 1: Data is AES-GCM encrypted

/// Archive header
#[derive(Debug, Clone, Copy)]
pub struct BrkHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub file_count: u32,
    pub index_offset: u64,
}

/// Entry metadata written into the index
#[derive(Debug, Clone)]
pub struct BrkEntryMeta {
    pub offset: u64,
    pub size: u64, // Actual size in the archive (compressed if FLAG_COMPRESSED)
    pub original_size: u64,
    pub flags: u32,
    pub nonce: [u8; 12],
    pub tag: [u8; 16],
}

fn read_exact_array<const N: usize, R: Read>(reader: &mut R) -> io::Result<[u8; N]> {
    let mut buf = [0u8; N];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_u16<R: Read>(reader: &mut R) -> io::Result<u16> {
    Ok(u16::from_le_bytes(read_exact_array::<2, _>(reader)?))
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    Ok(u32::from_le_bytes(read_exact_array::<4, _>(reader)?))
}

fn read_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    Ok(u64::from_le_bytes(read_exact_array::<8, _>(reader)?))
}

pub fn read_header<R: Read>(reader: &mut R) -> io::Result<BrkHeader> {
    let magic = read_exact_array::<4, _>(reader)?;
    if magic != BRK_MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a BRK file"));
    }

    Ok(BrkHeader {
        magic,
        version: read_u32(reader)?,
        file_count: read_u32(reader)?,
        index_offset: read_u64(reader)?,
    })
}

pub fn write_header<W: Write>(writer: &mut W, header: &BrkHeader) -> io::Result<()> {
    writer.write_all(&header.magic)?;
    writer.write_all(&header.version.to_le_bytes())?;
    writer.write_all(&header.file_count.to_le_bytes())?;
    writer.write_all(&header.index_offset.to_le_bytes())?;
    Ok(())
}

pub fn read_index_entry<R: Read>(reader: &mut R) -> io::Result<(String, BrkEntryMeta)> {
    let path_len = read_u16(reader)? as usize;
    let mut path_buf = vec![0u8; path_len];
    reader.read_exact(&mut path_buf)?;
    let path = String::from_utf8(path_buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Index path is not valid UTF-8"))?;

    let offset = read_u64(reader)?;
    let size = read_u64(reader)?;
    let original_size = read_u64(reader)?;
    let flags = read_u32(reader)?;
    let nonce = read_exact_array::<12, _>(reader)?;
    let tag = read_exact_array::<16, _>(reader)?;

    Ok((
        path,
        BrkEntryMeta {
            offset,
            size,
            original_size,
            flags,
            nonce,
            tag,
        },
    ))
}

pub fn write_index_entry<W: Write>(
    writer: &mut W,
    path: &str,
    meta: &BrkEntryMeta,
) -> io::Result<()> {
    let path_bytes = path.as_bytes();
    if path_bytes.len() > u16::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Index path is too long",
        ));
    }

    let path_len = path_bytes.len() as u16;
    writer.write_all(&path_len.to_le_bytes())?;
    writer.write_all(path_bytes)?;
    writer.write_all(&meta.offset.to_le_bytes())?;
    writer.write_all(&meta.size.to_le_bytes())?;
    writer.write_all(&meta.original_size.to_le_bytes())?;
    writer.write_all(&meta.flags.to_le_bytes())?;
    writer.write_all(&meta.nonce)?;
    writer.write_all(&meta.tag)?;
    Ok(())
}
