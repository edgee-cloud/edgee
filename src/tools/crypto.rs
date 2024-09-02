use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use crate::config;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// Encrypts a given text using AES-128-CBC encryption with PKCS7 padding.
///
/// # Arguments
///
/// * `text` - A string slice that holds the text to be encrypted.
///
/// # Returns
///
/// * `Ok(String)` - A `Result` containing the encrypted text as a hexadecimal string if successful.
/// * `Err(&'static str)` - A `Result` containing an error message if the input text is empty.
///
/// # Errors
///
/// This function will return an error if the input text is empty.
///
/// # Example
///
/// ```
/// let encrypted = encrypt("Hello, world!").unwrap();
/// println!("Encrypted text: {}", encrypted);
/// ```
pub fn encrypt(text: &str) -> Result<String, &'static str> {
    if text.is_empty() {
        return Err("Empty string".into());
    }
    let sec = &config::get().security;
    let data = Aes128CbcEnc::new(sec.aes_key.as_bytes().into(), sec.aes_iv.as_bytes().into())
        .encrypt_padded_vec_mut::<Pkcs7>(text.as_bytes());
    Ok(hex::encode(data))
}

/// Decrypts a given text using AES-128-CBC decryption with PKCS7 padding.
///
/// # Arguments
///
/// * `text` - A string slice that holds the text to be decrypted.
///
/// # Returns
///
/// * `Ok(String)` - A `Result` containing the decrypted text as a UTF-8 string if successful.
/// * `Err(&'static str)` - A `Result` containing an error message if the input text is empty, invalid hex, or decryption fails.
///
/// # Errors
///
/// This function will return an error if the input text is empty, if the input text is not valid hexadecimal, or if decryption fails.
///
/// # Example
///
/// ```
/// let decrypted = decrypt("some_encrypted_text").unwrap();
/// println!("Decrypted text: {}", decrypted);
/// ```
pub fn decrypt(text: &str) -> Result<String, &'static str>  {
    if text.is_empty() {
        return Err("Empty string".into());
    }

    let hex = hex::decode(text);
    if hex.is_err() {
        return Err("Invalid hex".into());
    }
    let sec = &config::get().security;
    let res = Aes128CbcDec::new(sec.aes_key.as_bytes().into(), sec.aes_iv.as_bytes().into())
        .decrypt_padded_vec_mut::<Pkcs7>(&hex.unwrap());
    if res.is_err() {
        return Err("Failed to decrypt".into());
    }
    Ok(String::from_utf8(res.unwrap()).unwrap())
}
