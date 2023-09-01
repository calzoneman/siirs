use anyhow::{bail, Result};
use rusqlite::Connection;
use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    sii::{self},
    get_value_as,
    sii::value::{ID, Struct}, scs::Archive, sqlite,
};

use self::locale::LocaleDB;

mod locale;

const ACHIEVEMENTS_SII_HASH: u64 = 0x5C075DC23D8D177;

pub struct AchievementStatus {
    pub name: String,
    pub requirements: Vec<Requirement>
}

pub fn get_achievement_status(
    save_path: &str,
    game_path: &str
) -> Result<Vec<AchievementStatus>> {
    let save_parser = sii::binary::Parser::new_from_save(save_path)?;   
    let mut conn = Connection::open(":memory:")?;
    sqlite::copy_to_sqlite(save_parser, &mut conn)?;
    let save_data = AchievementSaveData::new(conn)?;

    let locale_scs_path = PathBuf::from(game_path).join("locale.scs");
    let locale_db = LocaleDB::new_from_locale_scs(locale_scs_path.to_str().expect("illegal filename"))?;
    let core_scs_path = PathBuf::from(game_path).join("core.scs");

    let mut core = Archive::load_from_path(core_scs_path.to_str().expect("illegal filename"))?;
    let reader = core.open_entry(ACHIEVEMENTS_SII_HASH)?;
    let mut parser = sii::text::Parser::new_from_reader(reader)?;
    let mut results = Vec::new();

    loop {
        match parser.next() {
            Some(Ok(t)) => {
                let achievement: Box<dyn Achievement> = match t.struct_name.as_str() {
                    "achievement_each_company_data" => Box::from(AchievementEachCompany::try_from(t)?),
                    "achievement_visit_city_data" => Box::from(AchievementVisitCity::try_from(t)?),
                    "achievement_each_cargo_data" => Box::from(AchievementEachCargo::try_from(t)?),
                    _ => { continue; }
                };

                let (name, requirements) = achievement.eval(&save_data, &locale_db)?;
                results.push(AchievementStatus { name, requirements })
            }
            Some(Err(e)) => {
                bail!(e)
            }
            None => break,
        }
    }

    Ok(results)
}

#[derive(Debug, Eq, PartialEq)]
pub enum RequirementStatus {
    NotStarted,
    Started,
    Completed,
}

pub struct Requirement {
    pub name: String,
    pub status: RequirementStatus,
    pub progress_description: String,
}

trait Achievement {
    fn eval(
        &self,
        save: &AchievementSaveData,
        ldb: &LocaleDB,
    ) -> Result<(String, Vec<Requirement>)>;
}

struct AchievementSaveData {
    conn: Connection,
}

impl AchievementSaveData {
    fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "
            CREATE TEMPORARY VIEW IF NOT EXISTS v_deliveries AS
            SELECT params->>1 AS source,
                   params->>2 AS target,
                   params->>3 AS cargo
              FROM delivery_log_entry;
        ",
        )?;
        Ok(Self { conn })
    }
}

struct AchievementEachCompany {
    achievement_name: String,
    match_field: &'static str,
    // company ID -> required # of jobs
    companies: BTreeMap<ID, usize>,
    required_cargo: Option<Vec<String>>,
}

impl TryFrom<Struct> for AchievementEachCompany {
    type Error = anyhow::Error;

    fn try_from(value: Struct) -> Result<Self> {
        if value.struct_name != "achievement_each_company_data" {
            bail!(
                "cannot decode AchievementEachCompany from {}",
                value.struct_name
            );
        }

        let match_field = if value.fields.contains_key("sources") {
            "sources"
        } else if value.fields.contains_key("targets") {
            "targets"
        } else {
            bail!("achievement {:?} lacks sources or targets", value.id)
        };

        let required_cargo = if value.fields.contains_key("cargos") {
            Some(get_value_as!(value, "cargos", StringArray)?.clone())
        } else {
            None
        };

        let achievement_name = get_value_as!(value, "achievement_name", String)?.to_owned();
        let mut companies = BTreeMap::new();
        let target_arr = get_value_as!(value, match_field, StringArray)?;
        for t in target_arr {
            let target_id = ID::try_from(t.as_str())?;
            if !companies.contains_key(&target_id) {
                companies.insert(target_id.clone(), 0);
            }

            *(companies.get_mut(&target_id).unwrap()) += 1;
        }

        Ok(Self {
            achievement_name,
            match_field,
            companies,
            required_cargo,
        })
    }
}

