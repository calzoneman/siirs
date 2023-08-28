use anyhow::{Result, bail};
use byteorder::{ReadBytesExt, BigEndian, LittleEndian};
use flate2::read::ZlibDecoder;
use std::{fs::File, io::{Seek, Read}, collections::HashMap};

pub struct Archive {
    file: File,
    entries: HashMap<u64, Entry>
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EntryType {
    UncompressedFile,
    UncompressedDirEntries,
    CompressedFile,
    CompressedDirEntries,
}

impl TryFrom<u32> for EntryType {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 | 4 => Ok(Self::UncompressedFile),
            1 | 5 => Ok(Self::UncompressedDirEntries),
            2 | 6 => Ok(Self::CompressedFile),
            3 | 7 => Ok(Self::CompressedDirEntries),
            x => bail!("unknown entry type id {x}")
        }
    }
}

#[derive(Debug)]
pub struct Entry {
    pub hash: u64,
    pub offset: u32,
    _unknown: u32,
    pub entry_type: EntryType,
    pub crc32: u32,
    pub size: u32,
    pub zsize: u32
}

impl Entry {
    fn try_from_reader<R: Read>(r: &mut R) -> Result<Self> {
        Ok(Self {
            hash: r.read_u64::<LittleEndian>()?,
            offset: r.read_u32::<LittleEndian>()?,
            _unknown: r.read_u32::<LittleEndian>()?,
            entry_type: r.read_u32::<LittleEndian>().map(EntryType::try_from)??,
            crc32: r.read_u32::<LittleEndian>()?,
            size: r.read_u32::<LittleEndian>()?,
            zsize: r.read_u32::<LittleEndian>()?,
        })
    }
}

impl Archive {
    const SCS_SIGNATURE: u32 = u32::from_be_bytes(*b"SCS#");
    const CITYHASH_MARKER: u32 = u32::from_be_bytes(*b"CITY");

    pub fn load_from_file(mut file: File) -> Result<Self> {
        let signature = file.read_u32::<BigEndian>()?;
        if signature != Self::SCS_SIGNATURE {
            bail!("signature does not match: {:X}", signature);
        }

        let _unknown_maybe_version = file.read_u32::<LittleEndian>()?;
        let cityhash = file.read_u32::<BigEndian>()?;
        if cityhash != Self::CITYHASH_MARKER {
            bail!("expected CITY marker");
        }
        let entry_count = file.read_u32::<LittleEndian>()? as usize;
        let entry_offset = file.read_u32::<LittleEndian>()? as u64;

        file.seek(std::io::SeekFrom::Start(entry_offset))?;
        let mut entries: HashMap<u64, Entry> = HashMap::with_capacity(entry_count);
        for _ in 0..entry_count {
            let e = Entry::try_from_reader(&mut file)?;
            entries.insert(e.hash, e);
        }

        Ok(Self { file, entries })
    }

    pub fn load_from_path(path: &str) -> Result<Self> {
        let file = File::options()
            .create(false)
            .write(false)
            .read(true)
            .open(path)?;

        Self::load_from_file(file)
    }

    pub fn describe_entry(&self, hash: u64) -> Option<&Entry> {
        self.entries.get(&hash)
    }

    pub fn open_entry(&mut self, hash: u64) -> Result<EntryReader> {
        if let Some(entry) = self.describe_entry(hash) {
            let offset = entry.offset;
            let entry_type = entry.entry_type;
            let size = entry.size;
            self.file.seek(std::io::SeekFrom::Start(offset as u64))?;

            match entry_type {
                EntryType::UncompressedFile => {
                    Ok(EntryReader::UncompressedReader {
                        reader: UncompressedReader { 
                            file: &mut self.file,
                            length: size as usize,
                            i: 0
                        }
                    })
                }
                EntryType::CompressedFile => {
                    Ok(EntryReader::CompressedReader {
                        decoder: ZlibDecoder::new(&mut self.file)
                    })
                }
                t => bail!("unsupported entry type {t:?}")
            }
        } else {
            bail!("no such entry with hash {hash:X}")
        }
    }
}

pub struct UncompressedReader<'a> {
    file: &'a mut File,
    length: usize,
    i: usize,
}

impl UncompressedReader<'_> {
    fn remaining(&self) -> usize {
        self.length - self.i
    }
}

impl<'a> Read for UncompressedReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = std::cmp::min(buf.len(), self.remaining());
        let res = self.file.read(&mut buf[..n])?;
        self.i += res;
        Ok(res)
    }
}

pub enum EntryReader<'a> {
    UncompressedReader {
        reader: UncompressedReader<'a>
    },
    CompressedReader {
        // XXX: assumption here is that the deflate stream will EOF on its own,
        // so the underlying File reader is not limited by zsize.  This works
        // fine for the entries I care about.
        decoder: ZlibDecoder<&'a mut File>
    }
}

impl<'a> Read for EntryReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::UncompressedReader { reader } => {
                reader.read(buf)
            },
            Self::CompressedReader { decoder } => {
                decoder.read(buf)
            }
        }
    }
}