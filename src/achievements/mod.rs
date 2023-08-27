use anyhow::{bail, Result};
use rusqlite::Connection;
use std::{collections::BTreeMap, fs::File, io::Read};

use crate::{
    achievements::sii_text::{Lexer, Parser},
    data_get,
    sii::{parser::DataBlock, value::ID}, scs::Archive,
};

use self::locale::LocaleDB;

mod locale;
mod sii_text;

const ACHIEVEMENTS_SII_HASH: u64 = 0x5C075DC23D8D177;

pub fn check_achievements(
    conn: Connection,
    core_scs_path: &str,
    locale_sii: Option<&str>,
) -> Result<()> {
    let save_data = AchievementSaveData::new(conn)?;
    // TODO: load from locale.scs
    let locale_db = if let Some(filename) = locale_sii {
        LocaleDB::new_from_file(filename)?
    } else {
        LocaleDB::new_empty()
    };

    let mut core = Archive::load_from_path(core_scs_path)?;
    let reader = core.open_entry(ACHIEVEMENTS_SII_HASH)?;
    let lex = Lexer::new(reader.bytes().peekable());
    let mut parser = Parser::new(lex)?;

    loop {
        match parser.next() {
            Some(Ok(t)) if t.struct_name == "achievement_each_company_data" => {
                let achievement = AchievementEachCompany::try_from(t)?;
                let (name, req) = achievement.eval(&save_data, &locale_db)?;
                print_results(name, req);
            }
            Some(Ok(t)) if t.struct_name == "achievement_visit_city_data" => {
                let achievement = AchievementVisitCity::try_from(t)?;
                let (name, req) = achievement.eval(&save_data, &locale_db)?;
                print_results(name, req);
            }
            Some(Ok(t)) if t.struct_name == "achievement_each_cargo_data" => {
                let achievement = AchievementEachCargo::try_from(t)?;
                let (name, req) = achievement.eval(&save_data, &locale_db)?;
                print_results(name, req);
            }
            Some(Ok(_)) => {}
            Some(Err(e)) => {
                bail!(e)
            }
            None => break,
        }
    }

    Ok(())
}

fn print_results(achievement_name: String, requirements: Vec<Requirement>) {
    print_boxed(&achievement_name);
    for req in requirements {
        let prefix = if req.status == RequirementStatus::Completed {
            "\x1b[1;32m✓\x1b[1;30m "
        } else {
            "\x1b[0m  "
        };

        println!("{} {}: {}", prefix, req.progress_description, req.name);
    }

    println!("\x1b[0m")
}

fn print_boxed(s: &str) {
    println!("╭─{}─╮", "─".repeat(s.len()));
    println!("│ {} │", s);
    println!("╰─{}─╯", "─".repeat(s.len()));
}

#[derive(Debug, Eq, PartialEq)]
enum RequirementStatus {
    NotStarted,
    Started,
    Completed,
}

struct Requirement {
    name: String,
    status: RequirementStatus,
    progress_description: String,
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

impl TryFrom<DataBlock> for AchievementEachCompany {
    type Error = anyhow::Error;

    fn try_from(value: DataBlock) -> Result<Self> {
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
            Some(data_get!(value, "cargos", StringArray)?.clone())
        } else {
            None
        };

        let achievement_name = data_get!(value, "achievement_name", String)?.to_owned();
        let mut companies = BTreeMap::new();
        let target_arr = data_get!(value, match_field, StringArray)?;
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

impl TryFrom<DataBlock> for AchievementVisitCity {
    type Error = anyhow::Error;

    fn try_from(value: DataBlock) -> Result<Self> {
        if data_get!(value, "event_name", String)? != "city_visited" {
            bail!("expected achievement_visit_city_data to have event_name = city_visited");
        }

        let achievement_name = data_get!(value, "achievement_name", String)?.clone();
        let cities = data_get!(value, "cities", StringArray)?.clone();
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

impl TryFrom<DataBlock> for AchievementEachCargo {
    type Error = anyhow::Error;

    fn try_from(value: DataBlock) -> Result<Self> {
        if value.struct_name != "achievement_each_cargo_data" {
            bail!(
                "cannot decode AchievementEachCargo from {}",
                value.struct_name
            );
        }

        let cargos = data_get!(value, "cargos", StringArray)?.clone();
        let achievement_name = data_get!(value, "achievement_name", String)?.to_owned();

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
