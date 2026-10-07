//! BlueZ (`org.bluez`): the Bluetooth adapter and its devices, for
//! Bluetooth & Devices. Everything is read in one `GetManagedObjects` call;
//! devices are named by their address, and the object path is looked up
//! here, so a page never passes a path to the bus.
//!
//! Pairing that needs a PIN or a confirmation is asked by the session's
//! Bluetooth agent (Plasma's BlueDevil); Settings registers none of its own.
//! Device names are untrusted: capped, cleaned, shown as plain text.

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::error::clean;
use crate::{Error, ErrorKind};
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::blocking::proxy::{Builder, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const SERVICE: &str = "org.bluez";
const ADAPTER_IFACE: &str = "org.bluez.Adapter1";
const DEVICE_IFACE: &str = "org.bluez.Device1";
const BATTERY_IFACE: &str = "org.bluez.Battery1";

/// The longest device name shown, in characters.
pub const MAX_NAME: usize = 64;
/// Most devices listed.
pub const MAX_DEVICES: usize = 100;

type Props = HashMap<String, OwnedValue>;
type Objects = HashMap<OwnedObjectPath, HashMap<String, Props>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    /// The name other devices see.
    pub name: String,
    pub powered: bool,
    pub discoverable: bool,
    pub discovering: bool,
}

/// What a device is, from the icon BlueZ names for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Gamepad,
    Phone,
    Computer,
    Watch,
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Headphones => "headphones",
            Kind::Speaker => "speaker",
            Kind::Keyboard => "keyboard",
            Kind::Mouse => "mouse",
            Kind::Gamepad => "gamepad",
            Kind::Phone => "phone",
            Kind::Computer => "computer",
            Kind::Watch => "watch",
            Kind::Other => "other",
        }
    }

    fn from_icon(icon: &str) -> Kind {
        match icon {
            "audio-headset" | "audio-headphones" => Kind::Headphones,
            "audio-card" | "audio-speakers" | "audio-speaker" => Kind::Speaker,
            "input-keyboard" => Kind::Keyboard,
            "input-mouse" | "input-tablet" => Kind::Mouse,
            "input-gaming" => Kind::Gamepad,
            "phone" | "smartphone" => Kind::Phone,
            "computer" | "laptop" => Kind::Computer,
            "watch" => Kind::Watch,
            _ => Kind::Other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// `AA:BB:CC:DD:EE:FF`, upper case.
    pub address: String,
    pub name: String,
    pub kind: Kind,
    pub paired: bool,
    pub connected: bool,
    /// Percent, when the device reports it.
    pub battery: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// `None` when the computer has no Bluetooth adapter.
    pub adapter: Option<Adapter>,
    /// Paired devices, then the ones nearby, the connected ones first.
    pub devices: Vec<Device>,
}

/// Whether `address` is a Bluetooth address, `AA:BB:CC:DD:EE:FF`.
pub fn valid_address(address: &str) -> bool {
    let parts: Vec<&str> = address.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn get<'a>(p: &'a Props, name: &str) -> Option<&'a OwnedValue> {
    p.get(name)
}

fn bool_of(p: &Props, name: &str) -> bool {
    get(p, name)
        .and_then(|v| v.downcast_ref::<bool>().ok())
        .unwrap_or(false)
}

fn str_of(p: &Props, name: &str) -> String {
    get(p, name)
        .and_then(|v| v.downcast_ref::<&str>().ok().map(str::to_string))
        .unwrap_or_default()
}

fn refused(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Refused, detail)
}

