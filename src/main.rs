use std::fs::File;

use anyhow::{bail, Result};
use compress::zlib;
use sii::{crypt::Decryptor, parser::Parser};

mod sii;

fn main() -> Result<()> {
    let enc_file = File::open("/home/calvin/programming/siistuff/game.sii")?;
    let decrypted = Decryptor::new(enc_file).decrypt()?;
    let zread = zlib::Decoder::new(decrypted.as_slice());
    let mut parser = Parser::new(zread)?;

    loop {
        match parser.next_block() {
            Ok(None) => break,
            Ok(Some(b)) => match b {
                sii::parser::Block::Struct(s) => {
                    // println!("{:X} {}", s.id, s.name);
                    // for f in s.fields {
                    // println!("  - {:X} {}", f.value_type, f.name);
                    // }
                }
                sii::parser::Block::Data(d) => {
                    if d.struct_name == "mail_def" {
                        dbg!(&d);
                    }
                    // dbg!(&d);
                }
                _ => {}
            },
            Err(e) => bail!("shit: {e}"),
        }
    }

    Ok(())
}
