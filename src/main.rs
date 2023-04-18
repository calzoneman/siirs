use std::{fs::File, io::Read};

use anyhow::{bail, Result};
use compress::zlib;

mod crypt;
mod sii;

fn main() -> Result<()> {
    let mut enc_file = File::open("/home/calvin/programming/siistuff/game.sii")?;
    let mut enc_bytes = Vec::default();
    enc_file.read_to_end(&mut enc_bytes)?;

    let decrypted = crypt::decrypt_sii(&enc_bytes)?;
    let mut decoder = zlib::Decoder::new(decrypted.as_slice());
    //let mut decoded = Vec::default();
    //decoder.read_to_end(&mut decoded)?;
    let mut sii_reader = sii::Reader::new(decoder)?;
    loop {
        match sii_reader.next_block() {
            Ok(None) => break,
            Ok(Some(b)) => match b {
                sii::Block::Struct(s) => {
                    println!("{:X} {}", s.id, s.name);
                }
                sii::Block::Data(d) => {
                    dbg!(&d);
                }
                _ => {}
            },
            Err(e) => bail!("shit: {e}"),
        }
    }

    Ok(())
}