/// The adapter and devices in BlueZ's managed objects. Pure, for tests.
fn parse(objects: &Objects) -> (Option<(String, Adapter)>, Snapshot) {
    let mut adapters: Vec<(&OwnedObjectPath, &Props)> = objects
        .iter()
        .filter_map(|(p, i)| i.get(ADAPTER_IFACE).map(|props| (p, props)))
        .collect();
    adapters.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    let adapter = adapters.first().map(|(path, p)| {
        let alias = str_of(p, "Alias");
        let name = if alias.is_empty() {
            str_of(p, "Name")
        } else {
            alias
        };
        (
            path.as_str().to_string(),
            Adapter {
                name: clean(&name, MAX_NAME),
                powered: bool_of(p, "Powered"),
                discoverable: bool_of(p, "Discoverable"),
                discovering: bool_of(p, "Discovering"),
            },
        )
    });
    let mut devices = Vec::new();
    if let Some((adapter_path, _)) = &adapter {
        for ifaces in objects.values() {
            let Some(p) = ifaces.get(DEVICE_IFACE) else {
                continue;
            };
            let on_adapter = get(p, "Adapter")
                .and_then(|v| v.downcast_ref::<zbus::zvariant::ObjectPath<'_>>().ok())
                .is_some_and(|a| a.as_str() == adapter_path);
            let address = str_of(p, "Address").to_ascii_uppercase();
            if !on_adapter || !valid_address(&address) {
                continue;
            }
            let paired = bool_of(p, "Paired");
            let named = !str_of(p, "Name").is_empty() || !str_of(p, "Alias").is_empty();
            let seen = get(p, "RSSI").is_some();
            // Unpaired devices are listed while they are in range and say
            // who they are; BlueZ keeps the rest cached for minutes.
            if !paired && !(seen && named) {
                continue;
            }
            let alias = str_of(p, "Alias");
            let name = if !alias.is_empty() {
                alias
            } else if !str_of(p, "Name").is_empty() {
                str_of(p, "Name")
            } else {
                address.clone()
            };
            let battery = ifaces
                .get(BATTERY_IFACE)
                .and_then(|b| get(b, "Percentage"))
                .and_then(|v| v.downcast_ref::<u8>().ok())
                .filter(|p| *p <= 100);
            devices.push(Device {
                address,
                name: clean(&name, MAX_NAME),
                kind: Kind::from_icon(&str_of(p, "Icon")),
                paired,
                connected: bool_of(p, "Connected"),
                battery,
            });
        }
    }
    devices.sort_by(|a, b| {
        b.paired
            .cmp(&a.paired)
            .then(b.connected.cmp(&a.connected))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.address.cmp(&b.address))
    });
    devices.dedup_by(|a, b| a.address == b.address);
    devices.truncate(MAX_DEVICES);
    let snapshot = Snapshot {
        adapter: adapter.as_ref().map(|(_, a)| a.clone()),
        devices,
    };
    (adapter, snapshot)
}

pub struct Bluetooth {
    conn: Connection,
    interactive: Connection,
}

impl Bluetooth {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Bluetooth {
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

    fn objects(&self) -> Result<Objects, Error> {
        Ok(self
            .proxy(&self.conn, "/", "org.freedesktop.DBus.ObjectManager")?
            .call("GetManagedObjects", &())?)
    }

    fn read(&self) -> Result<(Option<(String, Adapter)>, Snapshot, Objects), Error> {
        let objects = self.objects()?;
        let (adapter, snapshot) = parse(&objects);
        Ok((adapter, snapshot, objects))
    }

    pub fn snapshot(&self) -> Result<Snapshot, Error> {
        Ok(self.read()?.1)
    }

    fn adapter_path(&self) -> Result<String, Error> {
        self.read()?
            .0
            .map(|(p, _)| p)
            .ok_or_else(|| refused("no Bluetooth adapter"))
    }

    /// The object path of the device with this address.
    fn device_path(&self, address: &str) -> Result<String, Error> {
        if !valid_address(address) {
            return Err(refused(format!("not a Bluetooth address: {address:?}")));
        }
        let (adapter, _, objects) = self.read()?;
        let (adapter_path, _) = adapter.ok_or_else(|| refused("no Bluetooth adapter"))?;
        let prefix = format!("{adapter_path}/");
        objects
            .iter()
            .filter(|(path, _)| path.as_str().starts_with(&prefix))
            .find(|(_, i)| {
                i.get(DEVICE_IFACE)
                    .is_some_and(|p| str_of(p, "Address").eq_ignore_ascii_case(address))
            })
            .map(|(p, _)| p.as_str().to_string())
            .ok_or_else(|| refused("that device is gone"))
    }

