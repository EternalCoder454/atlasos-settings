//! AccountsService (`org.freedesktop.Accounts`): the people who use this
//! computer. Every change is the service's own polkit action
//! (`org.freedesktop.accounts.*`); passwords go to the service as a crypt(3)
//! hash, as the KDE users page sends them, and are held only for the call
//! and never logged.

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::error::clean;
use crate::{Error, ErrorKind};
use std::ffi::{CStr, CString, c_char};
use std::sync::Mutex;
use zbus::blocking::{Connection, Proxy};
use zbus::proxy::MethodFlags;
use zbus::zvariant::OwnedObjectPath;

const NAME: &str = "org.freedesktop.Accounts";
const PATH: &str = "/org/freedesktop/Accounts";
const USER_IFACE: &str = "org.freedesktop.Accounts.User";

/// Most users read (a computer with more than this lists the first).
pub const MAX_USERS: usize = 64;
/// The longest full name, in characters.
pub const MAX_REAL_NAME: usize = 128;
/// The longest password accepted, in bytes.
pub const MAX_PASSWORD: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub uid: u64,
    /// The sign-in name.
    pub name: String,
    /// The full name; the sign-in name when there is none.
    pub real_name: String,
    /// A picture's path; empty for none.
    pub icon: String,
    pub admin: bool,
    pub locked: bool,
    pub auto_login: bool,
}

/// A sign-in name: what `useradd` takes by default.
pub fn valid_user_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && name.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// A full name: not empty, no control characters, not too long.
pub fn valid_real_name(name: &str) -> bool {
    let t = name.trim();
    !t.is_empty()
        && t.chars().count() <= MAX_REAL_NAME
        && !t.chars().any(|c| c.is_control() || c == ':' || c == ',')
}

/// A sign-in name made from a full name ("Ada Lovelace" is "ada"): lower
/// case ASCII letters and digits of the first word, `user` when none are
/// left.
pub fn suggest_user_name(real_name: &str) -> String {
    let first = real_name.split_whitespace().next().unwrap_or("");
    let mut out: String = first
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(32)
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "user");
        out.truncate(32);
    }
    out
}

/// A picture file AccountsService is asked to use: an absolute path to a
/// picture, nothing odd in the name.
pub fn valid_icon_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    path.starts_with('/')
        && path.len() <= 4096
        && !path.chars().any(char::is_control)
        && !path.split('/').any(|p| p == "..")
        && [".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg"]
            .iter()
            .any(|e| lower.ends_with(e))
}

#[link(name = "crypt")]
unsafe extern "C" {
    fn crypt(key: *const c_char, salt: *const c_char) -> *const c_char;
}

/// crypt(3) is not reentrant.
static CRYPT: Mutex<()> = Mutex::new(());

const SALT_CHARS: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn random_salt() -> Result<String, Error> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| Error::new(ErrorKind::Refused, format!("no random numbers: {e}")))?;
    let salt: String = bytes
        .iter()
        .map(|b| SALT_CHARS[(*b as usize) % SALT_CHARS.len()] as char)
        .collect();
    Ok(format!("$6${salt}"))
}

