//! AccountsService (`org.freedesktop.Accounts`): the people who use this
//! computer. Every change is the service's own polkit action
//! (`org.freedesktop.accounts.*`); passwords go to the service as a crypt(3)
//! hash (yescrypt, as Fedora stores them; SHA-512 where libcrypt has no
//! yescrypt), as the KDE users page sends them, and are held only for the
//! call and never logged. A picture is looked at before the service is asked
//! to copy it ([`check_picture`]).

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::error::clean;
use crate::{Error, ErrorKind};
use std::ffi::{CStr, CString, c_char, c_int, c_ulong};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
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
/// The longest password accepted, in bytes: libcrypt's own limit
/// (`CRYPT_MAX_PASSPHRASE_SIZE` is 512, counting the NUL), above which it
/// hashes nothing. A longer one is refused here, up front, not after the
/// account was created.
pub const MAX_PASSWORD: usize = 511;
/// The fewest characters a new password has: AccountsService stores the hash
/// it is given and checks nothing about the password (that is why the hash is
/// made here), so a password of one letter would be accepted for an
/// administrator. 8 is what PAM's pwquality asks of `passwd` by default.
pub const MIN_PASSWORD: usize = 8;

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

/// What the detail of a `Refused` error from [`Accounts::set_icon`] starts
/// with when the picture itself was the problem; the rest is a sentence for
/// the page.
pub const PICTURE_PREFIX: &str = "picture: ";

/// The largest picture AccountsService copies (it refuses more than 1 MB).
pub const MAX_PICTURE: u64 = 1024 * 1024;

/// The kind of picture file `head` (the first bytes of a file) is, when it is
/// one AccountsService and the sign-in screen can show: PNG, JPEG, GIF or
/// WebP, by what the file holds, not what it is called. Not SVG: a picture
/// the login screen draws should not be a document with a mind of its own.
pub fn picture_kind(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("gif")
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Looks at the file the person chose before AccountsService is asked to
/// copy it as their picture, and returns the path to give it: the file with
/// links followed (the service then opens what was looked at, not a link that
/// could be pointed elsewhere meanwhile). It must be a regular file, not
/// empty, at most [`MAX_PICTURE`], and a PNG, JPEG, GIF or WebP by its
/// contents. The service still does its own checks as the user the picture is
/// for; these tell the person why, and keep a link to a device, a pipe or a
/// secret from ever being handed on. The reason is a sentence for the page.
pub fn check_picture(file: &str) -> Result<String, &'static str> {
    if !valid_icon_path(file) {
        return Err("That isn't a picture file.");
    }
    let resolved = std::fs::canonicalize(file).map_err(|_| "Settings can't find that file.")?;
    let resolved = resolved
        .to_str()
        .ok_or("Settings can't use that file's name.")?
        .to_string();
    if resolved.chars().any(char::is_control) || resolved.len() > 4096 {
        return Err("Settings can't use that file's name.");
    }
    // No link at the end (it was resolved; one swapped in since is refused),
    // and no waiting on a pipe or a device.
    // The kind of file first, from `stat`, so a link to a terminal or a tape
    // is not even opened; the open below is checked again (`fstat`).
    if !std::fs::metadata(&resolved).is_ok_and(|m| m.is_file()) {
        return Err("That isn't a picture file.");
    }
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC)
        .open(&resolved)
        .map_err(|_| "Settings can't read that file.")?;
    let meta = f.metadata().map_err(|_| "Settings can't read that file.")?;
    if !meta.is_file() {
        return Err("That isn't a picture file.");
    }
    if meta.len() == 0 {
        return Err("That file is empty.");
    }
    if meta.len() > MAX_PICTURE {
        return Err("That picture is bigger than 1 MB. Choose a smaller one.");
    }
    let mut head = [0u8; 12];
    let mut n = 0;
    while n < head.len() {
        match f.read(&mut head[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err("Settings can't read that file."),
        }
    }
    if picture_kind(&head[..n]).is_none() {
        return Err("Choose a PNG, JPEG, WebP or GIF picture.");
    }
    Ok(resolved)
}

#[link(name = "crypt")]
unsafe extern "C" {
    fn crypt(key: *const c_char, salt: *const c_char) -> *const c_char;
    fn crypt_gensalt_rn(
        prefix: *const c_char,
        count: c_ulong,
        rbytes: *const c_char,
        nrbytes: c_int,
        output: *mut c_char,
        output_size: c_int,
    ) -> *mut c_char;
}

