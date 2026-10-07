//! More helpers for talking to python-dbusmock's own interface from a test
//! (properties and methods on mocked objects), beside `common`.

#![allow(dead_code)]

use crate::common::MockBus;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value};

fn connect(bus: &MockBus) -> Connection {
    zbus::blocking::connection::Builder::address(bus.address.as_str())
        .and_then(|b| b.build())
        .expect("connect to the test bus")
}

/// Calls `method` of `iface` on the object, with `body` as its arguments.
pub fn call<B>(bus: &MockBus, dest: &str, path: &str, iface: &str, method: &str, body: &B)
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    connect(bus)
        .call_method(Some(dest), path, Some(iface), method, body)
        .unwrap_or_else(|e| panic!("{method}: {e}"));
}

/// A generic python-dbusmock service (`name`, one object at `path` with
/// `iface`) on the test bus, stopped when this is dropped.
pub struct Generic(std::process::Child);

pub fn generic(bus: &MockBus, name: &str, path: &str, iface: &str) -> Generic {
    let child = std::process::Command::new("python3")
        .args(["-m", "dbusmock", "--system", name, path, iface])
        .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("python3 -m dbusmock");
    let conn = connect(bus);
    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).expect("DBus proxy");
    let bus_name = zbus::names::BusName::try_from(name).expect("bus name");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !dbus.name_has_owner(bus_name.clone()).unwrap_or(false) {
        assert!(
            std::time::Instant::now() < deadline,
            "{name} never appeared"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Generic(child)
}

impl Drop for Generic {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Adds property `name` with `value` to `iface` of the mocked object.
pub fn add_property<'v, V>(bus: &MockBus, dest: &str, path: &str, iface: &str, name: &str, value: V)
where
    V: Into<Value<'v>>,
{
    connect(bus)
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Mock"),
            "AddProperty",
            &(iface, name, value.into()),
        )
        .expect("AddProperty");
}

/// Sets an existing property of a mocked object.
pub fn set_property<'v, V>(bus: &MockBus, dest: &str, path: &str, iface: &str, name: &str, value: V)
where
    V: Into<Value<'v>>,
{
    connect(bus)
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &(iface, name, value.into()),
        )
        .expect("Set");
}

/// Reads a property of a mocked object.
pub fn get_property(bus: &MockBus, dest: &str, path: &str, iface: &str, name: &str) -> OwnedValue {
    let reply = connect(bus)
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(iface, name),
        )
        .expect("Get");
    reply.body().deserialize().expect("variant")
}

/// Adds a method to any object of a mock (`code` answers it, in Python).
pub fn add_method(
    bus: &MockBus,
    dest: &str,
    path: &str,
    iface: &str,
    method: &str,
    sigs: (&str, &str),
    code: &str,
) {
    connect(bus)
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Mock"),
            "AddMethod",
            &(iface, method, sigs.0, sigs.1, code),
        )
        .expect("AddMethod");
}

/// Everything a mocked object logged about its calls, names and arguments,
/// as text.
pub fn call_log(bus: &MockBus, dest: &str, path: &str) -> String {
    let reply = connect(bus)
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Mock"),
            "GetCalls",
            &(),
        )
        .expect("GetCalls");
    let list: Vec<(u64, String, Vec<OwnedValue>)> = reply.body().deserialize().expect("calls");
    format!("{list:?}")
}

/// The names of the methods a mocked object has been called with, in order.
pub fn calls(bus: &MockBus, dest: &str, path: &str) -> Vec<String> {
    let reply = connect(bus)
        .call_method(
            Some(dest),
            path,
            Some("org.freedesktop.DBus.Mock"),
            "GetCalls",
            &(),
        )
        .expect("GetCalls");
    let list: Vec<(u64, String, Vec<OwnedValue>)> = reply.body().deserialize().expect("calls");
    list.into_iter().map(|(_, m, _)| m).collect()
}
