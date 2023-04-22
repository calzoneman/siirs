use std::io::Read;

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

pub struct Decryptor<R: Read> {
    reader: R,
}

impl<R> Decryptor<R>
where
    R: Read,
{
    const SII_ENCRYPTED_SIGNATURE: u32 = 0x43736353;
    const SII_AES_KEY: [u8; 32] = [
        0x2A, 0x5F, 0xCB, 0x17, 0x91, 0xD2, 0x2F, 0xB6, 0x02, 0x45, 0xB3, 0xD8, 0x36, 0x9E, 0xD0,
        0xB2, 0xC2, 0x73, 0x71, 0x56, 0x3F, 0xBF, 0x1F, 0x3C, 0x9E, 0xDF, 0x6B, 0x11, 0x82, 0x5A,
        0x5D, 0x0A,
    ];

    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn decrypt(mut self) -> Result<Vec<u8>> {
        let iv = self.read_header()?;
        let mut buf = Vec::new();
        self.reader.read_to_end(&mut buf)?;

        Aes256CbcDec::new(&Self::SII_AES_KEY.into(), &iv.into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|e| anyhow!("decryption failed: {e}"))?;

        Ok(buf)
    }

    fn read_header(&mut self) -> Result<[u8; 16]> {
        let mut hmac: [u8; 32] = [0; 32];
        let mut iv: [u8; 16] = [0; 16];

        let signature = self.reader.read_u32::<LittleEndian>()?;
        if signature != Self::SII_ENCRYPTED_SIGNATURE {
            bail!("invalid signature: {signature:X}")
        }

        self.reader.read_exact(&mut hmac)?;
        self.reader.read_exact(&mut iv)?;
        let _len = self.reader.read_u32::<LittleEndian>()?;

        Ok(iv)
    }
}
