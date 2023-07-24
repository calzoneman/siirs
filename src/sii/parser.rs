use anyhow::{anyhow, bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{collections::HashMap, io::Read};

use super::value::{OrdinalStringTable, ReadFrom, Value, ID};

#[derive(Clone, Debug)]
pub struct StructFieldDef {
    pub value_type: u32,
    pub name: String,
    pub ordinal_table: Option<OrdinalStringTable>,
}

#[derive(Clone, Debug)]
pub struct StructDef {
    pub id: u32,
    pub name: String,
    pub fields: Vec<StructFieldDef>,
}

#[derive(Debug)]
pub struct DataBlock {
    pub id: ID,
    pub struct_name: String,
    pub fields: HashMap<String, Value>,
}

#[macro_export]
macro_rules! data_get {
    ($b:ident, $fname:expr, $variant:ident) => {
        match $b.fields.get($fname) {
            None => Err(anyhow::anyhow!("missing field {}", $fname)),
            Some(crate::sii::value::Value::$variant(v)) => Ok(v),
            Some(_) => Err(anyhow::anyhow!("mismatched type for {}", $fname)),
        }
    };
}

pub enum Block {
    Struct(StructDef),
    Data(DataBlock),
}

pub struct Parser<R: Read> {
    reader: R,
    struct_defs: HashMap<u32, StructDef>,
}

impl<R: Read> Parser<R> {
    const SII_SIGNATURE: u32 = 0x49495342;

    pub fn new(mut reader: R) -> Result<Self> {
        let signature = reader.read_u32::<LittleEndian>()?;
        if signature != Self::SII_SIGNATURE {
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
            let struct_def = self.parse_struct_def()?;
            if let Some(ref block) = struct_def {
                self.struct_defs.insert(block.id, block.clone());
            }

            Ok(struct_def.map(Block::Struct))
        } else {
            Ok(Some(self.parse_data_block(block_type)?))
        }
    }

    fn parse_struct_def(&mut self) -> Result<Option<StructDef>> {
        if !bool::read_from(&mut self.reader)? {
            return Ok(None); // EOF
        }

        let id = self.reader.read_u32::<LittleEndian>()?;
        let name = String::read_from(&mut self.reader)?;
        let mut fields = Vec::new();

        loop {
            let value_type = self.reader.read_u32::<LittleEndian>()?;
            if value_type == 0 {
                break;
            }

            let name = String::read_from(&mut self.reader)?;
            let ordinal_table = if value_type == 0x37 {
                Some(OrdinalStringTable::read_from(&mut self.reader)?)
            } else {
                None
            };

            fields.push(StructFieldDef {
                value_type,
                name,
                ordinal_table,
            })
        }

        Ok(Some(StructDef { id, name, fields }))
    }

    fn parse_data_block(&mut self, struct_id: u32) -> Result<Block> {
        let struct_def = self
            .struct_defs
            .get(&struct_id)
            .ok_or_else(|| anyhow!("missing struct def for {struct_id:X}"))?;

        let block_id = ID::read_from(&mut self.reader)?;

        let mut data = HashMap::with_capacity(struct_def.fields.len());
        for field in &struct_def.fields {
            let value = Value::read_from(
                &mut self.reader,
                field.value_type,
                field.ordinal_table.as_ref(),
            )?;
            data.insert(field.name.clone(), value);
        }

        Ok(Block::Data(DataBlock {
            id: block_id,
            struct_name: struct_def.name.clone(),
            fields: data,
        }))
    }
}
