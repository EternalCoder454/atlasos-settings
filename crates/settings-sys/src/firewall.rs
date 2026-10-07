//! The firewall: firewalld (`org.fedoraproject.FirewallD1`) for what it
//! lets through, and systemd (`org.freedesktop.systemd1`) for turning
//! `firewalld.service` on and off, as the Plasma firewall page does. Every
//! change is the owning service's polkit action; the calls ask for the
//! password prompt.

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::error::clean;
use crate::{Error, ErrorKind};
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::proxy::MethodFlags;
use zbus::zvariant::OwnedObjectPath;

const UNIT: &str = "firewalld.service";
const FW_NAME: &str = "org.fedoraproject.FirewallD1";
const FW_PATH: &str = "/org/fedoraproject/FirewallD1";
const FW_ZONE: &str = "org.fedoraproject.FirewallD1.zone";

/// Most services and ports read back.
pub const MAX_ITEMS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// `firewalld.service` is installed.
    pub installed: bool,
    /// It is running now.
    pub running: bool,
    /// It starts at boot.
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Port {
    /// "8080" or "8000-8100".
    pub port: String,
    /// "tcp", "udp", "sctp" or "dccp".
    pub protocol: String,
}

/// What the active zone lets through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    pub zone: String,
    /// The services allowed, sorted.
    pub services: Vec<String>,
    pub ports: Vec<Port>,
    /// Every service firewalld knows, sorted, for "add".
    pub available: Vec<String>,
}

/// A zone or service name: what firewalld's XML names allow.
pub fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && n.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// "80" or "8000-8100", each end 1 to 65535 and the range ascending.
pub fn valid_port(p: &str) -> bool {
    let one = |s: &str| s.len() <= 5 && s.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n));
    match p.split_once('-') {
        Some((a, b)) => {
            one(a) && one(b) && a.parse::<u32>().unwrap_or(0) < b.parse::<u32>().unwrap_or(0)
        }
        None => one(p),
    }
}

pub fn valid_protocol(p: &str) -> bool {
    matches!(p, "tcp" | "udp" | "sctp" | "dccp")
}

pub struct Firewall {
    conn: Connection,
    interactive: Connection,
}

