//! Network's backend: Wi-Fi, wired status, VPN, the hotspot and airplane
//! mode (NetworkManager, through `settings_sys::network`). Home reuses it
//! for its Wi-Fi switch. Lives only while the page is shown; every call
//! runs on a worker thread. Passwords are passed on to NetworkManager and
//! never kept.

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
        /// NetworkManager answered the last read.
        #[qproperty(bool, available)]
        /// A change is under way.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        #[qproperty(bool, wifi_present, cxx_name = "wifiPresent")]
        #[qproperty(bool, wifi_enabled, cxx_name = "wifiEnabled")]
        /// The adapter's own switch is off.
        #[qproperty(bool, wifi_blocked, cxx_name = "wifiBlocked")]
        #[qproperty(bool, airplane)]
        /// The Wi-Fi network in use; "" for none.
        #[qproperty(QString, wifi_ssid, cxx_name = "wifiSsid")]
        #[qproperty(bool, wifi_connecting, cxx_name = "wifiConnecting")]
        /// The network being joined by `connectTo`; "" for none.
        #[qproperty(QString, joining)]
        /// "", "unplugged", "disconnected", "connecting" or "connected".
        #[qproperty(QString, wired)]
        #[qproperty(QString, wired_interface, cxx_name = "wiredInterface")]
        /// JSON: [{ssid, signal, security, connected, connecting, known}].
        #[qproperty(QString, networks)]
        /// JSON: [{id, uuid, active, connecting}].
        #[qproperty(QString, vpns)]
        #[qproperty(bool, hotspot_saved, cxx_name = "hotspotSaved")]
        #[qproperty(QString, hotspot_name, cxx_name = "hotspotName")]
        #[qproperty(bool, hotspot_active, cxx_name = "hotspotActive")]
        #[namespace = "telamon_settings"]
        type NetworkPage = super::NetworkPageRust;
    }

    extern "RustQt" {
        /// Reads everything again, with the networks in range.
        #[qinvokable]
        fn refresh(self: Pin<&mut NetworkPage>);

        /// Reads the state without the list of networks (Home).
        #[qinvokable]
        #[cxx_name = "refreshStatus"]
        fn refresh_status(self: Pin<&mut NetworkPage>);

        /// Asks the adapter to look for networks again.
        #[qinvokable]
        fn scan(self: Pin<&mut NetworkPage>);

        #[qinvokable]
        #[cxx_name = "changeWifi"]
        fn change_wifi(self: Pin<&mut NetworkPage>, on: bool);

        #[qinvokable]
        #[cxx_name = "changeAirplane"]
        fn change_airplane(self: Pin<&mut NetworkPage>, on: bool);

        /// Joins `ssid`: with its saved connection when `password` is empty
        /// and there is one, else with the password.
        #[qinvokable]
        #[cxx_name = "connectTo"]
        fn connect_to(self: Pin<&mut NetworkPage>, ssid: &QString, password: &QString);

        #[qinvokable]
        fn disconnect(self: Pin<&mut NetworkPage>);

        #[qinvokable]
        fn forget(self: Pin<&mut NetworkPage>, ssid: &QString);

        #[qinvokable]
        #[cxx_name = "changeVpn"]
        fn change_vpn(self: Pin<&mut NetworkPage>, uuid: &QString, on: bool);

        /// Starts the hotspot: the saved one when `password` is empty.
        #[qinvokable]
        #[cxx_name = "startHotspot"]
        fn start_hotspot(self: Pin<&mut NetworkPage>, name: &QString, password: &QString);

        #[qinvokable]
        #[cxx_name = "stopHotspot"]
        fn stop_hotspot(self: Pin<&mut NetworkPage>);

        #[qinvokable]
        #[cxx_name = "forgetHotspot"]
        fn forget_hotspot(self: Pin<&mut NetworkPage>);

        /// Whether `password` can be a password for a network of this
        /// security ("open", "wep", "personal", "sae").
        #[qinvokable]
        #[cxx_name = "validPassword"]
        fn valid_password(self: &NetworkPage, security: &QString, password: &QString) -> bool;

        /// Whether `name` can be the name of a hotspot.
        #[qinvokable]
        #[cxx_name = "validHotspotName"]
        fn valid_hotspot_name(self: &NetworkPage, name: &QString) -> bool;

        /// A new random password for a hotspot, easy to read out.
        #[qinvokable]
        #[cxx_name = "randomPassword"]
        fn random_password(self: &NetworkPage) -> QString;
    }

    impl cxx_qt::Threading for NetworkPage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn network_page_make_unique() -> UniquePtr<NetworkPage>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use settings_sys::network::{self, Link, Network, Secret, Security, Status, WifiNetwork};
