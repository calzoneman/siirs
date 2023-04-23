use std::{fs::File, env};

use anyhow::{Result, bail};
use compress::zlib;
use sii::{
    crypt::Decryptor,
    game::{FromGameSave, GameSave, SaveSummary},
};

mod sii;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args[1] != "summary" {
        bail!("Usage: {} summary <file>", args[0]);
    }

    let fname = &args[2];
    let enc_file = File::open(fname)?;
    let decrypted = Decryptor::new(enc_file).decrypt()?;
    let zread = zlib::Decoder::new(decrypted.as_slice());
    let save = GameSave::new(zread)?;
    let summary = SaveSummary::from_game_save(&save)?;
    println!("{:?}", summary);

    Ok(())
}