    pub fn set_powered(&self, on: bool) -> Result<(), Error> {
        let path = self.adapter_path()?;
        Ok(self
            .proxy(&self.interactive, &path, ADAPTER_IFACE)?
            .set_property("Powered", on)?)
    }

    /// Whether other devices can find this one (BlueZ turns it off again
    /// after its discoverable timeout).
    pub fn set_discoverable(&self, on: bool) -> Result<(), Error> {
        let path = self.adapter_path()?;
        Ok(self
            .proxy(&self.interactive, &path, ADAPTER_IFACE)?
            .set_property("Discoverable", on)?)
    }

    /// Starts looking for devices. Not an error when it already does.
    pub fn start_discovery(&self) -> Result<(), Error> {
        let path = self.adapter_path()?;
        match self
            .proxy(&self.conn, &path, ADAPTER_IFACE)?
            .call::<_, _, ()>("StartDiscovery", &())
        {
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str() == "org.bluez.Error.InProgress" =>
            {
                Ok(())
            }
            r => Ok(r?),
        }
    }

    /// Stops looking. Not an error when it wasn't.
    pub fn stop_discovery(&self) -> Result<(), Error> {
        let path = self.adapter_path()?;
        match self
            .proxy(&self.conn, &path, ADAPTER_IFACE)?
            .call::<_, _, ()>("StopDiscovery", &())
        {
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str() == "org.bluez.Error.Failed"
                    || name.as_str() == "org.bluez.Error.NotReady" =>
            {
                Ok(())
            }
            r => Ok(r?),
        }
    }

    pub fn connect(&self, address: &str) -> Result<(), Error> {
        let path = self.device_path(address)?;
        Ok(self
            .proxy(&self.interactive, &path, DEVICE_IFACE)?
            .call("Connect", &())?)
    }

    pub fn disconnect(&self, address: &str) -> Result<(), Error> {
        let path = self.device_path(address)?;
        Ok(self
            .proxy(&self.interactive, &path, DEVICE_IFACE)?
            .call("Disconnect", &())?)
    }

    /// Pairs with the device, trusts it (so it reconnects by itself) and
    /// connects to it.
    pub fn pair(&self, address: &str) -> Result<(), Error> {
        let path = self.device_path(address)?;
        let d = self.proxy(&self.interactive, &path, DEVICE_IFACE)?;
        d.call::<_, _, ()>("Pair", &())?;
        d.set_property("Trusted", true)?;
        d.call::<_, _, ()>("Connect", &())?;
        Ok(())
    }

    /// Removes the device (unpairs it).
    pub fn forget(&self, address: &str) -> Result<(), Error> {
        let path = self.device_path(address)?;
        let adapter = self.adapter_path()?;
        let dev = OwnedObjectPath::try_from(path.as_str()).map_err(|e| refused(e.to_string()))?;
        Ok(self
            .proxy(&self.interactive, &adapter, ADAPTER_IFACE)?
            .call("RemoveDevice", &(dev,))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert!(valid_address("AA:BB:CC:DD:EE:FF"));
        assert!(valid_address("aa:bb:cc:dd:ee:0f"));
        for bad in ["", "AA:BB:CC:DD:EE", "AA-BB-CC-DD-EE-FF", "AA:BB:CC:DD:EE:GG", "../x"] {
            assert!(!valid_address(bad), "{bad:?}");
        }
    }

    #[test]
    fn icons() {
        assert_eq!(Kind::from_icon("audio-headset"), Kind::Headphones);
        assert_eq!(Kind::from_icon("input-keyboard"), Kind::Keyboard);
        assert_eq!(Kind::from_icon("something-new"), Kind::Other);
    }
}