/// The SHA-512 crypt(3) hash of `password` with a random salt, as
/// AccountsService's `SetPassword` takes it.
pub fn hash_password(password: &str) -> Result<String, Error> {
    if password.is_empty() || password.len() > MAX_PASSWORD || password.contains('\0') {
        return Err(Error::new(ErrorKind::Refused, "not a usable password"));
    }
    let key = CString::new(password)
        .map_err(|_| Error::new(ErrorKind::Refused, "not a usable password"))?;
    let salt =
        CString::new(random_salt()?).map_err(|_| Error::new(ErrorKind::Refused, "bad salt"))?;
    let _guard = CRYPT.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: both are NUL-terminated; the result points into libcrypt's
    // buffer, which is copied before the lock is released.
    let out = unsafe {
        let p = crypt(key.as_ptr(), salt.as_ptr());
        if p.is_null() {
            None
        } else {
            Some(CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    };
    match out {
        // "*0" and "*1" are crypt's failure answers.
        Some(h) if h.starts_with("$6$") => Ok(h),
        _ => Err(Error::new(
            ErrorKind::Refused,
            "hashing the password failed",
        )),
    }
}

pub struct Accounts {
    conn: Connection,
    interactive: Connection,
}

impl Accounts {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Accounts {
            conn: bus.connect()?,
            interactive: bus.connect_with(INTERACTIVE_TIMEOUT)?,
        })
    }

    fn manager<'a>(&self, conn: &'a Connection) -> Result<Proxy<'a>, Error> {
        Ok(Proxy::new(conn, NAME, PATH, "org.freedesktop.Accounts")?)
    }

    fn user_at<'a>(&self, conn: &'a Connection, path: &str) -> Result<Proxy<'a>, Error> {
        Ok(Proxy::new(conn, NAME, path.to_string(), USER_IFACE)?)
    }

    fn read_user(&self, path: &str) -> Result<User, Error> {
        let p = self.user_at(&self.conn, path)?;
        let name: String = p.get_property("UserName")?;
        let real: String = p.get_property("RealName").unwrap_or_default();
        let icon: String = p.get_property("IconFile").unwrap_or_default();
        let account_type: i32 = p.get_property("AccountType").unwrap_or(0);
        let locked: bool = p.get_property("Locked").unwrap_or(false);
        let auto: bool = p.get_property("AutomaticLogin").unwrap_or(false);
        let uid: u64 = p.get_property("Uid")?;
        let name = clean(&name, 64);
        let real = clean(real.trim(), MAX_REAL_NAME);
        Ok(User {
            uid,
            real_name: if real.is_empty() { name.clone() } else { real },
            name,
            icon: if valid_icon_path(&icon) {
                icon
            } else {
                String::new()
            },
            admin: account_type == 1,
            locked,
            auto_login: auto,
        })
    }

    /// The people who have signed in or been added, in the service's order.
    pub fn users(&self) -> Result<Vec<User>, Error> {
        let paths: Vec<OwnedObjectPath> = self.manager(&self.conn)?.call("ListCachedUsers", &())?;
        let mut users = Vec::new();
        for p in paths.iter().take(MAX_USERS) {
            // One user that can't be read is left out, not the whole list.
            match self.read_user(p.as_str()) {
                Ok(u) => users.push(u),
                Err(e) => log::warn!("reading a user: {}", e.detail),
            }
        }
        Ok(users)
    }

    /// The user with this ID, which need not be in `ListCachedUsers`.
    pub fn user(&self, uid: u64) -> Result<User, Error> {
        let path: OwnedObjectPath = self
            .manager(&self.conn)?
            .call("FindUserById", &(i64::try_from(uid).unwrap_or(-1),))?;
        self.read_user(path.as_str())
    }

    fn path_of(&self, uid: u64) -> Result<String, Error> {
        let path: OwnedObjectPath = self
            .manager(&self.conn)?
            .call("FindUserById", &(i64::try_from(uid).unwrap_or(-1),))?;
        Ok(path.as_str().to_string())
    }

    /// A call that may show a polkit prompt: the message asks for one.
    fn interactive_call<B>(&self, path: &str, method: &'static str, body: &B) -> Result<(), Error>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        let p = self.user_at(&self.interactive, path)?;
        p.call_with_flags::<_, _, ()>(method, MethodFlags::AllowInteractiveAuth.into(), body)?;
        Ok(())
    }

    pub fn set_real_name(&self, uid: u64, name: &str) -> Result<(), Error> {
        if !valid_real_name(name) {
            return Err(Error::new(ErrorKind::Refused, "not a usable name"));
        }
        let path = self.path_of(uid)?;
        self.interactive_call(&path, "SetRealName", &(name.trim(),))
    }

    pub fn set_icon(&self, uid: u64, file: &str) -> Result<(), Error> {
        if !valid_icon_path(file) {
            return Err(Error::new(ErrorKind::Refused, "not a picture file"));
        }
        let path = self.path_of(uid)?;
        self.interactive_call(&path, "SetIconFile", &(file,))
    }

    pub fn set_admin(&self, uid: u64, admin: bool) -> Result<(), Error> {
        let path = self.path_of(uid)?;
        self.interactive_call(&path, "SetAccountType", &(i32::from(admin),))
    }

    pub fn set_auto_login(&self, uid: u64, on: bool) -> Result<(), Error> {
        let path = self.path_of(uid)?;
        self.interactive_call(&path, "SetAutomaticLogin", &(on,))
    }

    /// Sets the password. The hash, not the password, goes on the bus.
    pub fn set_password(&self, uid: u64, password: &str) -> Result<(), Error> {
        let hash = hash_password(password)?;
        let path = self.path_of(uid)?;
        self.interactive_call(&path, "SetPassword", &(hash.as_str(), ""))
    }

    /// Adds a user (a standard one, or an administrator) and gives them a
    /// password. When the password can't be set the new user is removed
    /// again, so there is no account without one. Returns the new ID.
    pub fn create_user(
        &self,
        name: &str,
        real_name: &str,
        admin: bool,
        password: &str,
    ) -> Result<u64, Error> {
        if !valid_user_name(name) || !valid_real_name(real_name) {
            return Err(Error::new(ErrorKind::Refused, "not a usable name"));
        }
        // Checked before anything is created.
        let hash = hash_password(password)?;
        let m = self.manager(&self.interactive)?;
        let flags = MethodFlags::AllowInteractiveAuth.into();
        let path: OwnedObjectPath = m
            .call_with_flags(
                "CreateUser",
                flags,
                &(name, real_name.trim(), i32::from(admin)),
            )?
            .ok_or_else(|| Error::new(ErrorKind::Refused, "no answer to CreateUser"))?;
        let user = self.user_at(&self.interactive, path.as_str())?;
        let uid: u64 = user.get_property("Uid")?;
        let set: Result<Option<()>, zbus::Error> = user.call_with_flags(
            "SetPassword",
            MethodFlags::AllowInteractiveAuth.into(),
            &(hash.as_str(), ""),
        );
        if let Err(e) = set {
            let _ = self.delete_user(uid, true);
            return Err(e.into());
        }
        Ok(uid)
    }

    /// Removes a user, and their files when `remove_files`.
    pub fn delete_user(&self, uid: u64, remove_files: bool) -> Result<(), Error> {
        let m = self.manager(&self.interactive)?;
        m.call_with_flags::<_, _, ()>(
            "DeleteUser",
            MethodFlags::AllowInteractiveAuth.into(),
            &(i64::try_from(uid).unwrap_or(-1), remove_files),
        )?;
        Ok(())
    }
}

