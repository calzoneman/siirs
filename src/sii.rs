use anyhow::{anyhow, Result, bail};
use byteorder::{ReadBytesExt, LittleEndian};
use std::{collections::HashMap, io::Read};

const SII_SIGNATURE: u32 = 0x49495342;

pub type OrdinalStringTable = HashMap<u32, String>;

#[derive(Clone, Debug)]
pub struct StructFieldDef {
    pub value_type: u32,
    pub name: String,
    pub ordinal_table: Option<OrdinalStringTable>
}

#[derive(Clone, Debug)]
pub struct StructBlock {
    pub id: u32,
    pub name: String,
    pub fields: Vec<StructFieldDef>
}

#[derive(Debug)]
pub struct DataBlock {
    pub id: ID,
    pub fields: HashMap<String, Value>
}

pub enum Block {
    Struct(StructBlock),
    Data(DataBlock)
}

pub struct Reader<R: Read> {
    reader: R,
    struct_defs: HashMap<u32, StructBlock>
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
            struct_defs: HashMap::new()
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
        let name = self.read_string()?;
        let mut fields = Vec::new();

        loop {
            let value_type = self.reader.read_u32::<LittleEndian>()?;
            if value_type == 0 {
                break;
            }

            let name = self.read_string()?;
            let ordinal_table = if value_type == 0x37 {
                Some(self.read_ordinal_table()?)
            } else {
                None
            };

            fields.push(StructFieldDef {
                value_type,
                name,
                ordinal_table
            })
        }

        Ok(Some(StructBlock {
            id,
            name,
            fields
        }))
    }

    fn data_block(&mut self, struct_id: u32) -> Result<Option<Block>> {
        let struct_def = self.struct_defs.get(&struct_id)
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
        Ok(Some(Block::Data(DataBlock { id: block_id, fields: data })))
    }

    fn read_string(&mut self) -> Result<String> {
        let len = self.reader.read_u32::<LittleEndian>()?;
        let mut out = Vec::new();
        out.resize(len as usize, 0u8);
        self.reader.read_exact(&mut out)?;

        String::from_utf8(out)
            .map_err(|e| anyhow!("invalid string: {e}"))
    }

    fn read_ordinal_table(&mut self) -> Result<OrdinalStringTable> {
        let len = self.reader.read_u32::<LittleEndian>()?;
        let mut out = HashMap::with_capacity(len as usize);

        for _ in 0..len {
            let ordinal = self.reader.read_u32::<LittleEndian>()?;
            let string = self.read_string()?;
            out.insert(ordinal, string);
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub enum ID {
    Nameless(u64),
    Named(Vec<u64>) // TODO: decode the u64s
}

#[derive(Debug)]
pub enum Value {
    ID(ID),
    IDArray(Vec<ID>)
}

impl Value {
    pub fn read<R: Read>(reader: &mut R, field: &StructFieldDef) -> Result<Self> {
        match field.value_type {
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
        dbg!(len);

        let mut ids = Vec::with_capacity(len as usize);
        for _ in 0..len {
            ids.push(Self::read_id(reader)?);
        }

        Ok(Value::IDArray(ids))
    }
}