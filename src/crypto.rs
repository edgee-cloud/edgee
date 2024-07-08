use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};

use crate::config;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

pub fn encrypt(text: &str) -> String {
    let sec = &config::get().security;
    let data = Aes128CbcEnc::new(sec.aes_key.as_bytes().into(), sec.aes_iv.as_bytes().into())
        .encrypt_padded_vec_mut::<Pkcs7>(text.as_bytes());
    hex::encode(data)
}

pub fn decrypt(data: &str) -> Option<String> {
    let sec = &config::get().security;
    hex::decode(data).ok().and_then(|text| {
        Aes128CbcDec::new(sec.aes_key.as_bytes().into(), sec.aes_iv.as_bytes().into())
            .decrypt_padded_vec_mut::<Pkcs7>(&text)
            .map(|data| String::from_utf8_lossy(&data).to_string())
            .ok()
    })
}