/// Overwrites `bytes` in a way the compiler may not drop.
fn wipe(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        // SAFETY: a valid, aligned reference to a byte.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// A crypt(3) setting string (algorithm, cost and a random salt) for `prefix`
/// (`$y$` yescrypt, `$6$` SHA-512), made by libcrypt from random bytes of
/// the kernel's. `None` when libcrypt doesn't have that algorithm.
fn gensalt(prefix: &CStr, count: c_ulong) -> Result<Option<CString>, Error> {
    let mut random = [0u8; 64];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|e| Error::new(ErrorKind::Refused, format!("no random numbers: {e}")))?;
    let mut out = vec![0u8; 192];
    // SAFETY: `random` and `out` are valid for the lengths given and the
    // prefix is NUL-terminated; libcrypt writes a NUL-terminated string to
    // `out` or returns null.
    let made = unsafe {
        crypt_gensalt_rn(
            prefix.as_ptr(),
            count,
            random.as_ptr().cast(),
            random.len() as c_int,
            out.as_mut_ptr().cast(),
            out.len() as c_int,
        )
    };
    wipe(&mut random);
    if made.is_null() {
        return Ok(None);
    }
    // SAFETY: success: `out` holds a NUL-terminated string.
    let setting = unsafe { CStr::from_ptr(out.as_ptr().cast()) }.to_owned();
    Ok(Some(setting))
}

/// crypt(3) is not reentrant.
static CRYPT: Mutex<()> = Mutex::new(());

