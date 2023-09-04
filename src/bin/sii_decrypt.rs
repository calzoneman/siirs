use std::{fs::File, env, io::Write};

use anyhow::{Result, bail};
use flate2::read::ZlibDecoder;
use siirs::crypt::sii::Decryptor;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("Usage: {} <path to input sii> <path to output file>", args[0]);
    }

    let in_file = File::options()
        .read(true)
        .write(false)
        .open(&args[1])?;
    let mut out_file = File::options()
        .create_new(true)
        .write(true)
        .open(&args[2])?;

    let decryptor = Decryptor::new(in_file);
    let decrypted = decryptor.decrypt()?;

    if decrypted[0] == 0x78 && decrypted[1] == 0x9C {
        let mut zlib_in = ZlibDecoder::new(decrypted.as_slice());
        std::io::copy(&mut zlib_in, &mut out_file)?;
    } else {
        out_file.write_all(&decrypted)?;
    }

    Ok(())
}