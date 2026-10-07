//! NetworkManager (`org.freedesktop.NetworkManager`): Wi-Fi, wired status,
//! VPN connections, the hotspot and the radios, for Network. Every change
//! is NetworkManager's own (its polkit actions); passwords are handed to it
//! in the call and never kept ([`Secret`]).
//!
//! SSIDs and connection names are untrusted bytes: decoded lossily, capped
//! and cleaned ([`crate::error::clean`]).

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::error::clean;
use crate::{Error, ErrorKind};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use zbus::blocking::proxy::{Builder, Proxy};
use zbus::blocking::Connection;
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const SERVICE: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_IFACE: &str = "org.freedesktop.NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const SETTINGS_IFACE: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION_IFACE: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const DEVICE_IFACE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const AP_IFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";
const ACTIVE_IFACE: &str = "org.freedesktop.NetworkManager.Connection.Active";

/// The longest SSID shown, in characters (an SSID is at most 32 bytes).
pub const MAX_SSID: usize = 32;
/// Most networks listed.
pub const MAX_NETWORKS: usize = 64;
/// Most VPN connections listed.
pub const MAX_VPNS: usize = 32;
/// Most saved connections read.
const MAX_SAVED: usize = 256;
/// How long a connection may take to come up.
const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(30);

type Props = HashMap<String, OwnedValue>;
type ConnectionSettings = HashMap<String, Props>;

/// A password for the length of one call. Not shown by `Debug`, and
/// overwritten when dropped (as far as Rust lets a `String` be).
pub struct Secret(String);

