use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};

const SIZES: [i32; 24] =
    [23, 37, 52, 65, 72, 88, 92, 101, 134, 141, 144, 167, 168, 181, 195, 204, 231, 235, 256, 283, 301, 303, 310, 322];

const ORIGINAL: [char; 38] =
    ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '.'];
const CORRESPONDENCE: [[char; 38]; 24] = [
    ['k', 'd', 'g', 'a', 'j', 'e', 'p', '2', '5', 'y', 't', 'm', 'n', 'w', 'r', 'z', '7', 'x', '3', '8', 'b', '6', 'c', 'u', 'q', 'i', 's', '4', '1', '-', 'v', 'l', '9', 'h', 'o', 'f', '.', '0'],
    ['b', 's', 'y', 'f', '1', 'x', '5', 'v', '3', '.', '4', 'e', 'i', '6', 'q', 'n', 'u', '8', 'm', 'c', '0', 'd', 't', '7', 'l', 'z', 'k', 'a', '-', '2', 'r', 'o', 'j', 'w', '9', 'g', 'h', 'p'],
    ['-', 'x', 's', 'z', 'm', '6', '2', '8', 'f', 'i', 'y', 'o', 'b', '5', 'e', 'r', 'p', 'q', 'n', 'h', 'u', 'c', '0', 'w', 'd', '3', 'g', 'a', '4', '1', 't', 'v', 'k', '7', 'j', '.', '9', 'l'],
    ['3', 'l', 'r', 'a', 'i', 'j', 'q', 'y', 'c', 'b', 'f', 'n', 'e', 'v', '2', '6', '4', 'u', '7', 'o', '5', '9', 'z', 'k', 's', '.', 'd', 'p', 'w', '0', '8', 'g', '1', 'x', '-', 'm', 'h', 't'],
    ['n', 'r', 'm', '4', '3', '7', 'g', 'a', 'u', 'o', 'p', '9', 'i', 'd', 's', 'j', 'v', 'w', '8', 'f', '5', 'x', '6', 'k', 'y', 'z', 'q', '.', 'l', '1', '2', 'e', '0', 'h', 't', 'b', '-', 'c'],
    ['j', '-', 'y', 'i', '7', 'c', '4', '9', 'u', 'f', '8', 'z', 'd', 'q', '3', 'r', 'e', 'o', '6', 'h', 'v', 'k', 'b', '1', 'g', 'm', '.', 'n', 'l', 't', 'w', 'x', 'p', 's', '0', '2', 'a', '5'],
    ['o', 'p', 'g', 'e', 'u', '9', 'f', 't', 'a', 'q', 'b', 'r', 'x', '2', '8', '1', 'n', 'c', '6', 'h', 'l', '5', '-', 's', 'z', '3', '7', 'v', 'y', '4', '.', 'm', 'd', 'i', 'k', 'w', 'j', '0'],
    ['w', 'g', 'i', 'm', '3', 'r', '2', 's', 'a', 'b', '8', 'c', '5', 'h', 'e', 'y', '9', 'p', 'k', 'd', 'l', 'j', 'q', 'u', '0', 'v', '.', '-', 't', '6', 'z', 'x', 'n', '7', 'f', '4', 'o', '1'],
    ['1', '2', 'k', 'e', 'l', 'c', 'x', 'z', 's', '-', 'a', 'y', '8', '9', '5', 'v', 'n', '6', 'h', '0', '3', 'j', 'i', '7', 'g', 'q', '4', 'u', 'd', 'p', '.', 'o', 'r', 'w', 'b', 't', 'f', 'm'],
    ['5', '3', 'g', 'f', 'p', 'u', 'k', 'w', '8', 'e', '2', 'y', 'l', 'm', 'j', 's', 'b', '7', '4', 't', 'x', 'i', 'd', '0', 'z', '-', 'h', '6', 'o', '.', 'c', '9', 'n', '1', 'q', 'a', 'v', 'r'],
    ['1', 'k', 'e', '5', '9', 'x', 'h', 'g', '2', 'v', 'j', 'm', 'f', '4', '.', 'p', 'b', 'q', '7', 'l', 'r', 'i', 'a', '8', 'y', 'z', 'c', '0', 'n', 't', '3', '6', '-', 'd', 'o', 'w', 's', 'u'],
    ['8', 'v', 'b', 'q', 'k', '.', 'y', 'a', '5', 'c', 'n', 'g', '2', '0', 'd', 'j', 'l', '9', 'm', 'e', 'r', 'i', '3', 'x', '6', 's', 'w', '4', 'h', 'o', 'f', 't', '-', '1', 'u', 'p', 'z', '7'],
    ['w', '0', '6', 'k', 'j', '3', 'c', 'b', 't', '7', 'o', '-', 'e', '5', '9', 'd', 'n', 'y', 'x', 'v', 'q', 'i', 'z', 'l', 'f', 'a', '4', 'p', '8', 'h', '.', 'g', '1', 'u', '2', 'm', 's', 'r'],
    ['7', 'x', 'n', 'l', 'r', 'z', 'u', 'a', 'v', 'e', 'd', '1', 'y', '2', 'c', '3', '4', '.', 'k', 'h', 't', 'i', 'f', '6', '-', 'j', 'w', 'o', '0', 'm', '8', 'g', 'b', 's', 'q', 'p', '9', '5'],
    ['d', '1', 's', 'g', '3', '6', 'v', 'n', 'f', 'q', 'w', 'b', '0', 'k', 'c', '5', 'e', '2', 'h', 'p', 'i', 'y', 'j', '4', 't', '9', 'm', 'u', '7', '.', '8', 'r', 'o', 'a', 'x', 'z', '-', 'l'],
    ['c', '2', '7', 'n', 'z', 'm', 'x', 'h', 'i', 'v', '0', '.', 'u', 'g', 'l', '9', 'w', 'b', 'p', 'y', 'e', 'o', '5', 'f', '-', '8', '4', '1', 'q', 'k', 'a', 'd', 's', '3', 'j', 'r', 't', '6'],
    ['2', '0', 'q', '5', 'i', 't', 'h', '9', 'l', 'd', 'u', '6', '-', 'z', 'v', '7', 'x', 'w', 'g', 'c', 'a', '3', 'r', 'e', 'k', 'm', '8', 'b', 'n', '.', 'y', 's', '4', 'j', 'o', 'f', 'p', '1'],
    ['2', 'y', '1', 'z', 'p', '6', 'm', 'u', 'v', 'k', '-', 'd', 'f', 'i', 'g', 'n', 'l', '3', 'q', 'e', 'o', 'r', '0', 'j', '5', 'w', 's', 'h', 'c', 't', 'a', '.', '8', '9', '4', 'b', '7', 'x'],
    ['6', 'e', 'j', 'y', '2', 'b', 'g', '8', 'l', '9', 'z', 'w', 's', 'i', 'h', 'x', 'v', 'm', 'a', '1', '7', '5', 'u', '.', 'r', 'n', '3', '-', 'k', '0', 'c', 'd', 't', 'q', 'p', 'o', 'f', '4'],
    ['w', 'y', 's', '8', 'm', 'b', 'r', 'p', '0', 'k', '4', '9', 'f', 'h', '5', 'x', 'c', 'd', 't', 'i', 'g', 'e', 'q', '-', '3', 'o', 'u', 'a', 'z', '2', '.', 'n', 'v', 'l', 'j', '1', '6', '7'],
    ['8', 's', 'm', '1', '7', 'd', 'y', '0', 'p', 't', '.', '3', 'u', 'q', 'j', 'w', '-', '4', 'x', 'e', 'a', '9', 'o', 'c', 'r', 'h', 'g', 'k', 'v', '6', 'n', '2', 'f', 'z', 'i', 'l', 'b', '5'],
    ['j', 'm', 'k', '9', '4', 'g', 'q', 'd', 'x', 'w', 'y', 'v', '3', 'b', 'c', 'f', '5', '6', '2', 'p', 'e', 'a', 'n', 'i', 't', 'z', '-', '1', 'o', 'l', '8', 'h', 'r', '.', '7', '0', 'u', 's'],
    ['9', '4', 'g', 'z', 'r', '-', 'u', 'o', '.', 'x', '7', 'd', 'w', '3', 't', 'n', 's', 'f', 'l', '1', '2', 'k', 'y', 'i', '0', '5', 'c', 'p', 'j', 'b', 'm', '6', 'a', 'v', '8', 'e', 'h', 'q'],
    ['n', '6', 'o', 'k', 'l', 't', '3', 'a', 'f', 'y', '5', '7', 'm', 'q', 'p', '.', '1', 'e', 'd', 'c', '-', 'u', '4', 's', 'g', 'v', '8', 'r', '0', 'b', '2', 'x', 'z', 'j', '9', 'w', 'i', 'h'],
];


