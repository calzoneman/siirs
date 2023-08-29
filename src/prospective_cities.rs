use std::collections::HashMap;

use crate::{
    get_value_as,
    sii::{game::GameSave, value::Struct},
};
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CityScore {
    city: String,
    score: i32,
}

impl PartialOrd for CityScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl Ord for CityScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

pub fn find_profitable_cities(save: &GameSave) -> Result<()> {
    let mut city_to_jobs = HashMap::new();

    for (company_id, company) in save.iter_blocks_named("company") {
        let city_id = company_id
            .string_part(-1)
            .ok_or_else(|| anyhow!("could not extract city name from {company_id:?}"))?;
        if !city_to_jobs.contains_key(&city_id) {
            city_to_jobs.insert(city_id.clone(), Vec::new());
        }
        let offer_ids = get_value_as!(company, "job_offer", IDArray)?;

        for offer_id in offer_ids {
            let offer = save
                .get_block_by_id(offer_id)
                .ok_or_else(|| anyhow!("could not locate job offer {offer_id:?}"))?;
            let expiration = *get_value_as!(offer, "expiration_time", UInt32)?;
            // There seem to be a bunch of placeholder job offers with expirations of -1
            if expiration != (-1i32 as u32) {
                city_to_jobs.get_mut(&city_id).unwrap().push(offer);
            }
        }
    }

    let mut scores = Vec::new();

    for (city, jobs) in &city_to_jobs {
        let mut score = 0;

        for job in jobs {
            if has_return_job(city, job, &city_to_jobs)? {
                score += 1;
            } else {
                score -= 1;
            }
        }

        scores.push((city, score));
    }

    scores.sort_by(|a, b| b.1.cmp(&a.1));

    for (city, score) in scores {
        println!("{}:\t{}", city, score);
    }

    Ok(())
}

fn target_city(job: &Struct) -> Result<&str> {
    let dest_company = get_value_as!(job, "target", String)?;
    dest_company
        .split(".")
        .last()
        .ok_or_else(|| anyhow!("could not parse job destination {dest_company}"))
}

fn has_return_job(
    origin: &String,
    job: &Struct,
    city_to_jobs: &HashMap<String, Vec<&Struct>>,
) -> Result<bool> {
    let dest_city = target_city(job)?;

    if let Some(dest_jobs) = city_to_jobs.get(dest_city) {
        for return_job in dest_jobs {
            let return_dest = target_city(return_job)?;
            if return_dest == origin {
                return Ok(true);
            }
        }

        Ok(false)
    } else {
        Ok(false)
    }
}
