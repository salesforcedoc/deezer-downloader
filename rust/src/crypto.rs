use anyhow::{anyhow, Result};
use blowfish::Blowfish;
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use md5::{Digest, Md5};

const BLOWFISH_KEY: &[u8; 16] = b"g4el58wc0zvf9na1";
const IV: [u8; 8] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];

type Decryptor = cbc::Decryptor<Blowfish>;

/// Deezer derives the Blowfish key from the MD5 of the song id.
pub fn calc_bf_key(song_id: &str) -> [u8; 16] {
    // Deezer uses the *hex* MD5 (32 characters), not the raw 16 byte digest.
    let digest = hex::encode(Md5::digest(song_id.as_bytes()));
    let digest = digest.as_bytes();
    let mut key = [0u8; 16];
    for i in 0..16 {
        key[i] = digest[i] ^ digest[i + 16] ^ BLOWFISH_KEY[i];
    }
    key
}

/// Decrypt one 2048 byte block in place.
///
/// Deezer encrypts every third 2048 byte block of the stream with Blowfish-CBC
/// and no padding. The caller is responsible for feeding whole blocks.
pub fn decrypt_block_in_place(buffer: &mut [u8], key: &[u8; 16]) -> Result<()> {
    if buffer.len() % 8 != 0 {
        return Err(anyhow!(
            "encrypted block length {} is not a multiple of 8",
            buffer.len()
        ));
    }
    let decryptor = Decryptor::new_from_slices(key, &IV)
        .map_err(|err| anyhow!("invalid Blowfish key or IV: {err}"))?;
    decryptor
        .decrypt_padded_mut::<NoPadding>(buffer)
        .map_err(|err| anyhow!("Blowfish decryption failed: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::{BlockEncryptMut, KeyIvInit};

    #[test]
    fn blowfish_roundtrip() {
        type Encryptor = cbc::Encryptor<Blowfish>;

        let key = calc_bf_key("4329106072");
        let original: Vec<u8> = (0..2048).map(|i| (i % 251) as u8).collect();
        let mut encrypted = original.clone();

        let encryptor = Encryptor::new_from_slices(&key, &IV).unwrap();
        let length = encrypted.len();
        encryptor
            .encrypt_padded_mut::<NoPadding>(&mut encrypted, length)
            .unwrap();
        assert_ne!(encrypted, original);

        decrypt_block_in_place(&mut encrypted, &key).unwrap();
        assert_eq!(encrypted, original);
    }

    #[test]
    fn key_is_deterministic() {
        assert_eq!(calc_bf_key("123"), calc_bf_key("123"));
        assert_ne!(calc_bf_key("123"), calc_bf_key("124"));
    }

    #[test]
    fn key_matches_python_reference() {
        // python: md5hex = hashlib.md5(b"4329106072").hexdigest().encode()
        // key[i] = md5hex[i] ^ md5hex[i+16] ^ b"g4el58wc0zvf9na1"[i]
        assert_eq!(
            hex::encode(calc_bf_key("4329106072")),
            "3531626d613975316b7b22673c68336b"
        );
    }
}