impl Secret {
    pub fn new(text: impl Into<String>) -> Self {
        Secret(text.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.iter_mut().for_each(|b| *b = 0);
        std::hint::black_box(&bytes);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Open,
    /// WEP: old and weak, but still offered.
    Wep,
    /// WPA or WPA2 with a password.
    Personal,
    /// WPA3 with a password.
    Sae,
    /// A login at work or school; set up in Plasma's editor.
    Enterprise,
}

impl Security {
    pub fn secured(self) -> bool {
        self != Security::Open
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Security::Open => "open",
            Security::Wep => "wep",
            Security::Personal => "personal",
            Security::Sae => "sae",
            Security::Enterprise => "enterprise",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiNetwork {
    pub ssid: String,
    /// 0 to 100.
    pub signal: u8,
    pub security: Security,
    pub connected: bool,
    pub connecting: bool,
    /// NetworkManager has a saved connection for it (it can be forgotten).
    pub known: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// No cable.
    Unplugged,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wired {
    /// The interface, `enp3s0`.
    pub interface: String,
    pub link: Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vpn {
    pub id: String,
    pub uuid: String,
    pub active: bool,
    pub connecting: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotspot {
    /// The network name of the saved hotspot.
    pub ssid: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// There is a Wi-Fi adapter.
    pub wifi_present: bool,
    pub wifi_enabled: bool,
    /// The adapter's own switch (rfkill) is off: it can't be turned on here.
    pub wifi_blocked: bool,
    pub wwan_enabled: bool,
    pub wired: Option<Wired>,
    /// The network Wi-Fi is joined to, or being joined.
    pub wifi_ssid: Option<String>,
    pub wifi_connecting: bool,
    pub vpns: Vec<Vpn>,
    pub hotspot: Option<Hotspot>,
}

/// The kind of security from an access point's flags (NM80211ApFlags and
/// NM80211ApSecurityFlags).
pub fn security_of(flags: u32, wpa: u32, rsn: u32) -> Security {
    const KEY_PSK: u32 = 0x100;
    const KEY_8021X: u32 = 0x200;
    const KEY_SAE: u32 = 0x400;
    let all = wpa | rsn;
    if all & (KEY_PSK) != 0 {
        Security::Personal
    } else if all & KEY_SAE != 0 {
        Security::Sae
    } else if all & KEY_8021X != 0 {
        Security::Enterprise
    } else if flags & 1 != 0 || all != 0 {
        Security::Wep
    } else {
        Security::Open
    }
}

/// An SSID's bytes as text to show.
pub fn ssid_text(bytes: &[u8]) -> String {
    clean(String::from_utf8_lossy(bytes).trim(), MAX_SSID)
}

/// Whether `password` can be a Wi-Fi password for `security`: 8 to 63
/// characters (or 64 hex digits) for WPA, 5 or 13 characters for WEP.
pub fn valid_password(security: Security, password: &str) -> bool {
    let n = password.chars().count();
    let fits = match security {
        Security::Open | Security::Enterprise => password.is_empty(),
        Security::Wep => matches!(password.len(), 5 | 13 | 10 | 26),
        Security::Personal | Security::Sae => {
            (8..=63).contains(&n)
                || (password.len() == 64 && password.bytes().all(|b| b.is_ascii_hexdigit()))
        }
    };
    fits && !password.chars().any(char::is_control)
}

/// Whether `name` can be a network name for a hotspot: 1 to 32 bytes, no
/// control characters.
pub fn valid_ssid(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 32 && !name.chars().any(char::is_control)
}

struct Saved {
    path: String,
    kind: String,
    uuid: String,
    id: String,
    ssid: Vec<u8>,
    mode: String,
}

struct Active {
    path: String,
    uuid: String,
    connection: String,
    state: u32,
    devices: Vec<String>,
}

struct AccessPoint {
    path: String,
    ssid: Vec<u8>,
    signal: u8,
    security: Security,
}

struct Device {
    path: String,
    kind: u32,
    state: u32,
    interface: String,
}

pub struct Network {
    conn: Connection,
    interactive: Connection,
}

fn prop_str(p: &Props, name: &str) -> String {
    p.get(name)
        .and_then(|v| v.downcast_ref::<&str>().ok().map(str::to_string))
        .unwrap_or_default()
}

fn prop_bytes(p: &Props, name: &str) -> Vec<u8> {
    p.get(name)
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| Vec::<u8>::try_from(v).ok())
        .unwrap_or_default()
}

fn op(path: &str) -> Result<OwnedObjectPath, Error> {
    OwnedObjectPath::try_from(path).map_err(|e| refused(e.to_string()))
}

fn refused(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Refused, detail)
}

impl Network {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Network {
            conn: bus.connect()?,
            interactive: bus.connect_with(INTERACTIVE_TIMEOUT)?,
        })
    }

    fn proxy(
        &self,
        conn: &Connection,
        path: &str,
        iface: &'static str,
    ) -> Result<Proxy<'static>, Error> {
        Ok(Builder::new(conn)
            .destination(SERVICE)?
            .path(path.to_string())?
            .interface(iface)?
            .cache_properties(CacheProperties::No)
            .build()?)
    }

    fn nm(&self) -> Result<Proxy<'static>, Error> {
        self.proxy(&self.conn, NM_PATH, NM_IFACE)
    }

    fn nm_interactive(&self) -> Result<Proxy<'static>, Error> {
        self.proxy(&self.interactive, NM_PATH, NM_IFACE)
    }

    fn devices(&self) -> Result<Vec<Device>, Error> {
        let paths: Vec<OwnedObjectPath> = self.nm()?.call("GetDevices", &())?;
        let mut out = Vec::new();
        for p in paths.into_iter().take(64) {
            let path = p.as_str().to_string();
            let d = self.proxy(&self.conn, &path, DEVICE_IFACE)?;
            // A device that vanished meanwhile is not an error.
            let (Ok(kind), Ok(state)) = (
                d.get_property::<u32>("DeviceType"),
                d.get_property::<u32>("State"),
            ) else {
                continue;
            };
            out.push(Device {
                kind,
                state,
                interface: clean(&d.get_property::<String>("Interface").unwrap_or_default(), 32),
                path,
            });
        }
        Ok(out)
    }

    fn saved(&self) -> Result<Vec<Saved>, Error> {
        let paths: Vec<OwnedObjectPath> = self
            .proxy(&self.conn, SETTINGS_PATH, SETTINGS_IFACE)?
            .call("ListConnections", &())?;
        let mut out = Vec::new();
        for p in paths.into_iter().take(MAX_SAVED) {
            let path = p.as_str().to_string();
            let Ok(c) = self.proxy(&self.conn, &path, CONNECTION_IFACE) else {
                continue;
            };
            let Ok(settings) = c.call::<_, _, ConnectionSettings>("GetSettings", &()) else {
                continue;
            };
            let empty = Props::new();
            let head = settings.get("connection").unwrap_or(&empty);
            let wifi = settings.get("802-11-wireless").unwrap_or(&empty);
            out.push(Saved {
                kind: prop_str(head, "type"),
                uuid: prop_str(head, "uuid"),
                id: clean(&prop_str(head, "id"), 64),
                ssid: prop_bytes(wifi, "ssid"),
                mode: prop_str(wifi, "mode"),
                path,
            });
        }
        Ok(out)
    }