/// `crypt(key, setting)` as a string; `None` when libcrypt refuses.
fn crypt_with(key: &CStr, setting: &CStr) -> Option<String> {
    let _guard = CRYPT.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: both are NUL-terminated; the result points into libcrypt's
    // buffer, which is copied before the lock is released.
    unsafe {
        let p = crypt(key.as_ptr(), setting.as_ptr());
        if p.is_null() {
            None
        } else {
            Some(CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
}

/// The crypt(3) hash of `password` with a random salt, as AccountsService's
/// `SetPassword` takes it: yescrypt (`$y$`, a memory-hard hash, what Fedora's
/// shadow file holds), else SHA-512 with 100,000 rounds where libcrypt has
/// no yescrypt. The hash is checked by hashing the password again with it
/// before it is returned.
pub fn hash_password(password: &str) -> Result<String, Error> {
    if password.chars().count() < MIN_PASSWORD
        || password.len() > MAX_PASSWORD
        || password.contains('\0')
    {
        return Err(Error::new(ErrorKind::Refused, "not a usable password"));
    }
    let key = CString::new(password)
        .map_err(|_| Error::new(ErrorKind::Refused, "not a usable password"))?;
    // The key is overwritten when done, whichever way this ends.
    struct Wiped(Option<CString>);
    impl Drop for Wiped {
        fn drop(&mut self) {
            if let Some(k) = self.0.take() {
                let mut bytes = k.into_bytes_with_nul();
                wipe(&mut bytes);
                std::hint::black_box(&bytes);
            }
        }
    }
    let key = Wiped(Some(key));
    let key_ref = key.0.as_deref().expect("just set");
    for (prefix, count) in [(c"$y$", 0), (c"$6$", 100_000)] {
        let Some(setting) = gensalt(prefix, count)? else {
            continue;
        };
        let want = prefix.to_str().unwrap_or_default();
        // "*0" and "*1" are crypt's failure answers.
        let Some(hash) = crypt_with(key_ref, &setting).filter(|h| h.starts_with(want)) else {
            continue;
        };
        let again = CString::new(hash.as_str())
            .map_err(|_| Error::new(ErrorKind::Refused, "hashing the password failed"))?;
        if crypt_with(key_ref, &again).as_deref() == Some(hash.as_str()) {
            return Ok(hash);
        }
    }
    Err(Error::new(
        ErrorKind::Refused,
        "hashing the password failed",
    ))
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

    /// Gives the user the picture in `file`, which [`check_picture`] has
    /// looked at first (a `Refused` error whose detail is `picture: ` and the
    /// reason when it won't do).
    pub fn set_icon(&self, uid: u64, file: &str) -> Result<(), Error> {
        let file = check_picture(file)
            .map_err(|why| Error::new(ErrorKind::Refused, format!("{PICTURE_PREFIX}{why}")))?;
        let path = self.path_of(uid)?;
        self.interactive_call(&path, "SetIconFile", &(file.as_str(),))
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
        // Whatever goes wrong from here on, the new account is removed again:
        // there is never one without a password. Its ID is the service's
        // `Uid`, or the end of its object path (`.../User1002`).
        let uid_from_path = || {
            path.as_str()
                .rsplit('/')
                .next()
                .and_then(|n| n.strip_prefix("User"))
                .and_then(|n| n.parse::<u64>().ok())
        };
        let finish = || -> Result<u64, Error> {
            let user = self.user_at(&self.interactive, path.as_str())?;
            let set: Result<Option<()>, zbus::Error> = user.call_with_flags(
                "SetPassword",
                MethodFlags::AllowInteractiveAuth.into(),
                &(hash.as_str(), ""),
            );
            set?;
            user.get_property::<u64>("Uid")
                .ok()
                .or_else(uid_from_path)
                .ok_or_else(|| Error::new(ErrorKind::Refused, "the new user has no ID"))
        };
        match finish() {
            Ok(uid) => Ok(uid),
            Err(e) => {
                if let Some(uid) = self
                    .user_at(&self.interactive, path.as_str())
                    .ok()
                    .and_then(|u| u.get_property::<u64>("Uid").ok())
                    .or_else(uid_from_path)
                {
                    let _ = self.delete_user(uid, true);
                }
                Err(e)
            }
        }
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
    fn password_hashes_are_slow_salted_and_check_out() {
        let a = hash_password("correct horse").expect("hash");
        let b = hash_password("correct horse").expect("hash");
        // yescrypt where libcrypt has it (Fedora's does), else SHA-512.
        assert!(a.starts_with("$y$") || a.starts_with("$6$"), "{a}");
        assert_ne!(a, b, "a new salt each time");
        assert!(!a.contains("correct"));
        // The hash as the setting gives the hash back for the password, and
        // another for any other.
        let right = CString::new("correct horse").unwrap();
        let wrong = CString::new("correct horsf").unwrap();
        let setting = CString::new(a.as_str()).unwrap();
        assert_eq!(crypt_with(&right, &setting).as_deref(), Some(a.as_str()));
        assert_ne!(crypt_with(&wrong, &setting).as_deref(), Some(a.as_str()));
        // A hash AccountsService can write to the shadow file's field: no
        // colon, no whitespace, nothing but crypt(3)'s alphabet and `$`.
        assert!(
            a.bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'$' | b'.' | b'/')),
            "{a}"
        );
        assert!(hash_password("").is_err());
        // Too short is refused (7 characters), the shortest is not (8, counted
        // in characters, not bytes).
        assert!(hash_password("abcdefg").is_err());
        assert!(hash_password("abcdefgh").is_ok());
        assert!(hash_password("äöüäöüä").is_err());
        assert!(hash_password("äöüäöüäö").is_ok());
        assert!(hash_password(&"x".repeat(MAX_PASSWORD + 1)).is_err());
        assert!(hash_password("a\0b").is_err());
        // A long password and one that is not ASCII.
        assert!(hash_password(&"x".repeat(MAX_PASSWORD)).is_ok());
        assert!(hash_password("pässwörd ✓").is_ok());
    }

    #[test]
    fn sha512_stays_available_as_the_fallback() {
        // libcrypt always has it: the setting the fallback uses is good.
        let setting = gensalt(c"$6$", 100_000).expect("random").expect("sha512");
        assert!(setting.to_str().unwrap().starts_with("$6$rounds=100000$"));
        let hash = crypt_with(c"pw", &setting).expect("hash");
        assert!(hash.starts_with("$6$rounds=100000$"), "{hash}");
    }

    /// A scratch folder of this test's own.
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("settings-sys-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> String {
            let p = self.0.join(name);
            std::fs::write(&p, bytes).unwrap();
            p.to_str().unwrap().to_string()
        }
        fn path(&self, name: &str) -> String {
            self.0.join(name).to_str().unwrap().to_string()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    #[test]
    fn picture_kinds_are_told_by_contents() {
        assert_eq!(picture_kind(PNG), Some("png"));
        assert_eq!(picture_kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpeg"));
        assert_eq!(picture_kind(b"GIF89a\x01\0"), Some("gif"));
        assert_eq!(picture_kind(b"GIF87a"), Some("gif"));
        assert_eq!(picture_kind(b"RIFF\x10\0\0\0WEBPVP8 "), Some("webp"));
        for not in [
            &b""[..],
            b"RIFF\x10\0\0\0WAVEfmt ",
            b"RIFF",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            b"<?xml version=\"1.0\"?><svg/>",
            b"\x7fELF\x02\x01\x01",
            b"#!/bin/sh\n",
            b"GIF88a",
            b"\x89PNG\r\n\x1a",
        ] {
            assert_eq!(picture_kind(not), None, "{not:?}");
        }
    }

    #[test]
    fn a_picture_is_checked_before_the_service_is_asked() {
        let dir = Scratch::new("picture");
        let real = std::fs::canonicalize(&dir.0).unwrap();
        // The pictures the service takes, wherever the extension says.
        let png = dir.file("me.png", PNG);
        assert_eq!(
            check_picture(&png).unwrap(),
            real.join("me.png").to_str().unwrap()
        );
        assert!(check_picture(&dir.file("photo.JPG", &[0xFF, 0xD8, 0xFF, 0xE0])).is_ok());
        assert!(check_picture(&dir.file("a.webp", b"RIFF\0\0\0\0WEBPVP8 ")).is_ok());
        assert!(check_picture(&dir.file("no-extension.png", b"GIF89a....")).is_ok());

        // Not a picture, whatever it is called.
        for (name, bytes) in [
            ("text.png", &b"hello there\n"[..]),
            ("script.png", b"#!/bin/sh\nrm -rf ~\n"),
            ("elf.jpg", b"\x7fELF\x02\x01\x01\0"),
            ("vector.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            ("empty.png", b""),
        ] {
            let path = dir.file(name, bytes);
            assert!(check_picture(&path).is_err(), "{name}");
        }
        assert_eq!(
            check_picture(&dir.file("empty2.png", b"")).unwrap_err(),
            "That file is empty."
        );
        assert!(check_picture(&dir.path("missing.png")).is_err());
        // Not a path the service may be given at all.
        for bad in [
            "",
            "me.png",
            "../me.png",
            "/a/../b.png",
            "/tmp/a\nb.png",
            "/etc/shadow",
        ] {
            assert!(check_picture(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_picture_is_at_most_a_megabyte() {
        let dir = Scratch::new("picture-size");
        let exact = dir.path("exact.png");
        let mut bytes = PNG.to_vec();
        bytes.resize(MAX_PICTURE as usize, 0);
        std::fs::write(&exact, &bytes).unwrap();
        assert!(check_picture(&exact).is_ok());
        bytes.push(0);
        let over = dir.path("over.png");
        std::fs::write(&over, &bytes).unwrap();
        assert_eq!(
            check_picture(&over).unwrap_err(),
            "That picture is bigger than 1 MB. Choose a smaller one."
        );
        // A huge sparse file is looked at by its size, never read.
        let huge = dir.path("huge.png");
        let f = std::fs::File::create(&huge).unwrap();
        if f.set_len(8 << 30).is_ok() {
            assert!(check_picture(&huge).is_err());
        }
    }

    #[test]
    fn links_are_followed_to_the_file_that_was_checked() {
        use std::os::unix::fs::symlink;
        let dir = Scratch::new("picture-links");
        let real = std::fs::canonicalize(&dir.0).unwrap();
        let png = dir.file("real.png", PNG);
        // A link to a picture is fine, and the path handed on is the picture's.
        let link = dir.path("link.png");
        symlink(&png, &link).unwrap();
        assert_eq!(
            check_picture(&link).unwrap(),
            real.join("real.png").to_str().unwrap()
        );
        // A link that points at something else is judged by that: a secret
        // named like a picture is not one...
        let secret = dir.file("secret", b"root:$6$salt$hash:19000::::::\n");
        let sneaky = dir.path("sneaky.png");
        symlink(&secret, &sneaky).unwrap();
        assert!(check_picture(&sneaky).is_err());
        let shadow = dir.path("shadow.png");
        symlink("/etc/shadow", &shadow).unwrap();
        assert!(check_picture(&shadow).is_err());
        // ... a link to a folder or a device is not a file ...
        let folder = dir.path("folder.png");
        symlink(&dir.0, &folder).unwrap();
        assert!(check_picture(&folder).is_err());
        let device = dir.path("device.png");
        symlink("/dev/zero", &device).unwrap();
        assert!(check_picture(&device).is_err());
        // ... and one that leads nowhere, or round in a circle, is refused.
        let dangling = dir.path("dangling.png");
        symlink(dir.path("nowhere"), &dangling).unwrap();
        assert!(check_picture(&dangling).is_err());
        let a = dir.path("a.png");
        let b = dir.path("b.png");
        symlink(&b, &a).unwrap();
        symlink(&a, &b).unwrap();
        assert!(check_picture(&a).is_err());
    }

    #[test]
    fn a_pipe_or_a_folder_named_like_a_picture_is_not_waited_for() {
        use std::ffi::CString;
        let dir = Scratch::new("picture-pipe");
        let fifo = dir.path("pipe.png");
        let c = CString::new(fifo.as_str()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let started = std::time::Instant::now();
        assert!(check_picture(&fifo).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        let folder = dir.path("folder.png");
        std::fs::create_dir(&folder).unwrap();
        assert!(check_picture(&folder).is_err());
    }
}
