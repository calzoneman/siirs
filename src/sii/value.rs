use std::{collections::HashMap, fmt::Debug, io::Read};

use anyhow::{anyhow, bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};

pub trait ReadFrom
where
    Self: Sized,
{
    fn read_from<R: Read>(reader: &mut R) -> Result<Self>;
}

macro_rules! read_from {
    ($t:ty, $r:ident, $($expr:tt)*) => {
        impl ReadFrom for $t {
            fn read_from<R: Read>($r: &mut R) -> Result<Self> {
                $($expr)*
            }
        }
    };
}

read_from!(u16, reader, Ok(reader.read_u16::<LittleEndian>()?));
read_from!(i32, reader, Ok(reader.read_i32::<LittleEndian>()?));
read_from!(u32, reader, Ok(reader.read_u32::<LittleEndian>()?));
read_from!(u64, reader, Ok(reader.read_u64::<LittleEndian>()?));
read_from!(i64, reader, Ok(reader.read_i64::<LittleEndian>()?));
read_from!(f32, reader, Ok(reader.read_f32::<LittleEndian>()?));

macro_rules! def_vec {
    ($name:ident, $($t:ty),+) => {
        pub type $name = ($($t),+);

        impl ReadFrom for $name {
            fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
                Ok((
                    $(<$t>::read_from(reader)?),+
                ))
            }
        }
    };
}

def_vec!(Vec2s, f32, f32);
def_vec!(Vec3s, f32, f32, f32);
def_vec!(Vec4s, f32, f32, f32, f32);
def_vec!(Vec8s, f32, f32, f32, f32, f32, f32, f32, f32);
def_vec!(Vec3i, i32, i32, i32);

#[derive(Debug, Clone)]
pub struct OrdinalStringTable(HashMap<u32, String>);

impl OrdinalStringTable {
    pub fn get(&self, ordinal: u32) -> Option<&String> {
        self.0.get(&ordinal)
    }
}

impl ReadFrom for OrdinalStringTable {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u32::<LittleEndian>()?;
        let mut out = HashMap::with_capacity(len as usize);

        for _ in 0..len {
            let ordinal = reader.read_u32::<LittleEndian>()?;
            let string = String::read_from(reader)?;
            out.insert(ordinal, string);
        }

        Ok(OrdinalStringTable(out))
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ID {
    Nameless(u64),
    Named(Vec<u64>),
}

impl ID {
    pub fn string_part(&self, index: isize) -> Option<String> {
        match self {
            Self::Named(parts) => {
                let idx = if index < 0 {
                    (parts.len() as isize + index) as usize
                } else {
                    index as usize
                };
                parts.get(idx).map(|p| EncodedString(*p).to_string())
            }
            Self::Nameless(_) => None,
        }
    }
}

impl ToString for ID {
    fn to_string(&self) -> String {
        match self {
            Self::Nameless(id) => {
                let bytes = id.to_le_bytes();
                format!(
                    "_nameless.{:x}{:02x}.{:x}{:02x}.{:x}{:02x}.{:x}{:02x}",
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
                )
            }
            Self::Named(parts) => {
                parts
                    .iter()
                    .map(|p| EncodedString(*p).to_string())
                    .collect::<Vec<String>>() // TODO: do we really need this?
                    .join(".")
            }
        }
    }
}

impl Debug for ID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nameless(_) => f.debug_tuple("Nameless").field(&self.to_string()).finish(),
            Self::Named(_) => f.debug_tuple("Named").field(&self.to_string()).finish(),
        }
    }
}

impl ReadFrom for ID {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u8()?;

        if len == 0xFF {
            Ok(ID::Nameless(u64::read_from(reader)?))
        } else {
            let mut parts = Vec::with_capacity(len as usize);
            for _ in 0..len {
                parts.push(u64::read_from(reader)?);
            }

            Ok(ID::Named(parts))
        }
    }
}

#[derive(Clone, Copy)]
pub struct EncodedString(u64);

impl EncodedString {
    const CHARTABLE: [u8; 37] = *b"0123456789abcdefghijklmnopqrstuvwxyz_";
}

read_from!(
    EncodedString,
    reader,
    Ok(EncodedString(u64::read_from(reader)?))
);

impl ToString for EncodedString {
    fn to_string(&self) -> String {
        let mut res = String::new();
        let mut s = self.0;

        while s > 0 {
            let index = (s % 38 - 1) as usize;
            res.push(Self::CHARTABLE[index].into());
            s /= 38;
        }

        res
    }
}

impl Debug for EncodedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EncodedString")
            .field(&self.to_string())
            .finish()
    }
}

impl<T> ReadFrom for Vec<T>
where
    T: ReadFrom,
{
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u32::<LittleEndian>()?;

        let mut vals = Vec::with_capacity(len as usize);
        for _ in 0..len {
            vals.push(T::read_from(reader)?);
        }

        Ok(vals)
    }
}

impl ReadFrom for String {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let len = u32::read_from(reader)?;
        let mut out = Vec::new();
        out.resize(len as usize, 0u8);
        reader.read_exact(&mut out)?;

