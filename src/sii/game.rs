use std::{collections::HashMap, io::Read};

use crate::get_value_as;
use anyhow::{anyhow, Result};

use super::{
    binary::{Block, Parser},
    value::{ID, Struct},
};

pub struct GameSave {
    blocks: HashMap<ID, Struct>,
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
        let dlog_entry_ids = get_value_as!(dlog, "entries", IDArray)?;

        Ok(Self {
            total_fuel_liters: *get_value_as!(econ, "total_fuel_litres", UInt32)?,
            total_fuel_cost: *get_value_as!(econ, "total_fuel_price", Int64)?,
            total_fuel_visits: *get_value_as!(econ, "gas_station_visit_count", UInt32)?,
            total_xp: *get_value_as!(econ, "experience_points", UInt32)?,
            total_distance_driven: *get_value_as!(econ, "total_distance", UInt32)?,
            total_cities_visited: get_value_as!(econ, "visited_cities", EncodedStringArray)?.len(),
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
                Some(Block::Schema(_)) => {}
                Some(Block::Struct(db)) => {
                    objects.insert(db.id.clone(), db);
                }
            }
        }

        Ok(Self { blocks: objects })
    }

    pub fn get_block_by_id(&self, id: &ID) -> Option<&Struct> {
        self.blocks.get(id)
    }

    pub fn iter_blocks_named<'a>(
        &'a self,
        name: &'a str,
    ) -> Box<dyn Iterator<Item = (&ID, &Struct)> + 'a> {
        Box::new(
            self.blocks
                .iter()
                .filter(move |(_, s)| s.struct_name == name),
        )
    }

    pub fn iter_blocks<'a>(&'a self) -> Box<dyn Iterator<Item = (&ID, &Struct)> + 'a> {
        Box::new(self.blocks.iter())
    }

    pub fn single_block_named(&self, name: &str) -> Option<&Struct> {
        self.blocks
            .iter()
            .find(|(_, s)| s.struct_name == name)
            .map(|(_, block)| block)
    }
}