impl Firewall {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Firewall {
            conn: bus.connect()?,
            interactive: bus.connect_with(INTERACTIVE_TIMEOUT)?,
        })
    }

    fn manager<'a>(&self, conn: &'a Connection) -> Result<Proxy<'a>, Error> {
        Ok(Proxy::new(
            conn,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?)
    }

    fn fw<'a>(&self, conn: &'a Connection, iface: &'static str) -> Result<Proxy<'a>, Error> {
        Ok(Proxy::new(conn, FW_NAME, FW_PATH, iface)?)
    }

    /// Whether the firewall is installed, running and on at boot.
    pub fn status(&self) -> Result<Status, Error> {
        let unit: OwnedObjectPath = self.manager(&self.conn)?.call("LoadUnit", &(UNIT,))?;
        let u = Proxy::new(
            &self.conn,
            "org.freedesktop.systemd1",
            unit.as_str().to_string(),
            "org.freedesktop.systemd1.Unit",
        )?;
        let load: String = u.get_property("LoadState")?;
        let active: String = u.get_property("ActiveState")?;
        let file: String = u.get_property("UnitFileState").unwrap_or_default();
        Ok(Status {
            installed: load == "loaded",
            running: active == "active" || active == "activating",
            enabled: file == "enabled" || file == "enabled-runtime",
        })
    }

    /// Turns the firewall on (now and at boot) or off (now and at boot).
    pub fn set_enabled(&self, on: bool) -> Result<(), Error> {
        let m = self.manager(&self.interactive)?;
        let flags = MethodFlags::AllowInteractiveAuth.into();
        if on {
            m.call_with_flags::<_, _, (bool, Vec<(String, String, String)>)>(
                "EnableUnitFiles",
                flags,
                &(vec![UNIT], false, true),
            )?;
            m.call_with_flags::<_, _, OwnedObjectPath>(
                "StartUnit",
                MethodFlags::AllowInteractiveAuth.into(),
                &(UNIT, "replace"),
            )?;
        } else {
            m.call_with_flags::<_, _, OwnedObjectPath>("StopUnit", flags, &(UNIT, "replace"))?;
            m.call_with_flags::<_, _, Vec<(String, String, String)>>(
                "DisableUnitFiles",
                MethodFlags::AllowInteractiveAuth.into(),
                &(vec![UNIT], false),
            )?;
        }
        Ok(())
    }

    /// The zone the rules apply to: the default zone when it is in use, else
    /// the first zone in use.
    fn zone(&self) -> Result<String, Error> {
        let p = self.fw(&self.conn, "org.fedoraproject.FirewallD1")?;
        let default: String = p.call("getDefaultZone", &())?;
        let z = self.fw(&self.conn, FW_ZONE)?;
        let active: HashMap<String, HashMap<String, Vec<String>>> =
            z.call("getActiveZones", &())?;
        let pick = if active.is_empty() || active.contains_key(&default) {
            default
        } else {
            let mut names: Vec<_> = active.keys().cloned().collect();
            names.sort();
            names.swap_remove(0)
        };
        if valid_name(&pick) {
            Ok(pick)
        } else {
            Err(Error::new(ErrorKind::Refused, "firewalld named a bad zone"))
        }
    }

    /// What the active zone allows; `None` when firewalld isn't running.
    pub fn rules(&self) -> Result<Option<Rules>, Error> {
        // Ask systemd first: a call to a firewalld that is off would start it
        // again (D-Bus activation).
        if !self.status()?.running {
            return Ok(None);
        }
        let zone = match self.zone() {
            Ok(z) => z,
            Err(e) if e.kind == ErrorKind::NotRunning => return Ok(None),
            Err(e) => return Err(e),
        };
        let z = self.fw(&self.conn, FW_ZONE)?;
        let mut services: Vec<String> = z.call("getServices", &(&zone,))?;
        services.retain(|s| valid_name(s));
        services.sort();
        services.dedup();
        services.truncate(MAX_ITEMS);
        let listed: Vec<Vec<String>> = z.call("getPorts", &(&zone,))?;
        let mut ports: Vec<Port> = listed
            .into_iter()
            .filter_map(|p| match <[String; 2]>::try_from(p) {
                Ok([port, protocol]) if valid_port(&port) && valid_protocol(&protocol) => {
                    Some(Port { port, protocol })
                }
                _ => None,
            })
            .collect();
        ports.sort();
        ports.dedup();
        ports.truncate(MAX_ITEMS);
        let mut available: Vec<String> = self
            .fw(&self.conn, "org.fedoraproject.FirewallD1")?
            .call("listServices", &())?;
        available.retain(|s| valid_name(s));
        available.sort();
        available.dedup();
        available.truncate(MAX_ITEMS);
        Ok(Some(Rules {
            zone: clean(&zone, 64),
            services,
            ports,
            available,
        }))
    }

    /// Keeps what was changed now after a restart or reload of the firewall.
    fn persist(&self) -> Result<(), Error> {
        self.fw(&self.interactive, "org.fedoraproject.FirewallD1")?
            .call_with_flags::<_, _, ()>(
                "runtimeToPermanent",
                MethodFlags::AllowInteractiveAuth.into(),
                &(),
            )?;
        Ok(())
    }

    fn zone_call<B>(&self, method: &'static str, body: &B) -> Result<(), Error>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.fw(&self.interactive, FW_ZONE)?
            .call_with_flags::<_, _, String>(
                method,
                MethodFlags::AllowInteractiveAuth.into(),
                body,
            )?;
        self.persist()
    }

    pub fn add_service(&self, service: &str) -> Result<(), Error> {
        if !valid_name(service) {
            return Err(Error::new(ErrorKind::Refused, "not a service name"));
        }
        let zone = self.zone()?;
        self.zone_call("addService", &(zone.as_str(), service, 0i32))
    }

    pub fn remove_service(&self, service: &str) -> Result<(), Error> {
        if !valid_name(service) {
            return Err(Error::new(ErrorKind::Refused, "not a service name"));
        }
        let zone = self.zone()?;
        self.zone_call("removeService", &(zone.as_str(), service))
    }

    pub fn add_port(&self, port: &Port) -> Result<(), Error> {
        if !valid_port(&port.port) || !valid_protocol(&port.protocol) {
            return Err(Error::new(ErrorKind::Refused, "not a port"));
        }
        let zone = self.zone()?;
        self.zone_call(
            "addPort",
            &(
                zone.as_str(),
                port.port.as_str(),
                port.protocol.as_str(),
                0i32,
            ),
        )
    }

    pub fn remove_port(&self, port: &Port) -> Result<(), Error> {
        if !valid_port(&port.port) || !valid_protocol(&port.protocol) {
            return Err(Error::new(ErrorKind::Refused, "not a port"));
        }
        let zone = self.zone()?;
        self.zone_call(
            "removePort",
            &(zone.as_str(), port.port.as_str(), port.protocol.as_str()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        for ok in ["ssh", "samba-client", "kdeconnect", "AtlasOS", "a.b_c"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", "a b", "a/b", "../x", "a\n", &"a".repeat(65)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn ports() {
        for ok in ["1", "80", "65535", "8000-8100"] {
            assert!(valid_port(ok), "{ok}");
        }
        for bad in [
            "",
            "0",
            "65536",
            "-1",
            "80-",
            "8100-8000",
            "80-80",
            "a",
            "80 ",
            "1-2-3",
            "999999",
        ] {
            assert!(!valid_port(bad), "{bad:?}");
        }
        assert!(valid_protocol("tcp") && valid_protocol("udp"));
        assert!(!valid_protocol("icmp") && !valid_protocol("TCP") && !valid_protocol(""));
    }
}