    fn active(&self) -> Result<Vec<Active>, Error> {
        let paths: Vec<OwnedObjectPath> = self.nm()?.get_property("ActiveConnections")?;
        let mut out = Vec::new();
        for p in paths.into_iter().take(64) {
            let path = p.as_str().to_string();
            let a = self.proxy(&self.conn, &path, ACTIVE_IFACE)?;
            let Ok(state) = a.get_property::<u32>("State") else {
                continue;
            };
            out.push(Active {
                state,
                uuid: a.get_property("Uuid").unwrap_or_default(),
                connection: a
                    .get_property::<OwnedObjectPath>("Connection")
                    .map(|c| c.as_str().to_string())
                    .unwrap_or_default(),
                devices: a
                    .get_property::<Vec<OwnedObjectPath>>("Devices")
                    .unwrap_or_default()
                    .iter()
                    .map(|d| d.as_str().to_string())
                    .collect(),
                path,
            });
        }
        Ok(out)
    }

    fn access_points(&self, device: &str) -> Result<Vec<AccessPoint>, Error> {
        let w = self.proxy(&self.conn, device, WIRELESS_IFACE)?;
        let paths: Vec<OwnedObjectPath> = w.call("GetAllAccessPoints", &())?;
        let mut out = Vec::new();
        for p in paths.into_iter().take(512) {
            let path = p.as_str().to_string();
            let ap = self.proxy(&self.conn, &path, AP_IFACE)?;
            let Ok(ssid) = ap.get_property::<Vec<u8>>("Ssid") else {
                continue;
            };
            out.push(AccessPoint {
                ssid,
                signal: ap.get_property::<u8>("Strength").unwrap_or(0).min(100),
                security: security_of(
                    ap.get_property("Flags").unwrap_or(0),
                    ap.get_property("WpaFlags").unwrap_or(0),
                    ap.get_property("RsnFlags").unwrap_or(0),
                ),
                path,
            });
        }
        Ok(out)
    }

    fn wifi_device(&self) -> Result<Device, Error> {
        self.devices()?
            .into_iter()
            .find(|d| d.kind == 2)
            .ok_or_else(|| refused("no Wi-Fi adapter"))
    }

    pub fn status(&self) -> Result<Status, Error> {
        let nm = self.nm()?;
        let wifi_enabled: bool = nm.get_property("WirelessEnabled")?;
        let wifi_hw: bool = nm.get_property("WirelessHardwareEnabled").unwrap_or(true);
        let wwan_enabled: bool = nm.get_property("WwanEnabled").unwrap_or(false);
        let devices = self.devices()?;
        let saved = self.saved()?;
        let active = self.active()?;

        let wifi = devices.iter().find(|d| d.kind == 2);
        let wired = devices
            .iter()
            .find(|d| d.kind == 1 && d.state != 10)
            .map(|d| Wired {
                interface: d.interface.clone(),
                link: match d.state {
                    100 => Link::Connected,
                    40..=90 => Link::Connecting,
                    20 => Link::Unplugged,
                    _ => Link::Disconnected,
                },
            });
        let by_uuid = |uuid: &str| saved.iter().find(|s| s.uuid == uuid);

        // The Wi-Fi in use: an active, non-hotspot connection on the adapter.
        let mut wifi_ssid = None;
        let mut wifi_connecting = false;
        if let Some(w) = wifi {
            for a in active.iter().filter(|a| a.devices.contains(&w.path)) {
                let ssid = saved
                    .iter()
                    .find(|s| s.path == a.connection)
                    .filter(|s| s.kind == "802-11-wireless" && s.mode != "ap")
                    .map(|s| ssid_text(&s.ssid));
                if let Some(ssid) = ssid.filter(|s| !s.is_empty()) {
                    wifi_ssid = Some(ssid);
                    wifi_connecting = a.state == 1;
                    break;
                }
            }
        }

        let vpns = saved
            .iter()
            .filter(|s| matches!(s.kind.as_str(), "vpn" | "wireguard"))
            .take(MAX_VPNS)
            .map(|s| {
                let a = active.iter().find(|a| a.uuid == s.uuid);
                Vpn {
                    id: if s.id.is_empty() {
                        "VPN".into()
                    } else {
                        s.id.clone()
                    },
                    uuid: s.uuid.clone(),
                    active: a.is_some_and(|a| a.state == 2),
                    connecting: a.is_some_and(|a| a.state == 1),
                }
            })
            .collect();

        let hotspot = saved
            .iter()
            .find(|s| s.kind == "802-11-wireless" && s.mode == "ap")
            .map(|s| Hotspot {
                ssid: ssid_text(&s.ssid),
                active: active
                    .iter()
                    .any(|a| a.uuid == s.uuid && a.state <= 2 && by_uuid(&a.uuid).is_some()),
            });

        Ok(Status {
            wifi_present: wifi.is_some(),
            wifi_enabled,
            wifi_blocked: !wifi_hw,
            wwan_enabled,
            wired,
            wifi_ssid,
            wifi_connecting,
            vpns,
            hotspot,
        })
    }

