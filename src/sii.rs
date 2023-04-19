use anyhow::{anyhow, bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{collections::HashMap, io::Read};

const SII_SIGNATURE: u32 = 0x49495342;

pub type OrdinalStringTable = HashMap<u32, String>;

#[derive(Clone, Debug)]
pub struct StructFieldDef {
    pub value_type: u32,
    pub name: String,
    pub ordinal_table: Option<OrdinalStringTable>,
}

#[derive(Clone, Debug)]
pub struct StructBlock {
    pub id: u32,
    pub name: String,
    pub fields: Vec<StructFieldDef>,
}

#[derive(Debug)]
pub struct DataBlock {
    pub id: ID,
    pub fields: HashMap<String, Value>,
}

pub enum Block {
    Struct(StructBlock),
    Data(DataBlock),
}

pub struct Reader<R: Read> {
    reader: R,
    struct_defs: HashMap<u32, StructBlock>,
}

impl<R: Read> Reader<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let signature = reader.read_u32::<LittleEndian>()?;
        if signature != SII_SIGNATURE {
            bail!("invalid signature: {signature:X}")
        }

        let version = reader.read_u32::<LittleEndian>()?;
        if version != 3 {
            bail!("unsupported version: {version}")
        }

        Ok(Self {
            reader,
            struct_defs: HashMap::new(),
        })
    }

    pub fn next_block(&mut self) -> Result<Option<Block>> {
        let block_type = self.reader.read_u32::<LittleEndian>()?;

        if block_type == 0 {
            let struct_block = self.struct_block()?;
            if let Some(ref block) = struct_block {
                self.struct_defs.insert(block.id, block.clone());
            }

            Ok(struct_block.map(|s| Block::Struct(s)))
        } else {
            self.data_block(block_type)
        }
    }

    fn struct_block(&mut self) -> Result<Option<StructBlock>> {
        let valid = self.reader.read_u8()?;
        if valid == 0 {
            return Ok(None); // EOF
        }

        let id = self.reader.read_u32::<LittleEndian>()?;
        let name = Value::read_string(&mut self.reader)?;
        let mut fields = Vec::new();

        loop {
            let value_type = self.reader.read_u32::<LittleEndian>()?;
            if value_type == 0 {
                break;
            }

            let name = Value::read_string(&mut self.reader)?;
            let ordinal_table = if value_type == 0x37 {
                Some(self.read_ordinal_table()?)
            } else {
                None
            };

            fields.push(StructFieldDef {
                value_type,
                name,
                ordinal_table,
            })
        }

        Ok(Some(StructBlock { id, name, fields }))
    }

    fn data_block(&mut self, struct_id: u32) -> Result<Option<Block>> {
        let struct_def = self
            .struct_defs
            .get(&struct_id)
            .ok_or_else(|| anyhow!("missing struct def for {struct_id:X}"))?;

        let block_id = Value::read_id(&mut self.reader)?;

        let mut data = HashMap::with_capacity(struct_def.fields.len());
        for field in &struct_def.fields {
            dbg!(&field);
            let value = Value::read(&mut self.reader, &field)?;
            // dbg!(&field.name);
            dbg!(&value);
            data.insert(field.name.clone(), value);
        }

        // TODO: clean up option
        Ok(Some(Block::Data(DataBlock {
            id: block_id,
            fields: data,
        })))
    }

    fn read_ordinal_table(&mut self) -> Result<OrdinalStringTable> {
        let len = self.reader.read_u32::<LittleEndian>()?;
        let mut out = HashMap::with_capacity(len as usize);

        for _ in 0..len {
            let ordinal = self.reader.read_u32::<LittleEndian>()?;
            let string = Value::read_string(&mut self.reader)?;
            out.insert(ordinal, string);
        }

        Ok(out)
    }
}

pub enum ID {
    Nameless(u64),
    Named(Vec<u64>), // TODO: decode the u64s
}

const CHARTABLE: [u8; 37] = *b"0123456789abcdefghijklmnopqrstuvwxyz_";