impl Achievement for AchievementEachCompany {
    fn eval(
        &self,
        save: &AchievementSaveData,
        _ldb: &LocaleDB,
    ) -> Result<(String, Vec<Requirement>)> {
        let mut query = format!(
            "SELECT COUNT(1) FROM temp.v_deliveries WHERE {} = ?",
            &self.match_field[..self.match_field.len() - 1]
        );
        let cargo_params = if let Some(ref cargo) = self.required_cargo {
            query += &format!(" AND cargo IN ({})", sqlite_placeholders(cargo.len()));
            cargo.iter().map(|c| format!("cargo.{}", c)).collect()
        } else {
            vec![]
        };
        let requirements = self
            .companies
            .iter()
            .map(|(t, c)| {
                let mut params = vec![format!("company.volatile.{}", t.to_string())];
                params.append(&mut cargo_params.clone());
                let completed: usize =
                    save.conn
                        .query_row(&query, rusqlite::params_from_iter(params), |row| row.get(0))?;

                let status = if completed >= *c {
                    RequirementStatus::Completed
                } else if completed > 0 {
                    RequirementStatus::Started
                } else {
                    RequirementStatus::NotStarted
                };

                Ok(Requirement {
                    name: t.to_string(),
                    status,
                    progress_description: format!("{}/{}", completed, c),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((self.achievement_name.to_owned(), requirements))
    }
}

struct AchievementVisitCity {
    achievement_name: String,
    cities: Vec<String>,
}

impl TryFrom<Struct> for AchievementVisitCity {
    type Error = anyhow::Error;

    fn try_from(value: Struct) -> Result<Self> {
        if get_value_as!(value, "event_name", String)? != "city_visited" {
            bail!("expected achievement_visit_city_data to have event_name = city_visited");
        }

        let achievement_name = get_value_as!(value, "achievement_name", String)?.clone();
        let cities = get_value_as!(value, "cities", StringArray)?.clone();
        Ok(Self {
            achievement_name,
            cities,
        })
    }
}

impl Achievement for AchievementVisitCity {
    fn eval(
        &self,
        save: &AchievementSaveData,
        ldb: &LocaleDB,
    ) -> Result<(String, Vec<Requirement>)> {
        let query = "
            SELECT (CASE WHEN ? IN (SELECT value FROM json_each(visited_cities))
                    THEN 1
                    ELSE 0 END)
              FROM ECONOMY;";
        let requirements = self
            .cities
            .iter()
            .map(|c| {
                let completed: usize = save.conn.query_row(&query, [c], |row| row.get(0))?;

                let status = if completed > 0 {
                    RequirementStatus::Completed
                } else {
                    RequirementStatus::NotStarted
                };

                Ok(Requirement {
                    name: ldb.try_localize(c).unwrap_or(c).to_string(),
                    status,
                    progress_description: "visit".to_owned(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((self.achievement_name.to_owned(), requirements))
    }
}

struct AchievementEachCargo {
    achievement_name: String,
    cargos: Vec<String>,
}

impl TryFrom<Struct> for AchievementEachCargo {
    type Error = anyhow::Error;

    fn try_from(value: Struct) -> Result<Self> {
        if value.struct_name != "achievement_each_cargo_data" {
            bail!(
                "cannot decode AchievementEachCargo from {}",
                value.struct_name
            );
        }

        let cargos = get_value_as!(value, "cargos", StringArray)?.clone();
        let achievement_name = get_value_as!(value, "achievement_name", String)?.to_owned();

        Ok(Self {
            achievement_name,
            cargos,
        })
    }
}

impl Achievement for AchievementEachCargo {
    fn eval(
        &self,
        save: &AchievementSaveData,
        ldb: &LocaleDB,
    ) -> Result<(String, Vec<Requirement>)> {
        let requirements = self
            .cargos
            .iter()
            .map(|c| {
                let completed: usize = save.conn.query_row(
                    "SELECT COUNT(1) FROM temp.v_deliveries WHERE cargo = 'cargo.' || ?",
                    [c],
                    |row| row.get(0),
                )?;

                let status = if completed > 0 {
                    RequirementStatus::Completed
                } else {
                    RequirementStatus::NotStarted
                };

                Ok(Requirement {
                    name: ldb
                        .try_localize(&format!("cn_{}", c))
                        .unwrap_or(c)
                        .to_string(),
                    status,
                    progress_description: format!("{}/1", completed),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((self.achievement_name.to_owned(), requirements))
    }
}

fn sqlite_placeholders(count: usize) -> String {
    let mut s = "?,".repeat(count);
    s.pop();
    s
}