    /// The networks in range, one per name (the strongest), the one in use
    /// first, then the saved ones, then by signal.
    pub fn wifi_networks(&self) -> Result<Vec<WifiNetwork>, Error> {
        let Some(dev) = self.devices()?.into_iter().find(|d| d.kind == 2) else {
            return Ok(Vec::new());
        };
        let saved = self.saved()?;
        let active = self.active()?;
        let in_use: Vec<(String, bool)> = active
            .iter()
            .filter(|a| a.devices.contains(&dev.path))
            .filter_map(|a| {
                saved
                    .iter()
                    .find(|s| s.path == a.connection)
                    .filter(|s| s.kind == "802-11-wireless" && s.mode != "ap")
                    .map(|s| (ssid_text(&s.ssid), a.state == 1))
            })
            .collect();

        let mut best: HashMap<String, WifiNetwork> = HashMap::new();
        for ap in self.access_points(&dev.path)? {
            let ssid = ssid_text(&ap.ssid);
            // Hidden networks have no name to show.
            if ssid.is_empty() || ssid.chars().all(|c| c == '\u{FFFD}') {
                continue;
            }
            let state = in_use.iter().find(|(s, _)| *s == ssid);
            let n = WifiNetwork {
                known: saved.iter().any(|s| {
                    s.kind == "802-11-wireless" && s.mode != "ap" && ssid_text(&s.ssid) == ssid
                }),
                connected: state.is_some_and(|(_, connecting)| !connecting),
                connecting: state.is_some_and(|(_, connecting)| *connecting),
                signal: ap.signal,
                security: ap.security,
                ssid: ssid.clone(),
            };
            match best.get(&ssid) {
                Some(old) if old.signal >= n.signal => {}
                _ => {
                    best.insert(ssid, n);
                }
            }
        }
        let mut list: Vec<WifiNetwork> = best.into_values().collect();
        list.sort_by(|a, b| {
            (b.connected || b.connecting)
                .cmp(&(a.connected || a.connecting))
                .then(b.known.cmp(&a.known))
                .then(b.signal.cmp(&a.signal))
                .then(a.ssid.cmp(&b.ssid))
        });
        list.truncate(MAX_NETWORKS);
        Ok(list)
    }

    /// Asks the adapter to look for networks again. Not an error when it
    /// refuses (a scan just ran, the radio is off).
    pub fn scan(&self) {
        if let Ok(dev) = self.wifi_device()
            && let Ok(w) = self.proxy(&self.conn, &dev.path, WIRELESS_IFACE)
        {
            let options: HashMap<&str, Value<'_>> = HashMap::new();
            let _: Result<(), _> = w.call("RequestScan", &(options,));
        }
    }

    pub fn set_wifi_enabled(&self, on: bool) -> Result<(), Error> {
        Ok(self.nm_interactive()?.set_property("WirelessEnabled", on)?)
    }

