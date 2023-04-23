use std::{
    collections::HashMap,
    io::Read,
};

use crate::data_get;
use anyhow::{anyhow, Result};

use super::{
    parser::{Block, DataBlock, Parser},
    value::ID,
};

pub struct GameSave {
    blocks: HashMap<ID, DataBlock>,
}

pub trait FromGameSave
where
    Self: Sized,
{
    fn from_game_save(save: &GameSave) -> Result<Self>;
}

#[derive(Debug)]
pub struct SaveSummary {
    pub total_fuel_liters: u32,
    pub total_fuel_cost: i64,
    pub total_fuel_visits: u32,
    pub total_xp: u32,
    pub total_distance_driven: u32,
    pub total_cities_visited: usize,
    pub total_deliveries: usize,
}

impl FromGameSave for SaveSummary {
    fn from_game_save(save: &GameSave) -> Result<Self> {
        let econ = save
            .single_block_named("economy")
            .ok_or_else(|| anyhow!("missing economy data"))?;
        let dlog = save
            .single_block_named("delivery_log")
            .ok_or_else(|| anyhow!("missing delivery_log data"))?;
        let dlog_entry_ids = data_get!(dlog, "entries", IDArray)?;

        Ok(Self {
            total_fuel_liters: *data_get!(econ, "total_fuel_litres", UInt32)?,
            total_fuel_cost: *data_get!(econ, "total_fuel_price", Int64)?,
            total_fuel_visits: *data_get!(econ, "gas_station_visit_count", UInt32)?,
            total_xp: *data_get!(econ, "experience_points", UInt32)?,
            total_distance_driven: *data_get!(econ, "total_distance", UInt32)?,
            total_cities_visited: data_get!(econ, "visited_cities", EncodedStringArray)?.len(),
            total_deliveries: dlog_entry_ids.len(),
        })
    }
}

impl GameSave {
    pub fn new<R: Read>(reader: R) -> Result<Self> {
        let mut parser = Parser::new(reader)?;
        let mut objects = HashMap::new();

        loop {
            match parser.next_block()? {
                None => break,
                Some(Block::Struct(_)) => {}
                Some(Block::Data(db)) => {
                    objects.insert(db.id.clone(), db);
                }
            }
        }

        Ok(Self { blocks: objects })
    }

    #[allow(dead_code)]
    pub fn iter_blocks_named<'a>(
        &'a self,
        name: &'a str,
    ) -> Box<dyn Iterator<Item = (&ID, &DataBlock)> + 'a> {
        Box::new(
            self.blocks
                .iter()
                .filter(move |(_, s)| s.struct_name == name),
        )
    }

    pub fn single_block_named(&self, name: &str) -> Option<&DataBlock> {
        self.blocks
            .iter()
            .find(|(_, s)| s.struct_name == name)
            .map(|(_, block)| block)
    }
}
