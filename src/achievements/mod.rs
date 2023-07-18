use anyhow::{Result, bail};
use std::{collections::{HashMap, BTreeMap}, fs::File, io::Read};

use crate::{sii::{value::ID, parser::DataBlock, game::GameSave}, data_get, achievements::sii_text::{Lexer, Parser}};

mod sii_text;

pub fn check_achievements(save: &GameSave, achievements_sii: &str) -> Result<()> {
    let save_data = AchievementSaveData::try_from(save)?;
    let f = File::open(achievements_sii)?;
    let lex = Lexer::new(f.bytes().peekable());
    let mut parser = Parser::new(lex).unwrap();

    loop {
        match parser.next() {
            Some(Ok(t)) if t.struct_name == "achievement_each_company_data" => {
                let id = t.id.to_string();
                if let Ok(achievement) = AchievementEachCompany::try_from(t) {
                    let (name, req) = achievement.eval(&save_data)?;
                    print_results(name, req);
                } else {
                    println!("skipping unhandled achievement {}", id);
                    println!();
                }
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
    Completed
}

struct Requirement {
    name: String,
    status: RequirementStatus,
    progress_description: String,
}

trait Achievement {
    fn eval(&self, save: &AchievementSaveData) -> Result<(String, Vec<Requirement>)>;
}

struct AchievementSaveData {
    jobs: Vec<(ID, ID)>
}

impl AchievementSaveData {
    fn count_jobs_by_target(&self, target: &ID) -> Result<usize> {
        Ok(self.jobs.iter().filter(|(_, t)| t == target).count())
    }

    fn company_id(s: &String) -> Result<ID> {
        let stripped = s.replace("company.volatile.", "");
        stripped.try_into()
    }
}

impl TryFrom<&GameSave> for AchievementSaveData {
    type Error = anyhow::Error;

    fn try_from(save: &GameSave) -> Result<Self> {
        let jobs = save.iter_blocks_named("delivery_log_entry")
            .map(|(_, e)| {
                let params = data_get!(e, "params", StringArray)?;
                Ok((Self::company_id(&params[1])?, Self::company_id(&params[2])?))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { jobs })
    }
}

struct AchievementEachCompany {
    id: ID,
    achievement_name: String,
    // target company ID -> required # of jobs
    targets: BTreeMap<ID, usize>
}

impl TryFrom<DataBlock> for AchievementEachCompany {
    type Error = anyhow::Error;

    fn try_from(value: DataBlock) -> Result<Self> {
        if value.struct_name != "achievement_each_company_data" {
            bail!("cannot decode AchievementEachCompany from {}", value.struct_name);
        }

        if value.fields.contains_key("sources") || value.fields.contains_key("cargos") {
            bail!("cannot decode achievement_each_company_data with sources or cargos yet");
        }

        let achievement_name = data_get!(value, "achievement_name", String)?
            .to_owned();
        let mut targets = BTreeMap::new();
        let target_arr = data_get!(value, "targets", StringArray)?;
        for t in target_arr {
            let target_id = ID::try_from(t.as_str())?;
            if !targets.contains_key(&target_id) {
                targets.insert(target_id.clone(), 0);
            }

            *(targets.get_mut(&target_id).expect("inserted if didn't exist")) += 1;
        }

        Ok(Self {
            id: value.id,
            achievement_name,
            targets
        })
    }
}

impl Achievement for AchievementEachCompany {
    fn eval(&self, save: &AchievementSaveData) -> Result<(String, Vec<Requirement>)> {
        let requirements = self.targets.iter().map(|(t, c)| {
            let completed = save.count_jobs_by_target(t)?;
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
                progress_description: format!("{}/{}", completed, c)
            })
        }).collect::<Result<Vec<_>>>()?;

        Ok((self.achievement_name.to_owned(), requirements))
    }
}