    /// Wi-Fi and mobile broadband off, or back on (airplane mode's share of
    /// the radios; Bluetooth is BlueZ's).
    pub fn set_radios_enabled(&self, on: bool) -> Result<(), Error> {
        let nm = self.nm_interactive()?;
        nm.set_property("WirelessEnabled", on)?;
        // Mobile broadband may be absent; Wi-Fi is what has to work.
        if let Err(e) = nm.set_property("WwanEnabled", on) {
            log::info!("WwanEnabled: {e}");
        }
        Ok(())
    }

    /// Waits for the active connection at `path` to come up.
    fn wait_active(&self, path: &str) -> Result<(), Error> {
        let started = Instant::now();
        loop {
            let a = self.proxy(&self.conn, path, ACTIVE_IFACE)?;
            match a.get_property::<u32>("State") {
                Ok(2) => return Ok(()),
                Ok(0 | 1) => {}
                // Deactivating, deactivated, or gone.
                Ok(_) | Err(_) => return Err(refused("the connection did not come up")),
            }
            if started.elapsed() > ACTIVATE_TIMEOUT {
                return Err(Error::new(ErrorKind::Timeout, "the connection took too long"));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn delete_connection(&self, path: &str) {
        if let Ok(c) = self.proxy(&self.interactive, path, CONNECTION_IFACE) {
            let _: Result<(), _> = c.call("Delete", &());
        }
    }

    /// Joins the network `ssid`. A network with a saved connection is
    /// joined with that (no password asked); otherwise `password` goes to
    /// NetworkManager, which keeps it. A connection that doesn't come up is
    /// not kept.
    pub fn connect_wifi(&self, ssid: &str, password: Option<&Secret>) -> Result<(), Error> {
        let dev = self.wifi_device()?;
        let ap = self
            .access_points(&dev.path)?
            .into_iter()
            .filter(|a| ssid_text(&a.ssid) == ssid)
            .max_by_key(|a| a.signal)
            .ok_or_else(|| refused("that network is out of range"))?;
        let saved = self.saved()?;
        let known = saved.iter().find(|s| {
            s.kind == "802-11-wireless" && s.mode != "ap" && ssid_text(&s.ssid) == ssid
        });
        let nm = self.nm_interactive()?;

        let (created, active): (Option<String>, OwnedObjectPath) = match (known, password) {
            (Some(k), None) => {
                let path = op(k.path.as_str())?;
                let dev_path = op(dev.path.as_str())?;
                let ap_path = op(ap.path.as_str())?;
                (None, nm.call("ActivateConnection", &(path, dev_path, ap_path))?)
            }
            _ => {
                let settings = wifi_settings(&ap, password)?;
                let dev_path = op(dev.path.as_str())?;
                let ap_path = op(ap.path.as_str())?;
                let (conn, active): (OwnedObjectPath, OwnedObjectPath) =
                    nm.call("AddAndActivateConnection", &(settings, dev_path, ap_path))?;
                (Some(conn.as_str().to_string()), active)
            }
        };
        if let Err(e) = self.wait_active(active.as_str()) {
            if let Some(c) = created {
                self.delete_connection(&c);
            }
            return Err(e);
        }
        Ok(())
    }

    /// Leaves the Wi-Fi network in use (its connection stays saved).
    pub fn disconnect_wifi(&self) -> Result<(), Error> {
        let dev = self.wifi_device()?;
        let active = self.active()?;
        let saved = self.saved()?;
        for a in active.iter().filter(|a| {
            a.devices.contains(&dev.path)
                && saved
                    .iter()
                    .any(|s| s.path == a.connection && s.mode != "ap")
        }) {
            let path = op(&a.path)?;
            self.nm_interactive()?
                .call::<_, _, ()>("DeactivateConnection", &(path,))?;
        }
        Ok(())
    }

    /// Removes every saved connection for the network `ssid`.
    pub fn forget_wifi(&self, ssid: &str) -> Result<(), Error> {
        let mut any = false;
        for s in self.saved()? {
            if s.kind == "802-11-wireless" && s.mode != "ap" && ssid_text(&s.ssid) == ssid {
                let c = self.proxy(&self.interactive, &s.path, CONNECTION_IFACE)?;
                c.call::<_, _, ()>("Delete", &())?;
                any = true;
            }
        }
        if any {
            Ok(())
        } else {
            Err(refused("no saved connection for that network"))
        }
    }

    /// Connects or disconnects the VPN with this UUID.
    pub fn set_vpn(&self, uuid: &str, on: bool) -> Result<(), Error> {
        let nm = self.nm_interactive()?;
        if on {
            let s = self
                .saved()?
                .into_iter()
                .find(|s| s.uuid == uuid && matches!(s.kind.as_str(), "vpn" | "wireguard"))
                .ok_or_else(|| refused("no such VPN"))?;
            let path = op(s.path.as_str())?;
            let none = op("/")?;
            let active: OwnedObjectPath =
                nm.call("ActivateConnection", &(path, none.clone(), none))?;
            // Secrets come from the login agent (Plasma's), which may ask.
            self.wait_active(active.as_str())
        } else {
            let active = self.active()?;
            let Some(a) = active.iter().find(|a| a.uuid == uuid) else {
                return Ok(());
            };
            let path = op(a.path.as_str())?;
            Ok(nm.call("DeactivateConnection", &(path,))?)
        }
    }

    /// Starts the hotspot: the saved one when `password` is `None`, else a
    /// new one with this name and password (replacing the saved one).
    pub fn start_hotspot(&self, ssid: &str, password: Option<&Secret>) -> Result<(), Error> {
        let dev = self.wifi_device()?;
        let saved = self.saved()?;
        let old: Vec<&Saved> = saved
            .iter()
            .filter(|s| s.kind == "802-11-wireless" && s.mode == "ap")
            .collect();
        let nm = self.nm_interactive()?;
        let dev_path = op(dev.path.as_str())?;
        let none = op("/")?;
        let active: OwnedObjectPath = match (password, old.first()) {
            (None, Some(s)) => {
                let path = op(s.path.as_str())?;
                nm.call("ActivateConnection", &(path, dev_path, none))?
            }
            (None, None) => return Err(refused("no saved hotspot")),
            (Some(pw), _) => {
                if !valid_ssid(ssid) || !valid_password(Security::Personal, pw.expose()) {
                    return Err(refused("not a valid hotspot name or password"));
                }
                for s in &old {
                    self.delete_connection(&s.path);
                }
                let settings = hotspot_settings(ssid, pw);
                let (_conn, active): (OwnedObjectPath, OwnedObjectPath) =
                    nm.call("AddAndActivateConnection", &(settings, dev_path, none))?;
                active
            }
        };
        self.wait_active(active.as_str())
    }

    pub fn stop_hotspot(&self) -> Result<(), Error> {
        let saved = self.saved()?;
        let active = self.active()?;
        for a in &active {
            if saved
                .iter()
                .any(|s| s.uuid == a.uuid && s.kind == "802-11-wireless" && s.mode == "ap")
            {
                let path = op(a.path.as_str())?;
                self.nm_interactive()?
                    .call::<_, _, ()>("DeactivateConnection", &(path,))?;
            }
        }
        Ok(())
    }

    /// Removes the saved hotspot.
    pub fn forget_hotspot(&self) -> Result<(), Error> {
        self.stop_hotspot()?;
        for s in self.saved()? {
            if s.kind == "802-11-wireless" && s.mode == "ap" {
                self.delete_connection(&s.path);
            }
        }
        Ok(())
    }
}

type NewSettings<'a> = HashMap<&'static str, HashMap<&'static str, Value<'a>>>;

/// The connection for a new Wi-Fi network with its password. NetworkManager
/// fills in the rest, and keeps the password in its own store.
fn wifi_settings<'a>(ap: &AccessPoint, password: Option<&'a Secret>) -> Result<NewSettings<'a>, Error> {
    let mut s: NewSettings<'a> = HashMap::new();
    s.entry("connection").or_default().extend([
        ("type", Value::from("802-11-wireless")),
        ("id", Value::from(ssid_text(&ap.ssid))),
        ("autoconnect", Value::from(true)),
    ]);
    s.entry("802-11-wireless").or_default().extend([
        ("ssid", Value::from(ap.ssid.clone())),
        ("mode", Value::from("infrastructure")),
    ]);
    match (ap.security, password) {
        (Security::Open, _) => {}
        (Security::Enterprise, _) => {
            return Err(refused("this network needs a login set up in Plasma"));
        }
        (sec, Some(pw)) if valid_password(sec, pw.expose()) => {
            s.get_mut("802-11-wireless")
                .expect("just inserted")
                .insert("security", Value::from("802-11-wireless-security"));
            let sec_map = s.entry("802-11-wireless-security").or_default();
            match sec {
                Security::Personal => {
                    sec_map.insert("key-mgmt", Value::from("wpa-psk"));
                    sec_map.insert("psk", Value::from(pw.expose()));
                }
                Security::Sae => {
                    sec_map.insert("key-mgmt", Value::from("sae"));
                    sec_map.insert("psk", Value::from(pw.expose()));
                }
                _ => {
                    sec_map.insert("key-mgmt", Value::from("none"));
                    sec_map.insert("wep-key0", Value::from(pw.expose()));
                }
            }
        }
        _ => return Err(refused("a valid password is needed")),
    }
    Ok(s)
}

fn hotspot_settings<'a>(ssid: &'a str, password: &'a Secret) -> NewSettings<'a> {
    let mut s: NewSettings<'a> = HashMap::new();
    s.entry("connection").or_default().extend([
        ("type", Value::from("802-11-wireless")),
        ("id", Value::from(ssid)),
        ("autoconnect", Value::from(false)),
    ]);
    s.entry("802-11-wireless").or_default().extend([
        ("ssid", Value::from(ssid.as_bytes().to_vec())),
        ("mode", Value::from("ap")),
        ("security", Value::from("802-11-wireless-security")),
    ]);
    s.entry("802-11-wireless-security").or_default().extend([
        ("key-mgmt", Value::from("wpa-psk")),
        ("psk", Value::from(password.expose())),
    ]);
    s.entry("ipv4")
        .or_default()
        .insert("method", Value::from("shared"));
    s.entry("ipv6")
        .or_default()
        .insert("method", Value::from("ignore"));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_from_flags() {
        assert_eq!(security_of(0, 0, 0), Security::Open);
        assert_eq!(security_of(1, 0, 0), Security::Wep);
        assert_eq!(security_of(1, 0, 0x100), Security::Personal);
        assert_eq!(security_of(1, 0x100, 0x400), Security::Personal);
        assert_eq!(security_of(1, 0, 0x400), Security::Sae);
        assert_eq!(security_of(1, 0, 0x200), Security::Enterprise);
    }

    #[test]
    fn ssids_are_safe_text() {
        assert_eq!(ssid_text(b"Cafe"), "Cafe");
        assert_eq!(ssid_text(b"a\x1b[31mb"), "a\u{FFFD}[31mb");
        assert_eq!(ssid_text(&[0xff, 0xfe]), "\u{FFFD}\u{FFFD}");
        assert_eq!(ssid_text("\u{202E}gnp".as_bytes()), "\u{FFFD}gnp");
        assert_eq!(ssid_text(&[b'x'; 200]).chars().count(), MAX_SSID + 1);
        assert_eq!(ssid_text(b""), "");
    }

    #[test]
    fn passwords() {
        assert!(valid_password(Security::Personal, "correct horse"));
        assert!(!valid_password(Security::Personal, "short"));
        assert!(!valid_password(Security::Personal, &"x".repeat(64)));
        assert!(valid_password(Security::Personal, &"a".repeat(64)));
        assert!(!valid_password(Security::Personal, "with\nnewline"));
        assert!(valid_password(Security::Open, ""));
        assert!(!valid_password(Security::Open, "x"));
        assert!(valid_password(Security::Wep, "12345"));
    }

    #[test]
    fn secrets_stay_out_of_debug() {
        let s = Secret::new("hunter22hunter22");
        assert!(!format!("{s:?}").contains("hunter"));
        assert_eq!(s.expose(), "hunter22hunter22");
    }

    #[test]
    fn hotspot_names() {
        assert!(valid_ssid("My Laptop"));
        assert!(!valid_ssid("  "));
        assert!(!valid_ssid(&"x".repeat(33)));
        assert!(!valid_ssid("a\nb"));
    }
}