/// This function encrypts a given string using a randomly selected correspondence array.
///
/// # Arguments
///
/// * `input` - A string slice that holds the input string to be encrypted.
///
/// # Returns
///
/// * A String that represents the encrypted string.
///
/// # Encryption Process
///
/// The function encrypts the string by replacing each character in the input string with a corresponding character from a randomly selected correspondence array (CORRESPONDENCE).
/// The specific array used for replacement is determined by a random number between 0 and 23.
///
/// If a character in the input string does not exist in the original array (ORIGINAL), it remains unchanged in the encrypted string.
fn encrypt_string(input: &str) -> String {
    // Create a mutable copy of the input string
    let mut result = input.to_string();

    // pick a random number between 0 and 23
    let pick = thread_rng().gen_range(0..24);

    // For each character in the string
    result = result.chars().map(|c| {
        // Find the index of the character in the original array
        let index = ORIGINAL.iter().position(|&x| x == c);

        // If the character exists in the original array, replace it with the corresponding character from the CORRESPONDENCE array
        // If the character does not exist in the original array, leave it unchanged
        if index.is_none() {
            c
        } else {
            CORRESPONDENCE[pick][index.unwrap()]
        }
    }).collect();

    // Return the encrypted string
    result
}

/// This function decrypts a given string based on a correspondence index.
///
/// # Arguments
///
/// * `input` - A string slice that holds the input string to be decrypted.
/// * `correspondence_index` - An usize that represents the index of the correspondence array to be used for decryption.
///
/// # Returns
///
/// * A String that represents the decrypted string.
///
/// # Decryption Process
///
/// The function decrypts the string by replacing each character in the input string with a corresponding character from the original array (ORIGINAL).
/// The specific array used for replacement is determined by the correspondence index.
///
/// If a character in the input string does not exist in the correspondence array (CORRESPONDENCE), it remains unchanged in the decrypted string.
#[allow(dead_code)]
fn decrypt_string(input: &str, correspondence_index: usize) -> String {
    // Create a mutable copy of the input string
    let mut result = input.to_string();

    // For each character in the string
    result = result.chars().map(|c| {
        // Find the index of the character in the correspondence array
        let index = CORRESPONDENCE[correspondence_index].iter().position(|&x| x == c);

        // If the character exists in the correspondence array, replace it with the corresponding character from the ORIGINAL array
        // If the character does not exist in the correspondence array, leave it unchanged
        if index.is_none() {
            c
        } else {
            ORIGINAL[index.unwrap()]
        }
    }).collect();

    // Return the decrypted string
    result
}

