use anyhow::{anyhow, Result};
use std::{collections::HashMap, io::Read};

use crate::{get_value_as, scs::Archive, crypt::threenk, sii::text::{Lexer, Parser}};

pub struct LocaleDB(HashMap<String, String>);

impl LocaleDB {
    const EN_US_LOCAL_SII_HASH: u64 = 0x748A55BF49E4F39E; // locale/en_us/local.sii

    pub fn new_empty() -> Self {
        Self(HashMap::new())
    }

    pub fn new_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let lex = Lexer::new(reader.bytes().peekable());
        let mut parser = Parser::new(lex)?;
        let db_struct = parser
            .next()
            .ok_or_else(|| anyhow!("missing localization_db struct"))??;

        let mut entries = HashMap::new();
        let keys = get_value_as!(db_struct, "key", StringArray)?;
        let values = get_value_as!(db_struct, "val", StringArray)?;
        for (key, value) in keys.iter().zip(values.iter()) {
            entries.insert(key.clone(), value.clone());
        }

        Ok(Self(entries))
    }

    pub fn new_from_locale_scs(locale_scs_path: &str) -> Result<Self> {
        let mut locale_scs = Archive::load_from_path(locale_scs_path)?;
        let reader = locale_scs.open_entry(Self::EN_US_LOCAL_SII_HASH)?;
        let mut decryptor = threenk::Decryptor::new(reader)?;
        Self::new_from_reader(&mut decryptor)
    }

    pub fn try_localize(&self, key: &String) -> Option<&String> {
        self.0.get(key)
    }
}
