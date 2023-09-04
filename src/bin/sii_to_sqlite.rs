use std::{fs::File, env};

use anyhow::{Result, bail};
use flate2::read::ZlibDecoder;
use rusqlite::Connection;
use siirs::{crypt::sii::Decryptor, sii::binary::Parser, sqlite};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("Usage: {} <path to input sii> <path to output sqlite>", args[0]);
    }

    let in_file = File::options()
        .read(true)
        .write(false)
        .open(&args[1])?;

    let decryptor = Decryptor::new(in_file);
    let decrypted = decryptor.decrypt()?;
    let zlib_in = ZlibDecoder::new(decrypted.as_slice());
    let parser = Parser::new(zlib_in)?;
    let mut conn = Connection::open(&args[2])?;
    sqlite::copy_to_sqlite(parser, &mut conn)?;

    Ok(())
}