/// This function calculates the possible sizes of the path based on the hostname.
///
/// # Arguments
///
/// * `hostname` - A string slice that holds the hostname.
///
/// # Returns
///
/// * A Vector of i32 that represents the possible sizes of the path.
///
/// # Process
///
/// The function first retrieves the sizes from the SIZES constant and the length of the hostname.
/// It then calculates the possible sizes of the path by adding the length of the hostname and the length of the first slash to each size in the SIZES constant.
/// The calculated sizes are stored in a new vector.
#[allow(dead_code)]
fn guess_path_sizes(hostname: &str) -> Vec<i32> {
    let sizes = SIZES;
    let hostname_len = hostname.len() as i32;

    let mut merged: Vec<i32> = vec![];
    for size in sizes.iter() {
        // add the hostname length and the first slash length to the sizes
        merged.push(size + hostname_len + 1);
    }
    merged
}

/// This function generates a random string of a given length.
///
/// # Arguments
///
/// * `len` - An usize that represents the length of the string to be generated.
///
/// # Returns
///
/// * A String that represents the generated random string.
///
/// # Process
///
/// The function first defines a character set that excludes the characters 'E', 'd', 'g', and 'e'.
/// It then creates a random number generator.
/// The function generates the random string by choosing a character from the character set for each position in the string.
/// The chosen character is then converted to a char and added to the string.
/// The process is repeated until the string reaches the desired length.
fn generate_random_string(len: usize) -> String {
    // string in which it is impossible to find Edgee
    let charset: &[u8] = b"abcfhijklmnopqrstuvwxyzABCDFGHIJKLMNOPQRSTUVWXYZ0123456789-_.";
    let mut rng = rand::thread_rng();
    let random_string: String = (0..len).map(|_| *charset.choose(&mut rng).unwrap() as char).collect();

    random_string
}

