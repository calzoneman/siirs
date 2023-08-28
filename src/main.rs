use std::{env, fs::File};

use anyhow::{bail, Result};
use flate2::read::ZlibDecoder;
use rusqlite::Connection;
use crypt::sii::Decryptor;
use sii::{
    game::{FromGameSave, GameSave, SaveSummary},
    parser::Parser,
};

mod achievements;
mod prospective_cities;
mod scs;
mod sii;
mod sqlite;
mod crypt;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        bail!("Usage: {} summary <file>", args[0]);
    }

    match args[1].as_str() {
        "to-sqlite" => {
            let enc_file = File::open(&args[2])?;
            let decrypted = Decryptor::new(enc_file).decrypt()?;
            let zread = ZlibDecoder::new(decrypted.as_slice());
            let parser = Parser::new(zread)?;
            let mut conn = Connection::open(&args[3])?;
            sqlite::copy_to_sqlite(parser, &mut conn)?;
        }
        "check-achievements-v2" => {
            let enc_file = File::open(&args[2])?;
            let decrypted = Decryptor::new(enc_file).decrypt()?;
            let zread = ZlibDecoder::new(decrypted.as_slice());
            let parser = Parser::new(zread)?;
            let mut conn = Connection::open(":memory:")?;
            sqlite::copy_to_sqlite(parser, &mut conn)?;
            achievements::check_achievements(
                conn,
                &args[3],
                Some(&args[4]),
            )?;
        }
        "decrypt-3nk" => {
            let mut enc_file = File::open(&args[2])?;
            let mut dec_file = File::create(&args[3])?;
            crypt::threenk::decrypt_into(&mut enc_file, &mut dec_file)?;
        }

        _ => {
            let fname = &args[2];
            let enc_file = File::open(fname)?;
            let decrypted = Decryptor::new(enc_file).decrypt()?;
            let zread = ZlibDecoder::new(decrypted.as_slice());
            let save = GameSave::new(zread)?;

            match args[1].as_str() {
                "summary" => {
                    let summary = SaveSummary::from_game_save(&save)?;
                    println!("{:?}", summary);
                }
                "prospective-cities" => {
                    prospective_cities::find_profitable_cities(&save)?;
                }
                "debug-list-blocks" => {
                    for (id, block) in save.iter_blocks() {
                        println!("{}\tid={:?}", block.struct_name, id);
                    }
                }
                "debug-dump-blocks-named" => {
                    for (_, block) in save.iter_blocks_named(&args[3]) {
                        println!("{:?}", block);
                    }
                }
                _ => bail!("unknown command {}", args[1]),
            }
        }
    }

    Ok(())
}
