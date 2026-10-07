//! systemd-hostnamed (`org.freedesktop.hostname1`): the device's name, for
//! System. The name people see is the pretty hostname; the static hostname
//! (what the network sees) is derived from it. Changes are hostnamed's
//! polkit actions (`org.freedesktop.hostname1.set-*hostname`).

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::error::{clean, is_invisible};
use crate::{Error, ErrorKind};

#[zbus::proxy(
    interface = "org.freedesktop.hostname1",
    default_service = "org.freedesktop.hostname1",
    default_path = "/org/freedesktop/hostname1",
    gen_async = false,
    blocking_name = "HostnameProxy"
)]
trait Hostname {
    #[zbus(property)]
    fn hostname(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn static_hostname(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn pretty_hostname(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn chassis(&self) -> zbus::Result<String>;

    fn set_pretty_hostname(&self, name: &str, interactive: bool) -> zbus::Result<()>;
    fn set_static_hostname(&self, name: &str, interactive: bool) -> zbus::Result<()>;
}

/// The longest device name accepted, in characters.
pub const MAX_NAME: usize = 64;
/// The longest static hostname (a DNS label).
pub const MAX_HOSTNAME: usize = 63;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// The name to show: the pretty hostname, else the static one, else the
    /// transient one. Made safe to show.
    pub name: String,
    /// The static hostname (may be empty).
    pub hostname: String,
    /// `laptop`, `desktop`, `vm`, ... or empty.
    pub chassis: String,
}

/// Whether `name` can be a device name: not blank, at most [`MAX_NAME`]
/// characters, no control or invisible characters.
pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= MAX_NAME
        && !name.chars().any(|c| c.is_control() || is_invisible(c))
}

/// The static hostname for a device name: lower-case ASCII letters, digits
/// and single hyphens, at most [`MAX_HOSTNAME`] long; empty when nothing of
/// the name is left ("Zoë's PC" gives `zo-s-pc`).
pub fn hostname_for(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= MAX_HOSTNAME {
            break;
        }
    }
    out.truncate(MAX_HOSTNAME);
    out.trim_end_matches('-').to_string()
}

pub struct Hostname {
    conn: zbus::blocking::Connection,
    interactive: zbus::blocking::Connection,
}

impl Hostname {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Hostname {
            conn: bus.connect()?,
            interactive: bus.connect_with(INTERACTIVE_TIMEOUT)?,
        })
    }

    pub fn status(&self) -> Result<Status, Error> {
        let p = HostnameProxy::new(&self.conn)?;
        let pretty = clean(p.pretty_hostname()?.trim(), MAX_NAME);
        let hostname = clean(&p.static_hostname()?, MAX_NAME);
        let name = if !pretty.is_empty() {
            pretty
        } else if !hostname.is_empty() {
            hostname.clone()
        } else {
            clean(&p.hostname()?, MAX_NAME)
        };
        Ok(Status {
            name,
            hostname,
            chassis: clean(&p.chassis().unwrap_or_default(), 32),
        })
    }

    /// Names the device: the pretty hostname as given, and the static
    /// hostname derived from it when anything of it is left. polkit may ask
    /// for a password.
    pub fn set_name(&self, name: &str) -> Result<(), Error> {
        let name = name.trim();
        if !valid_name(name) {
            return Err(Error::new(
                ErrorKind::Refused,
                format!("not a device name: {name:?}"),
            ));
        }
        let p = HostnameProxy::new(&self.interactive)?;
        p.set_pretty_hostname(name, true)?;
        let host = hostname_for(name);
        if !host.is_empty() {
            p.set_static_hostname(&host, true)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("Zach's Laptop"));
        assert!(valid_name("Zoë"));
        for bad in ["", "   ", "a\nb", "a\u{202E}b", &"x".repeat(65)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn hostnames_from_names() {
        assert_eq!(hostname_for("Zach's Laptop"), "zach-s-laptop");
        assert_eq!(hostname_for("  --Atlas  PC--  "), "atlas-pc");
        assert_eq!(hostname_for("Zoë"), "zo");
        assert_eq!(hostname_for("日本"), "");
        assert_eq!(hostname_for(&"a".repeat(100)).len(), MAX_HOSTNAME);
        let h = hostname_for(&format!("{}-b", "a".repeat(62)));
        assert!(!h.ends_with('-') && h.len() <= MAX_HOSTNAME, "{h}");
    }
}