        String::from_utf8(out).map_err(|e| anyhow!("invalid string: {e}"))
    }
}

impl ReadFrom for bool {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        if reader.read_u8()? == 0 {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

/// See https://github.com/TheLazyTomcat/SII_Decrypt/blob/master/Documents/Binary%20SII%20-%20Types.txt
#[derive(Debug)]
pub enum Value {
    String(String),
    StringArray(Vec<String>),
    EncodedString(EncodedString),
    EncodedStringArray(Vec<EncodedString>),
    Single(f32),
    SingleArray(Vec<f32>),
    Vec2s(Vec2s),
    Vec3s(Vec3s),
    Vec3sArray(Vec<Vec3s>),
    Vec3i(Vec3i),
    Vec3iArray(Vec<Vec3i>),
    Vec4s(Vec4s),
    Vec4sArray(Vec<Vec4s>),
    Vec8s(Vec8s),
    Vec8sArray(Vec<Vec8s>),
    Int32(i32),
    Int32Array(Vec<i32>),
    UInt32(u32),
    UInt32Array(Vec<u32>),
    UInt16(u16),
    UInt16Array(Vec<u16>),
    Int64(i64),
    Int64Array(Vec<i64>),
    UInt64(u64),
    UInt64Array(Vec<u64>),
    ByteBool(bool),
    ByteBoolArray(Vec<bool>),
    OrdinalString(String),
    ID(ID),
    IDArray(Vec<ID>),
}

impl Value {
    pub fn read_from<R: Read>(
        reader: &mut R,
        value_type: u32,
        ordinal_table: Option<&OrdinalStringTable>,
    ) -> Result<Self> {
        match value_type {
            0x01 => String::read_from(reader).map(Self::String),
            0x02 => Vec::<String>::read_from(reader).map(Self::StringArray),
            0x03 => EncodedString::read_from(reader).map(Self::EncodedString),
            0x04 => Vec::<EncodedString>::read_from(reader).map(Self::EncodedStringArray),
            0x05 => f32::read_from(reader).map(Self::Single),
            0x06 => Vec::<f32>::read_from(reader).map(Self::SingleArray),
            0x07 => Vec2s::read_from(reader).map(Self::Vec2s),
            0x09 => Vec3s::read_from(reader).map(Self::Vec3s),
            0x0A => Vec::<Vec3s>::read_from(reader).map(Self::Vec3sArray),
            0x11 => Vec3i::read_from(reader).map(Self::Vec3i),
            0x12 => Vec::<Vec3i>::read_from(reader).map(Self::Vec3iArray),
            0x17 => Vec4s::read_from(reader).map(Self::Vec4s),
            0x18 => Vec::<Vec4s>::read_from(reader).map(Self::Vec4sArray),
            0x19 => Vec8s::read_from(reader).map(Self::Vec8s),
            0x1A => Vec::<Vec8s>::read_from(reader).map(Self::Vec8sArray),
            0x25 => i32::read_from(reader).map(Self::Int32),
            0x26 => Vec::<i32>::read_from(reader).map(Self::Int32Array),
            0x27 => u32::read_from(reader).map(Self::UInt32),
            0x28 => Vec::<u32>::read_from(reader).map(Self::UInt32Array),
            0x2B => u16::read_from(reader).map(Self::UInt16),
            0x2C => Vec::<u16>::read_from(reader).map(Self::UInt16Array),
            0x2F => u32::read_from(reader).map(Self::UInt32),
            0x31 => i64::read_from(reader).map(Self::Int64),
            0x32 => Vec::<i64>::read_from(reader).map(Self::Int64Array),
            0x33 => u64::read_from(reader).map(Self::UInt64),
            0x34 => Vec::<u64>::read_from(reader).map(Self::UInt64Array),
            0x35 => bool::read_from(reader).map(Self::ByteBool),
            0x36 => Vec::<bool>::read_from(reader).map(Self::ByteBoolArray),
            0x37 => Self::read_ordinal_string(reader, ordinal_table),
            0x39 => ID::read_from(reader).map(Self::ID),
            0x3A => Vec::<ID>::read_from(reader).map(Self::IDArray),
            0x3B => ID::read_from(reader).map(Self::ID),
            0x3C => Vec::<ID>::read_from(reader).map(Self::IDArray),
            0x3D => ID::read_from(reader).map(Self::ID),
            _ => Err(anyhow!("unknown value type {0:X}", value_type)),
        }
    }

    fn read_ordinal_string<R: Read>(
        reader: &mut R,
        table: Option<&OrdinalStringTable>,
    ) -> Result<Value> {
        let ordinal = u32::read_from(reader)?;

        match table {
            Some(t) => {
                let s = t
                    .get(ordinal)
                    .ok_or_else(|| anyhow!("missing ordinal table entry for {ordinal}"))?;

                Ok(Value::OrdinalString(s.clone()))
            }
            None => {
                bail!("missing ordinal table")
            }
        }
    }
}
