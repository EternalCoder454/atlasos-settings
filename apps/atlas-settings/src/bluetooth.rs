//! Bluetooth & Devices' backend: the adapter and its devices (BlueZ,
//! through `settings_sys::bluetooth`). Home reuses it for its Bluetooth
//! switch. Lives only while the page is shown; every call runs on a worker
//! thread. Looking for devices runs only while the page asks for it, and
//! stops when the page goes.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        /// The first read has answered (or failed).
        #[qproperty(bool, loaded)]
        /// BlueZ answered the last read.
        #[qproperty(bool, available)]
        /// A change is under way.
        #[qproperty(bool, busy)]
        /// The address of the device being paired, connected, ...; "" for
        /// none.
        #[qproperty(QString, working)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        #[qproperty(bool, adapter)]
        #[qproperty(bool, powered)]
        #[qproperty(bool, discoverable)]
        #[qproperty(bool, discovering)]
        /// The name other devices see this computer by.
        #[qproperty(QString, adapter_name, cxx_name = "adapterName")]
        /// JSON: [{address, name, kind, paired, connected, battery}], the
        /// battery -1 when unknown.
        #[qproperty(QString, devices)]
        /// How many devices are connected.
        #[qproperty(i32, connected_count, cxx_name = "connectedCount")]
        #[namespace = "atlas_settings"]
        type BluetoothPage = super::BluetoothPageRust;
    }

    extern "RustQt" {
        /// Reads everything again.
        #[qinvokable]
        fn refresh(self: Pin<&mut BluetoothPage>);

        #[qinvokable]
        #[cxx_name = "changePowered"]
        fn change_powered(self: Pin<&mut BluetoothPage>, on: bool);

        #[qinvokable]
        #[cxx_name = "changeDiscoverable"]
        fn change_discoverable(self: Pin<&mut BluetoothPage>, on: bool);

        /// Looks for devices until `stopDiscovery` or until the page goes.
        #[qinvokable]
        #[cxx_name = "startDiscovery"]
        fn start_discovery(self: Pin<&mut BluetoothPage>);

        #[qinvokable]
        #[cxx_name = "stopDiscovery"]
        fn stop_discovery(self: Pin<&mut BluetoothPage>);

        #[qinvokable]
        fn pair(self: Pin<&mut BluetoothPage>, address: &QString, name: &QString);

        #[qinvokable]
        #[cxx_name = "connectDevice"]
        fn connect_device(self: Pin<&mut BluetoothPage>, address: &QString, name: &QString);

        #[qinvokable]
        #[cxx_name = "disconnectDevice"]
        fn disconnect_device(self: Pin<&mut BluetoothPage>, address: &QString);

        /// Removes (unpairs) the device.
        #[qinvokable]
        fn forget(self: Pin<&mut BluetoothPage>, address: &QString);
    }

    impl cxx_qt::Threading for BluetoothPage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn bluetooth_page_make_unique() -> UniquePtr<BluetoothPage>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use settings_sys::bluetooth::{Bluetooth, Snapshot};
use settings_sys::{Error, ErrorKind};
use std::sync::mpsc;

#[derive(Default)]
pub struct BluetoothPageRust {
    loaded: bool,
    available: bool,
    busy: bool,
    working: QString,
    error: QString,
    adapter: bool,
    powered: bool,
    discoverable: bool,
    discovering: bool,
    adapter_name: QString,
    devices: QString,
    connected_count: i32,
    /// A read is on its way: another one is not started.
    reading: bool,
    /// The thread that looks for devices; dropping this (the page going, or
    /// `stopDiscovery`) ends it, and BlueZ stops with the connection.
    discovery: Option<mpsc::Sender<()>>,
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn read() -> Result<Snapshot, Error> {
    Bluetooth::new(&crate::support::system())?.snapshot()
}

fn devices_json(s: &Snapshot) -> String {
    json!(
        s.devices
            .iter()
            .map(|d| json!({
                "address": d.address,
                "name": d.name,
                "kind": d.kind.as_str(),
                "paired": d.paired,
                "connected": d.connected,
                "battery": d.battery.map_or(-1, i32::from),
            }))
            .collect::<Vec<_>>()
    )
    .to_string()
}

impl qobject::BluetoothPage {
    fn show(mut self: Pin<&mut Self>, read: Option<Result<Snapshot, Error>>) {
        self.as_mut().rust_mut().reading = false;
        self.as_mut().set_loaded(true);
        let Some(read) = read else {
            self.as_mut()
                .set_error(q("Settings couldn't read the Bluetooth settings."));
            return;
        };
        match read {
            Ok(s) => {
                self.as_mut().set_available(true);
                self.as_mut().set_adapter(s.adapter.is_some());
                let a = s.adapter.as_ref();
                self.as_mut().set_powered(a.is_some_and(|a| a.powered));
                self.as_mut()
                    .set_discoverable(a.is_some_and(|a| a.discoverable));
                self.as_mut()
                    .set_discovering(a.is_some_and(|a| a.discovering));
                self.as_mut()
                    .set_adapter_name(q(a.map_or("", |a| a.name.as_str())));
                self.as_mut().set_devices(q(&devices_json(&s)));
                let connected = s.devices.iter().filter(|d| d.connected).count();
                self.as_mut()
                    .set_connected_count(i32::try_from(connected).unwrap_or(i32::MAX));
            }
            Err(e) => {
                self.as_mut().set_available(false);
                let text = worker::describe("reading BlueZ", &e);
                self.as_mut().set_error(q(&text));
            }
        }
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        if self.rust().reading {
            return;
        }
        self.as_mut().rust_mut().reading = true;
        let qt = self.qt_thread();
        let started = worker::run(qt, "bluetooth-read", read, |mut o, r| {
            // Not while a change is under way: its own answer is newer.
            if o.rust().busy {
                o.as_mut().rust_mut().reading = false;
            } else {
                o.as_mut().show(r);
            }
        });
        if !started {
            self.as_mut().rust_mut().reading = false;
        }
    }

