use anyhow::{anyhow, Result};
use std::{collections::HashMap, io::Read};

use crate::{scs::Archive, crypt::threenk, sii::text::{Lexer, Parser}, take_value_as};

pub struct LocaleDB(HashMap<String, String>);

impl LocaleDB {
    const EN_US_LOCAL_SII_HASH: u64 = 0x748A55BF49E4F39E; // locale/en_us/local.sii

    #[allow(dead_code)]
    pub fn new_empty() -> Self {
        Self(HashMap::new())
    }

    pub fn new_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let lex = Lexer::new(reader.bytes().peekable());
        let mut parser = Parser::new(lex)?;
        let mut db_struct = parser
            .next()
            .ok_or_else(|| anyhow!("missing localization_db struct"))??;

        let mut entries = HashMap::new();
        let keys = take_value_as!(db_struct, "key", StringArray)?;
        let values = take_value_as!(db_struct, "val", StringArray)?;
        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            entries.insert(key, value);
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
