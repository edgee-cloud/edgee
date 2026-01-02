use crate::config;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};

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
/// ```ignore
/// let encrypted = encrypt("Hello, world!").unwrap();
/// println!("Encrypted text: {}", encrypted);
/// ```
pub fn encrypt(text: &str) -> Result<String, &'static str> {
    if text.is_empty() {
        return Err("Empty string");
    }
    let sec = &config::get().compute;
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
/// ```ignore
/// let decrypted = decrypt("some_encrypted_text").unwrap();
/// println!("Decrypted text: {}", decrypted);
/// ```
pub fn decrypt(text: &str) -> Result<String, &'static str> {
    if text.is_empty() {
        return Err("Empty string");
    }

    let hex = hex::decode(text);
    if hex.is_err() {
        return Err("Invalid hex");
    }
    let sec = &config::get().compute;
    let res = Aes128CbcDec::new(sec.aes_key.as_bytes().into(), sec.aes_iv.as_bytes().into())
        .decrypt_padded_vec_mut::<Pkcs7>(&hex.unwrap());
    if res.is_err() {
        return Err("Failed to decrypt");
    }
    Ok(String::from_utf8(res.unwrap()).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::init_test_config;
    use pretty_assertions::assert_eq;

    #[test]
    fn decrypt_valid_text() {
        init_test_config();
        let encrypted_text = encrypt("a text to encrypt and decrypt");
        assert!(encrypted_text.is_ok());
        let decrypted_text = decrypt(encrypted_text.unwrap().as_str());
        assert!(decrypted_text.is_ok());

        assert_eq!(
            decrypted_text.unwrap().as_str(),
            "a text to encrypt and decrypt"
        );
    }

    #[test]
    fn encrypt_empty_fails() {
        init_test_config();
        let encrypted_text = encrypt("");
        assert!(encrypted_text.is_err());
    }

    #[test]
    fn decrypt_empty_text() {
        let result = decrypt("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Empty string");
    }

    #[test]
    fn decrypt_invalid_hex() {
        let result = decrypt("invalid_hex");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid hex");
    }

    #[test]
    fn decrypt_failed_decryption() {
        init_test_config();
        let result = decrypt("08b64faae3de3b5c6e17b3f2c45be4517030d18a4bfdefbff604e87294bb2d31");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Failed to decrypt");
    }
}
