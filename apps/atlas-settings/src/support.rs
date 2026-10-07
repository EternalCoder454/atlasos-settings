//! What the page backends that talk to the system share: which bus, and a
//! source of random passwords.

use settings_sys::Bus;
use std::io::Read;

/// The system bus. A debug build started with `ATLAS_SETTINGS_TEST_BUS`
/// (a bus address) talks to that bus instead: how smoke runs show a page
/// with python-dbusmock's services behind it. Release builds ignore it.
pub fn system() -> Bus {
    if cfg!(debug_assertions)
        && let Ok(address) = std::env::var("ATLAS_SETTINGS_TEST_BUS")
        && !address.is_empty()
    {
        return Bus::Address(address);
    }
    Bus::System
}

/// A random password of `len` letters and digits, without the ones that
/// look alike (0 O 1 l I): easy to read out to someone.
pub fn random_password(len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    // Bytes above this would make some characters likelier than others.
    let limit = 256 - 256 % ALPHABET.len();
    let mut out = String::new();
    let Ok(mut random) = std::fs::File::open("/dev/urandom") else {
        return out;
    };
    let mut byte = [0u8; 1];
    while out.len() < len {
        if random.read_exact(&mut byte).is_err() {
            return String::new();
        }
        if usize::from(byte[0]) < limit {
            out.push(char::from(ALPHABET[usize::from(byte[0]) % ALPHABET.len()]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_passwords_are_long_enough_and_readable() {
        let a = random_password(12);
        let b = random_password(12);
        assert_eq!(a.len(), 12);
        assert_ne!(a, b);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() && !"0O1lI".contains(c))
        );
        // Good for a hotspot: 8 to 63 characters.
        assert!(settings_sys::network::valid_password(
            settings_sys::network::Security::Personal,
            &a
        ));
    }
}
