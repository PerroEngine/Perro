use std::io::{self, Read, Write};

pub const PERRO_ASSETS_MAGIC: [u8; 4] = perro_asset_formats::archive::MAGIC;
pub const PERRO_ASSETS_COMPRESSED_MAGIC: [u8; 4] = perro_asset_formats::archive::COMPRESSED_MAGIC;

pub const FLAG_COMPRESSED: u32 = perro_asset_formats::archive::FLAG_COMPRESSED;

/// Archive header
#[derive(Debug, Clone, Copy)]
pub struct PerroAssetsHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub file_count: u32,
    pub index_offset: u64,
}

/// Entry metadata written into the index
#[derive(Debug, Clone)]
pub struct PerroAssetsEntryMeta {
    pub offset: u64,
    pub size: u64,          // Actual size in archive (compressed if FLAG_COMPRESSED)
    pub original_size: u64, // Original uncompressed size
    pub flags: u32,
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

pub fn read_header<R: Read>(reader: &mut R) -> io::Result<PerroAssetsHeader> {
    let magic = read_exact_array::<4, _>(reader)?;
    if magic != PERRO_ASSETS_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Not a PerroAssets file",
        ));
    }

    let version = read_u32(reader)?;
    if version != perro_asset_formats::archive::VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported PerroAssets version {version}"),
        ));
    }

    Ok(PerroAssetsHeader {
        magic,
        version,
        file_count: read_u32(reader)?,
        index_offset: read_u64(reader)?,
    })
}

pub fn write_header<W: Write>(writer: &mut W, header: &PerroAssetsHeader) -> io::Result<()> {
    writer.write_all(&header.magic)?;
    writer.write_all(&header.version.to_le_bytes())?;
    writer.write_all(&header.file_count.to_le_bytes())?;
    writer.write_all(&header.index_offset.to_le_bytes())?;
    Ok(())
}

pub fn read_index_entry<R: Read>(reader: &mut R) -> io::Result<(String, PerroAssetsEntryMeta)> {
    let path_len = read_u16(reader)? as usize;
    let mut path_buf = vec![0u8; path_len];
    reader.read_exact(&mut path_buf)?;
    let path = String::from_utf8(path_buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 path"))?;

    let offset = read_u64(reader)?;
    let size = read_u64(reader)?;
    let original_size = read_u64(reader)?;
    let flags = read_u32(reader)?;
    if flags & !FLAG_COMPRESSED != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported PerroAssets entry flags {flags:#x}"),
        ));
    }

    Ok((
        path,
        PerroAssetsEntryMeta {
            offset,
            size,
            original_size,
            flags,
        },
    ))
}

pub fn write_index_entry<W: Write>(
    writer: &mut W,
    path: &str,
    meta: &PerroAssetsEntryMeta,
) -> io::Result<()> {
    let path_bytes = path.as_bytes();
    if path_bytes.len() > u16::MAX as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Path too long"));
    }

    let path_len = path_bytes.len() as u16;
    writer.write_all(&path_len.to_le_bytes())?;
    writer.write_all(path_bytes)?;
    writer.write_all(&meta.offset.to_le_bytes())?;
    writer.write_all(&meta.size.to_le_bytes())?;
    writer.write_all(&meta.original_size.to_le_bytes())?;
    writer.write_all(&meta.flags.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PERRO_ASSETS_MAGIC, read_header, read_index_entry};
    use std::io::{Cursor, ErrorKind};

    #[test]
    fn header_rejects_unknown_version() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PERRO_ASSETS_MAGIC);
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&20u64.to_le_bytes());

        let err = read_header(&mut Cursor::new(bytes)).expect_err("unknown version");
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn index_rejects_unknown_flags() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.push(b'x');
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());

        let err = read_index_entry(&mut Cursor::new(bytes)).expect_err("unknown flags");
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }
}