/// This function generates a path for a given hostname.
///
/// # Arguments
///
/// * `hostname` - A string slice that holds the hostname.
///
/// # Returns
///
/// * A String that represents the generated path.
///
/// # Process
///
/// The function first creates a random number generator and generates a random string of a size chosen from the SIZES constant.
/// It then encrypts the hostname.
///
/// The function then generates the path by merging the random string and the encrypted hostname.
/// If the total length of the random string and the encrypted hostname is greater than or equal to twice the length of the encrypted hostname,
/// each character of the encrypted hostname is inserted after each character of the random string.
/// If the total length is less than twice the length of the encrypted hostname,
/// the first (total length - length of the encrypted hostname) characters of the encrypted hostname are inserted after each character of the random string,
/// and the remaining characters of the encrypted hostname are appended to the end of the random string.
///
/// The function returns the generated path, which starts with a slash.
pub fn generate(hostname: &str) -> String {
    // Create a random number generator
    let mut rng = thread_rng();

    // generate random string
    let size = SIZES.choose(&mut rng).unwrap().to_owned();
    let random_string: String = generate_random_string(size as usize);
    let random_string_len = random_string.len();

    // encrypt the hostname
    let encrypted_hostname = encrypt_string(hostname);
    let encrypted_hostname_len = encrypted_hostname.len();

    let total_len = random_string_len + encrypted_hostname_len;

    // now, we have to generate the path merging the random_string and the encrypted_hostname
    // if total_len >= encrypted_hostname_len * 2 , we put each letter of the encrypted_hostname after each letter of the random_string
    // if total_len < encrypted_hostname_len * 2, we put the first (total_len - encrypted_hostname_len) letters of the encrypted_hostname after each letter of the random_string
    // then we add the last letters of the encrypted_hostname to the end of the random_string
    let mut result = String::new();
    result.push("/".parse().unwrap());

    if total_len >= encrypted_hostname_len * 2 {
        let mut encrypted_hostname_iter = encrypted_hostname.chars();
        for c in random_string.chars() {
            result.push(c);
            if let Some(encrypted_char) = encrypted_hostname_iter.next() {
                result.push(encrypted_char);
            }
        }
    } else {
        let mut random_string_iter = random_string.chars();
        for c in encrypted_hostname.chars() {
            if let Some(encrypted_char) = random_string_iter.next() {
                result.push(encrypted_char);
            }
            result.push(c);
        }
    }

    result
}