/// The ID of the user running this program.
pub fn current_uid() -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self")
        .map(|m| u64::from(m.uid()))
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names() {
        for ok in ["ada", "_x", "a-b_c9", &"a".repeat(32)] {
            assert!(valid_user_name(ok), "{ok}");
        }
        for bad in [
            "",
            "Ada",
            "9a",
            "a b",
            "a:b",
            "-a",
            &"a".repeat(33),
            "é",
            "a\n",
        ] {
            assert!(!valid_user_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn real_names() {
        assert!(valid_real_name("Ada Lovelace"));
        assert!(valid_real_name("  Zoë  "));
        for bad in ["", "   ", "a\nb", "a:b", "a,b", "\u{1b}[0m"] {
            assert!(!valid_real_name(bad), "{bad:?}");
        }
        assert!(!valid_real_name(&"x".repeat(MAX_REAL_NAME + 1)));
    }

    #[test]
    fn suggested_names_are_valid() {
        assert_eq!(suggest_user_name("Ada Lovelace"), "ada");
        assert_eq!(suggest_user_name("  Zoë Smith"), "zo");
        assert_eq!(suggest_user_name("9 Lives"), "user9");
        assert_eq!(suggest_user_name("日本語"), "user");
        for n in ["Ada", "O'Brien", "!!!", "", "x".repeat(80).as_str()] {
            assert!(valid_user_name(&suggest_user_name(n)), "{n}");
        }
    }

    #[test]
    fn icon_paths() {
        assert!(valid_icon_path("/home/ada/Pictures/me.png"));
        assert!(valid_icon_path("/usr/share/pixmaps/faces/Cat.JPG"));
        for bad in [
            "",
            "me.png",
            "/home/ada/../root/x.png",
            "/etc/shadow",
            "/home/a\nb.png",
        ] {
            assert!(!valid_icon_path(bad), "{bad:?}");
        }
    }

    #[test]
    fn password_hashes_are_sha512_crypt() {
        let a = hash_password("correct horse").expect("hash");
        let b = hash_password("correct horse").expect("hash");
        assert!(a.starts_with("$6$"), "{a}");
        // 86 characters of hash after "$6$<16 salt>$".
        assert_eq!(a.len(), 3 + 16 + 1 + 86, "{a}");
        assert_ne!(a, b, "a new salt each time");
        assert!(!a.contains("correct"));
        assert!(hash_password("").is_err());
        assert!(hash_password(&"x".repeat(MAX_PASSWORD + 1)).is_err());
        assert!(hash_password("a\0b").is_err());
    }
}