    /// Runs change `job`, then reads again. `fail` words a failure.
    fn change<J, F>(mut self: Pin<&mut Self>, working: &str, job: J, fail: F)
    where
        J: FnOnce() -> Result<(), Error> + Send + 'static,
        F: FnOnce(&Error) -> String + Send + 'static,
    {
        if self.rust().busy {
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        self.as_mut().set_working(q(working));
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "bluetooth-change",
            move || {
                let r = job();
                (r, read())
            },
            move |mut o, r| {
                o.as_mut().set_busy(false);
                o.as_mut().set_working(QString::default());
                match r {
                    Some((result, read)) => {
                        o.as_mut().show(Some(read));
                        if let Err(e) = result {
                            let text = fail(&e);
                            o.as_mut().set_error(q(&text));
                        }
                    }
                    None => o.as_mut().show(None),
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
            self.as_mut().set_working(QString::default());
        }
    }

    fn plain(what: &'static str) -> impl FnOnce(&Error) -> String + Send + 'static {
        move |e| worker::describe(what, e)
    }

    pub fn change_powered(self: Pin<&mut Self>, on: bool) {
        self.change(
            "",
            move || Bluetooth::new(&crate::support::system())?.set_powered(on),
            |e| {
                log::warn!("Bluetooth power: {}", e.detail);
                if e.kind == ErrorKind::Refused {
                    "Bluetooth couldn't be switched. It may be turned off by a switch on this computer."
                        .to_string()
                } else {
                    e.describe().to_string()
                }
            },
        );
    }

    pub fn change_discoverable(self: Pin<&mut Self>, on: bool) {
        self.change(
            "",
            move || Bluetooth::new(&crate::support::system())?.set_discoverable(on),
            Self::plain("changing visibility"),
        );
    }

    pub fn start_discovery(mut self: Pin<&mut Self>) {
        if self.rust().discovery.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel::<()>();
        let spawned = std::thread::Builder::new()
            .name("bluetooth-discovery".into())
            .spawn(move || {
                // BlueZ looks for devices for as long as this connection
                // lives, so it is kept here until the page lets go.
                let Ok(bt) = Bluetooth::new(&crate::support::system()) else {
                    return;
                };
                if let Err(e) = bt.start_discovery() {
                    log::info!("looking for devices: {}", e.detail);
                    return;
                }
                // Returns when the sender is dropped.
                let _ = rx.recv();
                if let Err(e) = bt.stop_discovery() {
                    log::info!("stopping the search: {}", e.detail);
                }
            });
        match spawned {
            Ok(_) => self.as_mut().rust_mut().discovery = Some(tx),
            Err(e) => log::error!("starting the device search: {e}"),
        }
    }

    pub fn stop_discovery(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().discovery = None;
    }

    pub fn pair(self: Pin<&mut Self>, address: &QString, name: &QString) {
        let address = address.to_string();
        let working = address.clone();
        let name = name.to_string();
        self.change(
            &working,
            move || Bluetooth::new(&crate::support::system())?.pair(&address),
            move |e| {
                log::warn!("pairing: {}", e.detail);
                match e.kind {
                    ErrorKind::Refused | ErrorKind::Timeout => format!(
                        "Couldn't pair with “{name}”. Make sure it is in pairing mode and try again."
                    ),
                    _ => e.describe().to_string(),
                }
            },
        );
    }

    pub fn connect_device(self: Pin<&mut Self>, address: &QString, name: &QString) {
        let address = address.to_string();
        let working = address.clone();
        let name = name.to_string();
        self.change(
            &working,
            move || Bluetooth::new(&crate::support::system())?.connect(&address),
            move |e| {
                log::warn!("connecting: {}", e.detail);
                match e.kind {
                    ErrorKind::Refused | ErrorKind::Timeout => {
                        format!(
                            "Couldn't connect to “{name}”. Make sure it is turned on and in range."
                        )
                    }
                    _ => e.describe().to_string(),
                }
            },
        );
    }

    pub fn disconnect_device(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        let working = address.clone();
        self.change(
            &working,
            move || Bluetooth::new(&crate::support::system())?.disconnect(&address),
            Self::plain("disconnecting"),
        );
    }

    pub fn forget(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        let working = address.clone();
        self.change(
            &working,
            move || Bluetooth::new(&crate::support::system())?.forget(&address),
            Self::plain("removing the device"),
        );
    }
}
