//! The portals' permission store
//! (`org.freedesktop.impl.portal.PermissionStore`, in the user's session):
//! what each app may do through the desktop portals, as flatpak-kcm shows
//! it. Nothing is privileged: the store is the user's own.

use crate::bus::Bus;
use crate::{Error, ErrorKind};
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

const NAME: &str = "org.freedesktop.impl.portal.PermissionStore";
const PATH: &str = "/org/freedesktop/impl/portal/PermissionStore";

/// Something a portal asks the person about, per app: the store's table and
/// ID for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Background,
    Camera,
    Microphone,
    /// Screenshots and screen sharing.
    Screen,
}

impl Permission {
    pub const ALL: [Permission; 4] = [
        Permission::Background,
        Permission::Camera,
        Permission::Microphone,
        Permission::Screen,
    ];

    /// The name Settings' pages use.
    pub fn name(self) -> &'static str {
        match self {
            Permission::Background => "background",
            Permission::Camera => "camera",
            Permission::Microphone => "microphone",
            Permission::Screen => "screen",
        }
    }

    pub fn from_name(n: &str) -> Option<Permission> {
        Self::ALL.into_iter().find(|p| p.name() == n)
    }

    /// (table, ID) in the store.
    fn key(self) -> (&'static str, &'static str) {
        match self {
            Permission::Background => ("background", "background"),
            Permission::Camera => ("devices", "camera"),
            Permission::Microphone => ("devices", "microphone"),
            Permission::Screen => ("screenshot", "screenshot"),
        }
    }
}

/// What an app was told for a permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Nothing is stored, or "ask": the app asks the person when it needs it.
    Ask,
    Yes,
    No,
}

impl Answer {
    pub fn name(self) -> &'static str {
        match self {
            Answer::Ask => "ask",
            Answer::Yes => "yes",
            Answer::No => "no",
        }
    }

    pub fn from_name(n: &str) -> Option<Answer> {
        match n {
            "ask" => Some(Answer::Ask),
            "yes" => Some(Answer::Yes),
            "no" => Some(Answer::No),
            _ => None,
        }
    }
}

/// An application ID as the store knows it: a reverse-DNS name.
pub fn valid_app_id(id: &str) -> bool {
    id.len() <= 255
        && id.contains('.')
        && !id.starts_with('.')
        && !id.ends_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !id.as_bytes()[0].is_ascii_digit()
}

pub struct Portal {
    conn: Connection,
}

fn is_not_found(e: &zbus::Error) -> bool {
    matches!(e, zbus::Error::MethodError(name, _, _)
        if name.as_str().ends_with("NotFound"))
}

impl Portal {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Portal {
            conn: bus.connect()?,
        })
    }

    fn store(&self) -> Result<Proxy<'_>, Error> {
        Ok(Proxy::new(&self.conn, NAME, PATH, NAME)?)
    }

    /// Whether the store answers at all (a session without portals has none).
    pub fn available(&self) -> bool {
        self.store()
            .and_then(|p| {
                p.call::<_, _, (HashMap<String, Vec<String>>, OwnedValue)>(
                    "Lookup",
                    &("settings-probe", "settings-probe"),
                )
                .map_err(Error::from)
                .map(|_| ())
                .or_else(|e| {
                    // "Not found" is an answer; no service is not.
                    if e.kind == ErrorKind::NotRunning {
                        Err(e)
                    } else {
                        Ok(())
                    }
                })
            })
            .is_ok()
    }

    /// What `app` was told for `what`.
    pub fn get(&self, what: Permission, app: &str) -> Result<Answer, Error> {
        if !valid_app_id(app) {
            return Err(Error::new(ErrorKind::Refused, "not an app ID"));
        }
        let (table, id) = what.key();
        let all: HashMap<String, Vec<String>> = match self
            .store()?
            .call::<_, _, (HashMap<String, Vec<String>>, OwnedValue)>("Lookup", &(table, id))
        {
            Ok((p, _)) => p,
            Err(e) if is_not_found(&e) => return Ok(Answer::Ask),
            Err(e) => return Err(e.into()),
        };
        Ok(all
            .get(app)
            .and_then(|v| v.first())
            .and_then(|v| Answer::from_name(v))
            .unwrap_or(Answer::Ask))
    }

    /// Tells the store what `app` may do. "Ask" removes what is stored.
    pub fn set(&self, what: Permission, app: &str, answer: Answer) -> Result<(), Error> {
        if !valid_app_id(app) {
            return Err(Error::new(ErrorKind::Refused, "not an app ID"));
        }
        let (table, id) = what.key();
        let store = self.store()?;
        match answer {
            Answer::Ask => match store.call::<_, _, ()>("DeletePermission", &(table, id, app)) {
                Ok(()) => Ok(()),
                // Nothing stored: already asking.
                Err(e) if is_not_found(&e) => Ok(()),
                Err(e) => Err(e.into()),
            },
            a => {
                store.call::<_, _, ()>(
                    "SetPermission",
                    &(table, true, id, app, vec![a.name().to_string()]),
                )?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids() {
        for ok in [
            "org.mozilla.firefox",
            "com.valvesoftware.Steam",
            "io.github.a_b-c.App",
        ] {
            assert!(valid_app_id(ok), "{ok}");
        }
        for bad in [
            "", "firefox", ".a.b", "a.b.", "a..b", "a/b.c", "../x.y", "9a.b", "a b.c", "a.b\n",
        ] {
            assert!(!valid_app_id(bad), "{bad:?}");
        }
        assert!(!valid_app_id(&format!("a.{}", "b".repeat(300))));
    }

    #[test]
    fn names_round_trip() {
        for p in Permission::ALL {
            assert_eq!(Permission::from_name(p.name()), Some(p));
        }
        for a in [Answer::Ask, Answer::Yes, Answer::No] {
            assert_eq!(Answer::from_name(a.name()), Some(a));
        }
        assert_eq!(Permission::from_name("../x"), None);
        assert_eq!(Answer::from_name("maybe"), None);
    }
}