impl ID {
    fn decode_part(mut part: u64) -> String {
        let mut res = String::new();
        while part > 0 {
            let index = (part % 38 - 1) as usize;
            res.push(CHARTABLE[index].into() /* TODO: annoying */);

            part /= 38;
        }

        res
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
                    .map(|p| ID::decode_part(*p))
                    .collect::<Vec<String>>() // TODO: do we really need this?
                    .join(".")
            }
        }
    }
}

impl std::fmt::Debug for ID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nameless(id) => f.debug_tuple("Nameless").field(&self.to_string()).finish(),
            Self::Named(id) => f.debug_tuple("Named").field(&self.to_string()).finish(),
        }
    }
}

macro_rules! def_vec {
    ($vname:ident, $iname:ident, $($itype:ty),+) => {
        pub type $vname = ($($itype),+);
    };
}

#[derive(Debug)]
pub enum Value {
    // 0x01
    String(String),
    // 0x02
    StringArray(Vec<String>),
    // 0x03
    EncodedString(u64),
    // 0x04
    EncodedStringArray(Vec<u64>),
    // 0x05
    Single(f32),
    // 0x06
    SingleArray(Vec<f32>),
    // 0x07
    Vec2s(Vec2s),
    // 0x09
    Vec3s(Vec3s),
    // 0x0A
    Vec3sArray(Vec<Vec3s>),
    // 0x11
    Vec3i(Vec3i),
    // 0x12
    Vec3iArray(Vec<Vec3i>),
    // 0x17
    Vec4s(Vec4s),
    // 0x18
    Vec4sArray(Vec<Vec4s>),
    // 0x19
    Vec8s(Vec8s),
    // 0x1A
    Vec8sArray(Vec<Vec8s>),
    // 0x25
    Int32(i32),
    // 0x26
    Int32Array(Vec<i32>),
    // 0x27
    UInt32(u32),
    // 0x28
    UInt32Array(Vec<u32>),
    // 0x2B
    UInt16(u16),
    // 0x2C
    UInt16Array(Vec<u16>),
    // 0x31
    Int64(i64),
    Int64Array(Vec<i64>),
    // 0x33
    UInt64(u64),
    UInt64Array(Vec<u64>),
    // 0x35
    ByteBool(bool),
    // 0x36
    ByteBoolArray(Vec<bool>),
    // 0x37
    OrdinalString(String),
    // 0x39
    ID(ID),
    // 0x3A
    IDArray(Vec<ID>),
}

macro_rules! def_array {
    ($aname:ident, $iname:ident, $itype:ty) => {
        pub fn $aname<R: Read>(reader: &mut R) -> Result<Vec<$itype>> {
            let len = reader.read_u32::<LittleEndian>()?;

            let mut vals = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vals.push(Self::$iname(reader)?);
            }

            Ok(vals)
        }
    };
}

// macro_rules! match_read {
//     {$r:expr, $v:expr, [$(($num:literal, $vtype:ident, $rname:ident)),+]} => {
//         match $v {
//             $($num => Ok(Value::$vtype(Self::$rname($r)?)),)+
//             _ => Err(anyhow!("unknown value type {0:X}", $v)),
//         }
//     };
// }

macro_rules! match_read {
    {$r:expr, $v:expr, [$(($num:literal, $vtype:ident, $itype:ty)),+]} => {
        match $v {
            $($num => Ok(Value::$vtype(<$itype>::read_from($r)?)),)+
            _ => Err(anyhow!("unknown value type {0:X}", $v)),
        }
    };
}

