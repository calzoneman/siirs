use std::io::Read;

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};

const SII_ENCRYPTED_SIGNATURE: u32 = 0x43736353;
const SII_AES_KEY: [u8; 32] = [
    0x2A, 0x5F, 0xCB, 0x17, 0x91, 0xD2, 0x2F, 0xB6, 0x02, 0x45, 0xB3, 0xD8, 0x36, 0x9E, 0xD0, 0xB2,
    0xC2, 0x73, 0x71, 0x56, 0x3F, 0xBF, 0x1F, 0x3C, 0x9E, 0xDF, 0x6B, 0x11, 0x82, 0x5A, 0x5D, 0x0A,
];

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

pub fn decrypt_sii(file: &[u8]) -> Result<Vec<u8>> {
    let mut file = file;
    let mut hmac: [u8; 32] = [0; 32];
    let mut iv: [u8; 16] = [0; 16];

    let signature = file.read_u32::<LittleEndian>()?;
    if signature != SII_ENCRYPTED_SIGNATURE {
        bail!("invalid signature: {signature:X}")
    }

    file.read_exact(&mut hmac)?;
    file.read_exact(&mut iv)?;
    let _len = file.read_u32::<LittleEndian>()?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;

    Aes256CbcDec::new(&SII_AES_KEY.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| anyhow!("decryption failed: {e}"))?;

    Ok(buf)
}
