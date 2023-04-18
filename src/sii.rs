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

#[derive(Debug)]
pub enum Value {
    // 0x01
    String(String),
    // 0x02
    StringArray(Vec<String>),
    // 0x05
    Single(f32),
    // 0x07
    Vec2s(f32, f32),
    // 0x27
    UInt32(u32),
    // 0x28
    UInt32Array(Vec<u32>),
    // 0x33
    UInt64(u64),
    // 0x35
    ByteBool(bool),
    // 0x39
    ID(ID),
    // 0x3A
    IDArray(Vec<ID>),
}

impl Value {
    pub fn read<R: Read>(reader: &mut R, field: &StructFieldDef) -> Result<Self> {
        match field.value_type {
            0x01 => Ok(Value::String(Self::read_string(reader)?)),
            0x02 => Ok(Value::StringArray(Self::read_string_array(reader)?)),
            0x05 => Ok(Value::Single(Self::read_single(reader)?)),
            0x07 => {
                let (first, second) = Self::read_vec2s(reader)?;
                Ok(Value::Vec2s(first, second))
            }
            0x27 => Ok(Value::UInt32(Self::read_uint32(reader)?)),
            0x28 => Ok(Value::UInt32Array(Self::read_uint32_array(reader)?)),
            0x33 => Ok(Value::UInt64(Self::read_uint64(reader)?)),
            0x35 => Ok(Value::ByteBool(Self::read_bytebool(reader)?)),
            0x39 => Ok(Value::ID(Self::read_id(reader)?)),
            0x3A => Self::read_id_array(reader),
            _ => Err(anyhow!("unknown value type {0:X}", field.value_type)),
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

    fn read_id_array<R: Read>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u32::<LittleEndian>()?;

        let mut ids = Vec::with_capacity(len as usize);
        for _ in 0..len {
            ids.push(Self::read_id(reader)?);
        }

        Ok(Value::IDArray(ids))
    }

    pub fn read_string<R: Read>(reader: &mut R) -> Result<String> {
        let len = reader.read_u32::<LittleEndian>()?;
        let mut out = Vec::new();
        out.resize(len as usize, 0u8);
        reader.read_exact(&mut out)?;

        String::from_utf8(out).map_err(|e| anyhow!("invalid string: {e}"))
    }

    pub fn read_string_array<R: Read>(reader: &mut R) -> Result<Vec<String>> {
        let len = reader.read_u32::<LittleEndian>()?;

        let mut strs = Vec::with_capacity(len as usize);
        for _ in 0..len {
            strs.push(Self::read_string(reader)?);
        }

        Ok(strs)
    }

    pub fn read_uint32<R: Read>(reader: &mut R) -> Result<u32> {
        Ok(reader.read_u32::<LittleEndian>()?)
    }

    pub fn read_uint32_array<R: Read>(reader: &mut R) -> Result<Vec<u32>> {
        let len = reader.read_u32::<LittleEndian>()?;

        let mut ints = Vec::with_capacity(len as usize);
        for _ in 0..len {
            ints.push(Self::read_uint32(reader)?);
        }

        Ok(ints)
    }

    pub fn read_uint64<R: Read>(reader: &mut R) -> Result<u64> {
        Ok(reader.read_u64::<LittleEndian>()?)
    }

    pub fn read_single<R: Read>(reader: &mut R) -> Result<f32> {
        Ok(reader.read_f32::<LittleEndian>()?)
    }

    pub fn read_vec2s<R: Read>(reader: &mut R) -> Result<(f32, f32)> {
        Ok((Self::read_single(reader)?, Self::read_single(reader)?))
    }

    pub fn read_bytebool<R: Read>(reader: &mut R) -> Result<bool> {
        if reader.read_u8()? == 0 {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}
