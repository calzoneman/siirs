#![allow(non_snake_case)]
use std::{fs::File, env};

use anyhow::{Result, bail};
use siirs::crypt::threenk::Decryptor;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("Usage: {} <path to input 3nK encrypted file> <path to output sii>", args[0]);
    }

    let in_file = File::options()
        .read(true)
        .write(false)
        .open(&args[1])?;
    let mut out_file = File::options()
        .create_new(true)
        .write(true)
        .open(&args[2])?;

    let mut decryptor = Decryptor::new(in_file)?;
    std::io::copy(&mut decryptor, &mut out_file)?;

    Ok(())
}