/// This function validates a given hostname and path.
///
/// # Arguments
///
/// * `hostname` - A string slice that holds the hostname.
/// * `path` - A string slice that holds the path.
///
/// # Returns
///
/// * A boolean that indicates whether the given hostname and path are valid.
///
/// # Validation Process
///
/// The function first checks if the length of the path corresponds to a generated path for the given hostname.
/// If it does not, the function returns false.
///
/// The function then removes the first slash of the path and calculates the lengths of the new path and the hostname.
///
/// The function extracts the encrypted hostname from the path by iterating over the new path and taking every second character if possible.
/// If the total length of the new path is greater than or equal to twice the length of the hostname, it only takes the second character of each pair.
/// If the total length is less than twice the length of the hostname, it takes the second character of the first (total length - length of the hostname) pairs and the last characters.
///
/// The function then decrypts the encrypted hostname using each correspondence array in turn.
/// If the decrypted hostname matches the given hostname, the function returns true.
/// If none of the decrypted hostnames match the given hostname, the function returns false.
#[allow(dead_code)]
pub fn validate(hostname: &str, path: &str) -> bool {
    // check if the path length corresponds to a generated path
    if !guess_path_sizes(hostname).contains(&(path.len() as i32)) {
        return false;
    }

    // remove the first slash of the path
    let new_path = &path[1..];
    let hostname_len = hostname.len();
    let new_path_len = new_path.len();

    // extract the encrypted hostname from the path
    // iterate over the new_path and take every second character if possible
    // if new_path_len >= (hostname_len * 2), we only take the second character of each pair
    // if new_path_len < (hostname_len * 2), we take the second character of the first (new_path_len - hostname_len) pairs and the last characters
    let mut encrypted_hostname = String::new();
    let mut new_path_iter = new_path.chars();

    if new_path_len < (hostname_len * 2) {
        for _ in 0..(new_path_len - hostname_len) {
            if let Some(_) = new_path_iter.next() {
                if let Some(encrypted_char) = new_path_iter.next() {
                    encrypted_hostname.push(encrypted_char);
                }
            }
        }
        // add the last characters
        for _ in 0..(hostname_len - (new_path_len - hostname_len)) {
            if let Some(encrypted_char) = new_path_iter.next() {
                encrypted_hostname.push(encrypted_char);
            }
        }
    } else {
        for _ in 0..hostname_len {
            if let Some(_) = new_path_iter.next() {
                if let Some(encrypted_char) = new_path_iter.next() {
                    encrypted_hostname.push(encrypted_char);
                }
            }
        }
    }

    // decrypt the encrypted hostname
    for correspondence_index in 0..CORRESPONDENCE.len() {
        let decrypted_hostname = decrypt_string(&encrypted_hostname, correspondence_index);
        if decrypted_hostname == hostname {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_string_with_valid_input() {
        let input = "hello";
        let encrypted = encrypt_string(input);
        assert_ne!(input, encrypted);
    }

    #[test]
    fn encrypt_string_with_empty_input() {
        let input = "";
        let encrypted = encrypt_string(input);
        assert_eq!(input, encrypted);
    }

    #[test]
    fn decrypt_string_with_empty_input() {
        let input = "";
        let decrypted = decrypt_string(input, 0);
        assert_eq!(input, decrypted);
    }

    #[test]
    fn guess_path_sizes_with_valid_hostname() {
        let hostname = "example.com";
        let sizes = guess_path_sizes(hostname);
        assert!(!sizes.is_empty());
    }

    #[test]
    fn generate_random_string_with_valid_length() {
        let length = 10;
        let random_string = generate_random_string(length);
        assert_eq!(random_string.len(), length);
    }

    #[test]
    fn generate_path_with_valid_hostname() {
        let hostname = "example.com";
        let path = generate(hostname);
        assert!(path.starts_with('/'));
    }

    #[test]
    fn validate_with_correct_hostname_and_path() {
        let hostname = "example.com";
        let path = generate(hostname);
        let is_valid = validate(hostname, &path);
        assert!(is_valid);
    }

    #[test]
    fn validate_with_incorrect_hostname_and_path() {
        let hostname = "example.com";
        let path = generate("different.com");
        let is_valid = validate(hostname, &path);
        assert!(!is_valid);
    }

    #[test]
    fn validate_with_invalid_path_length() {
        let hostname = "example.com";
        let path = "/short";
        let is_valid = validate(hostname, path);
        assert!(!is_valid);
    }
}