trait ReadFrom
where Self: Sized {
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

impl<T> ReadFrom for Vec<T>
where T: ReadFrom {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
            let len = reader.read_u32::<LittleEndian>()?;

            let mut vals = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vals.push(T::read_from(reader)?);
            }

            Ok(vals)
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

impl Value {
    pub fn read<R: Read>(reader: &mut R, field: &StructFieldDef) -> Result<Self> {
        if field.value_type == 0x37 {
            return Self::read_ordinal_string(reader, field);
        }

        match_read! {
            reader,
            field.value_type,
            [
                (0x01, String, String),
                (0x02, StringArray, Vec<String>),
                (0x03, EncodedString, u64),
                (0x04, EncodedStringArray, Vec<u64>),
                (0x05, Single, f32),
                (0x06, SingleArray, Vec<f32>),
                (0x07, Vec2s, Vec2s),
                (0x09, Vec3s, Vec3s),
                (0x0A, Vec3sArray, Vec<Vec3s>),
                (0x11, Vec3i, Vec3i),
                (0x12, Vec3iArray, Vec<Vec3i>),
                (0x17, Vec4s, Vec4s),
                (0x18, Vec4sArray, Vec<Vec4s>),
                (0x19, Vec8s, Vec8s),
                (0x1A, Vec8sArray, Vec<Vec8s>),
                (0x25, Int32, i32),
                (0x26, Int32Array, Vec<i32>),
                (0x27, UInt32, u32),
                (0x28, UInt32Array, Vec<u32>),
                (0x2B, UInt16, u16),
                (0x2C, UInt16Array, Vec<u16>),
                (0x2F, UInt32, u32), // Maybe
                (0x31, Int64, i64),
                (0x32, Int64Array, Vec<i64>), // Maybe
                (0x33, UInt64, u64),
                (0x34, UInt64Array, Vec<u64>),
                (0x35, ByteBool, bool),
                (0x36, ByteBoolArray, Vec<bool>),
                (0x39, ID, ID),
                (0x3A, IDArray, Vec<ID>),
                (0x3B, ID, ID),
                (0x3C, IDArray, Vec<ID>),
                (0x3D, ID, ID)
            ]
        }
    }

    pub fn read_id<R: Read>(reader: &mut R) -> Result<ID> {
        let len = reader.read_u8()?;

        if len == 0xFF {
            Ok(ID::Nameless(reader.read_u64::<LittleEndian>()?))
        } else {
            let mut parts = Vec::with_capacity(len as usize);
            for _ in 0..len {
                parts.push(reader.read_u64::<LittleEndian>()?);
            }

            Ok(ID::Named(parts))
        }
    }

    def_array!(read_id_array, read_id, ID);

    pub fn read_string<R: Read>(reader: &mut R) -> Result<String> {
        let len = reader.read_u32::<LittleEndian>()?;
        let mut out = Vec::new();
        out.resize(len as usize, 0u8);
        reader.read_exact(&mut out)?;

        String::from_utf8(out).map_err(|e| anyhow!("invalid string: {e}"))
    }

    def_array!(read_string_array, read_string, String);

    pub fn read_uint32<R: Read>(reader: &mut R) -> Result<u32> {
        Ok(reader.read_u32::<LittleEndian>()?)
    }

    def_array!(read_uint32_array, read_uint32, u32);

    pub fn read_uint64<R: Read>(reader: &mut R) -> Result<u64> {
        Ok(reader.read_u64::<LittleEndian>()?)
    }

    def_array!(read_uint64_array, read_uint64, u64);

    pub fn read_single<R: Read>(reader: &mut R) -> Result<f32> {
        Ok(reader.read_f32::<LittleEndian>()?)
    }

    def_array!(read_single_array, read_single, f32);

    pub fn read_vec2s<R: Read>(reader: &mut R) -> Result<Vec2s> {
        Ok((Self::read_single(reader)?, Self::read_single(reader)?))
    }

    pub fn read_vec3s<R: Read>(reader: &mut R) -> Result<Vec3s> {
        Ok((Self::read_single(reader)?, Self::read_single(reader)?, Self::read_single(reader)?))
    }

    def_array!(read_vec3s_array, read_vec3s, Vec3s);

    pub fn read_bytebool<R: Read>(reader: &mut R) -> Result<bool> {
        if reader.read_u8()? == 0 {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    def_array!(read_bytebool_array, read_bytebool, bool);

    fn read_ordinal_string<R: Read>(reader: &mut R, field: &StructFieldDef) -> Result<Value> {
        let ordinal = u32::read_from(reader)?;

        match field.ordinal_table {
            Some(ref t) => {
                let s = t.get(&ordinal).ok_or_else(|| anyhow!("missing ordinal table entry for {ordinal}"))?;

                Ok(Value::OrdinalString(s.clone()))
            }
            None => {
                bail!("missing ordinal table for {0}", field.name)
            }
        }
    }
}
