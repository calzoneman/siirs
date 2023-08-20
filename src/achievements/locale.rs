use anyhow::{anyhow, Result};
use std::{collections::HashMap, fs::File, io::Read};

use crate::data_get;

use super::sii_text::{Lexer, Parser};

pub struct LocaleDB(HashMap<String, String>);

impl LocaleDB {
    pub fn new_empty() -> Self {
        Self(HashMap::new())
    }

    pub fn new_from_file(filename: &str) -> Result<Self> {
        let f = File::open(filename)?;
        let lex = Lexer::new(f.bytes().peekable());
        let mut parser = Parser::new(lex)?;
        let db_struct = parser
            .next()
            .ok_or_else(|| anyhow!("missing localization_db struct"))??;

        let mut entries = HashMap::new();
        let keys = data_get!(db_struct, "key", StringArray)?;
        let values = data_get!(db_struct, "val", StringArray)?;
        for (key, value) in keys.iter().zip(values.iter()) {
            entries.insert(key.clone(), value.clone());
        }

        Ok(Self(entries))
    }

    pub fn try_localize(&self, key: &String) -> Option<&String> {
        self.0.get(key)
    }
}