use settings_sys::{Error, ErrorKind};

#[derive(Default)]
pub struct NetworkPageRust {
    loaded: bool,
    available: bool,
    busy: bool,
    error: QString,
    wifi_present: bool,
    wifi_enabled: bool,
    wifi_blocked: bool,
    airplane: bool,
    wifi_ssid: QString,
    wifi_connecting: bool,
    joining: QString,
    wired: QString,
    wired_interface: QString,
    networks: QString,
    vpns: QString,
    hotspot_saved: bool,
    hotspot_name: QString,
    hotspot_active: bool,
    /// A read is on its way: another one is not started.
    reading: bool,
}

/// What a read gives. The networks are `None` when only the state was
/// asked for; Bluetooth is read for airplane mode.
struct Read {
    status: Result<Status, Error>,
    networks: Option<Vec<WifiNetwork>>,
    bluetooth_on: bool,
}

fn read(with_networks: bool) -> Read {
    let bus = crate::support::system();
    let net = Network::new(&bus);
    let status = net.as_ref().map_err(Clone::clone).and_then(Network::status);
    let networks = if with_networks && status.is_ok() {
        net.as_ref().ok().and_then(|n| n.wifi_networks().ok())
    } else {
        None
    };
    let bluetooth_on = settings_sys::bluetooth::Bluetooth::new(&bus)
        .and_then(|b| b.snapshot())
        .ok()
        .and_then(|s| s.adapter)
        .is_some_and(|a| a.powered);
    Read {
        status,
        networks,
        bluetooth_on,
    }
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn networks_json(list: &[WifiNetwork]) -> String {
    json!(
        list.iter()
            .map(|n| json!({
                "ssid": n.ssid,
                "signal": n.signal,
                "security": n.security.as_str(),
                "connected": n.connected,
                "connecting": n.connecting,
                "known": n.known,
            }))
            .collect::<Vec<_>>()
    )
    .to_string()
}

fn parse_security(s: &str) -> Security {
    match s {
        "wep" => Security::Wep,
        "personal" => Security::Personal,
        "sae" => Security::Sae,
        "enterprise" => Security::Enterprise,
        _ => Security::Open,
    }
}

/// What to say when joining `ssid` failed.
fn join_failure(ssid: &str, with_password: bool, e: &Error) -> String {
    log::warn!("joining {ssid}: {}", e.detail);
    match e.kind {
        ErrorKind::Denied | ErrorKind::NotRunning | ErrorKind::Bus => e.describe().to_string(),
        ErrorKind::Timeout => format!("“{ssid}” didn't answer in time."),
        ErrorKind::Refused if with_password => {
            format!("Couldn't join “{ssid}”. Check the password and try again.")
        }
        ErrorKind::Refused => format!(
            "Couldn't join “{ssid}”. If its password changed, forget the network and join it again."
        ),
    }
}

impl qobject::NetworkPage {
    fn show(mut self: Pin<&mut Self>, read: Option<Read>) {
        self.as_mut().rust_mut().reading = false;
        self.as_mut().set_loaded(true);
        let Some(Read {
            status,
            networks,
            bluetooth_on,
        }) = read
        else {
            self.as_mut()
                .set_error(q("Settings couldn't read the network settings."));
            return;
        };
        match status {
            Ok(s) => {
                self.as_mut().set_available(true);
                self.as_mut().set_wifi_present(s.wifi_present);
                self.as_mut().set_wifi_enabled(s.wifi_enabled);
                self.as_mut().set_wifi_blocked(s.wifi_blocked);
                self.as_mut()
                    .set_airplane(!s.wifi_enabled && !s.wwan_enabled && !bluetooth_on);
                self.as_mut()
                    .set_wifi_ssid(q(s.wifi_ssid.as_deref().unwrap_or("")));
                self.as_mut().set_wifi_connecting(s.wifi_connecting);
                let (wired, iface) = match &s.wired {
                    None => ("", ""),
                    Some(w) => (
                        match w.link {
                            Link::Unplugged => "unplugged",
                            Link::Disconnected => "disconnected",
                            Link::Connecting => "connecting",
                            Link::Connected => "connected",
                        },
                        w.interface.as_str(),
                    ),
                };
                self.as_mut().set_wired(q(wired));
                self.as_mut().set_wired_interface(q(iface));
                let vpns = json!(
                    s.vpns
                        .iter()
                        .map(|v| json!({
                            "id": v.id, "uuid": v.uuid,
                            "active": v.active, "connecting": v.connecting,
                        }))
                        .collect::<Vec<_>>()
                );
                self.as_mut().set_vpns(q(&vpns.to_string()));
                self.as_mut().set_hotspot_saved(s.hotspot.is_some());
                self.as_mut()
                    .set_hotspot_active(s.hotspot.as_ref().is_some_and(|h| h.active));
                self.as_mut()
                    .set_hotspot_name(q(s.hotspot.as_ref().map_or("", |h| h.ssid.as_str())));
                if let Some(list) = networks {
                    self.as_mut().set_networks(q(&networks_json(&list)));
                }
            }
            Err(e) => {
                self.as_mut().set_available(false);
                let text = worker::describe("reading NetworkManager", &e);
                self.as_mut().set_error(q(&text));
            }
        }
    }

    fn start_read(mut self: Pin<&mut Self>, with_networks: bool) {
        if self.rust().reading {
            return;
        }
        self.as_mut().rust_mut().reading = true;
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "network-read",
            move || read(with_networks),
            |mut o, r| {
                // Not while a change is under way: its own answer is newer.
                if o.rust().busy {
                    o.as_mut().rust_mut().reading = false;
                } else {
                    o.as_mut().show(r);
                }
            },
        );
        if !started {
            self.as_mut().rust_mut().reading = false;
        }
    }

    pub fn refresh(self: Pin<&mut Self>) {
        self.start_read(true);
    }

    pub fn refresh_status(self: Pin<&mut Self>) {
        self.start_read(false);
    }

    pub fn scan(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(
            qt,
            "network-scan",
            || {
                if let Ok(n) = Network::new(&crate::support::system()) {
                    n.scan();
                }
            },
            |_, _| {},
        );
    }

    /// Runs change `job`, then reads everything again. `fail` words a
    /// failure.
    fn change<J, F>(mut self: Pin<&mut Self>, joining: &str, job: J, fail: F)
    where
        J: FnOnce() -> Result<(), Error> + Send + 'static,
        F: FnOnce(&Error) -> String + Send + 'static,
    {
        if self.rust().busy {
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        self.as_mut().set_joining(q(joining));
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "network-change",
            move || {
                let r = job();
                (r, read(true))
            },
            move |mut o, r| {
                o.as_mut().set_busy(false);
                o.as_mut().set_joining(QString::default());
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
            self.as_mut().set_joining(QString::default());
        }
    }

    fn plain(what: &'static str) -> impl FnOnce(&Error) -> String + Send + 'static {
        move |e| worker::describe(what, e)
    }

    pub fn change_wifi(self: Pin<&mut Self>, on: bool) {
        self.change(
            "",
            move || Network::new(&crate::support::system())?.set_wifi_enabled(on),
            Self::plain("switching Wi-Fi"),
        );
    }

    pub fn change_airplane(self: Pin<&mut Self>, on: bool) {
        self.change(
            "",
            move || {
                let bus = crate::support::system();
                // Bluetooth first: where it can't be changed (no adapter)
                // the rest still is.
                if let Ok(bt) = settings_sys::bluetooth::Bluetooth::new(&bus)
                    && let Err(e) = bt.set_powered(!on)
                    && e.kind != ErrorKind::Refused
                {
                    log::info!("Bluetooth for airplane mode: {}", e.detail);
                }
                Network::new(&bus)?.set_radios_enabled(!on)
            },
            Self::plain("switching airplane mode"),
        );
    }

    pub fn connect_to(self: Pin<&mut Self>, ssid: &QString, password: &QString) {
        let ssid = ssid.to_string();
        let joining = ssid.clone();
        let named = ssid.clone();
        let pw = password.to_string();
        let with_password = !pw.is_empty();
        self.change(
            &joining,
            move || {
                let secret = Secret::new(pw);
                let net = Network::new(&crate::support::system())?;
                net.connect_wifi(&ssid, (!secret.is_empty()).then_some(&secret))
            },
            move |e| join_failure(&named, with_password, e),
        );
    }

    pub fn disconnect(self: Pin<&mut Self>) {
        self.change(
            "",
            || Network::new(&crate::support::system())?.disconnect_wifi(),
            Self::plain("leaving the network"),
        );
    }

    pub fn forget(self: Pin<&mut Self>, ssid: &QString) {
        let ssid = ssid.to_string();
        self.change(
            "",
            move || Network::new(&crate::support::system())?.forget_wifi(&ssid),
            Self::plain("forgetting the network"),
        );
    }

    pub fn change_vpn(self: Pin<&mut Self>, uuid: &QString, on: bool) {
        let uuid = uuid.to_string();
        self.change(
            "",
            move || Network::new(&crate::support::system())?.set_vpn(&uuid, on),
            move |e| {
                log::warn!("VPN: {}", e.detail);
                if on && e.kind == ErrorKind::Refused {
                    "The VPN didn't connect. Check its settings in Plasma.".to_string()
                } else {
                    e.describe().to_string()
                }
            },
        );
    }

    pub fn start_hotspot(self: Pin<&mut Self>, name: &QString, password: &QString) {
        let name = name.to_string();
        let pw = password.to_string();
        self.change(
            "",
            move || {
                let secret = Secret::new(pw);
                Network::new(&crate::support::system())?
                    .start_hotspot(&name, (!secret.is_empty()).then_some(&secret))
            },
            |e| {
                log::warn!("hotspot: {}", e.detail);
                if e.kind == ErrorKind::Refused {
                    "The hotspot didn't start. This computer's Wi-Fi may not be able to share a connection."
                        .to_string()
                } else {
                    e.describe().to_string()
                }
            },
        );
    }

    pub fn stop_hotspot(self: Pin<&mut Self>) {
        self.change(
            "",
            || Network::new(&crate::support::system())?.stop_hotspot(),
            Self::plain("stopping the hotspot"),
        );
    }

    pub fn forget_hotspot(self: Pin<&mut Self>) {
        self.change(
            "",
            || Network::new(&crate::support::system())?.forget_hotspot(),
            Self::plain("removing the hotspot"),
        );
    }

    pub fn valid_password(&self, security: &QString, password: &QString) -> bool {
        network::valid_password(parse_security(&security.to_string()), &password.to_string())
    }

    pub fn valid_hotspot_name(&self, name: &QString) -> bool {
        network::valid_ssid(&name.to_string())
    }

    pub fn random_password(&self) -> QString {
        q(&crate::support::random_password(12))
    }
}
