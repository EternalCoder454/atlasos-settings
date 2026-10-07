//! Privacy & Security's backend: the firewall (firewalld, and systemd for
//! turning it on and off) and Atlas's crash report setting (the same file
//! Atlas Updater writes, through atlas-framework-system). The screen
//! lock's `kscreenlockerrc` and the location agent's autostart entry are
//! KConfig on the C++ side. Lives only while the page is shown; every call
//! runs on a worker thread.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        /// The first read has answered (or failed).
        #[qproperty(bool, loaded)]
        /// A change is under way.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        /// The firewall is installed.
        #[qproperty(bool, fw_installed, cxx_name = "fwInstalled")]
        #[qproperty(bool, fw_running, cxx_name = "fwRunning")]
        /// The zone the rules are for.
        #[qproperty(QString, fw_zone, cxx_name = "fwZone")]
        /// The services allowed in the zone (firewalld's names).
        #[qproperty(QStringList, fw_services, cxx_name = "fwServices")]
        /// The ports allowed, as "8080/tcp" or "8000-8100/udp".
        #[qproperty(QStringList, fw_ports, cxx_name = "fwPorts")]
        /// Every service firewalld knows.
        #[qproperty(QStringList, fw_available, cxx_name = "fwAvailable")]
        /// Crash reports are on.
        #[qproperty(bool, crash_enabled, cxx_name = "crashEnabled")]
        /// A crash report server is set up (reports can be sent).
        #[qproperty(bool, crash_has_server, cxx_name = "crashHasServer")]
        #[namespace = "atlas_settings"]
        type PrivacyPage = super::PrivacyPageRust;
    }

    extern "RustQt" {
        /// Reads everything again.
        #[qinvokable]
        fn refresh(self: Pin<&mut PrivacyPage>);

        /// The firewall on or off.
        #[qinvokable]
        #[cxx_name = "changeFirewall"]
        fn change_firewall(self: Pin<&mut PrivacyPage>, on: bool);

        #[qinvokable]
        #[cxx_name = "addService"]
        fn add_service(self: Pin<&mut PrivacyPage>, service: &QString);

        #[qinvokable]
        #[cxx_name = "removeService"]
        fn remove_service(self: Pin<&mut PrivacyPage>, service: &QString);

        /// `port` as "8080/tcp".
        #[qinvokable]
        #[cxx_name = "addPort"]
        fn add_port(self: Pin<&mut PrivacyPage>, port: &QString);

        #[qinvokable]
        #[cxx_name = "removePort"]
        fn remove_port(self: Pin<&mut PrivacyPage>, port: &QString);

        #[qinvokable]
        #[cxx_name = "changeCrashReports"]
        fn change_crash_reports(self: Pin<&mut PrivacyPage>, on: bool);

        /// Whether `text` is a port ("8080" or "8000-8100").
        #[qinvokable]
        #[cxx_name = "validPort"]
        fn valid_port(self: &PrivacyPage, text: &QString) -> bool;
    }

    impl cxx_qt::Threading for PrivacyPage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn privacy_page_make_unique() -> UniquePtr<PrivacyPage>;
    }
}

use crate::worker;
use atlas_framework_ui::atlas_framework_system::crash;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use settings_sys::firewall::{self, Firewall, Port, Rules, Status};
use settings_sys::{Bus, Error, ErrorKind, updater};

#[derive(Default)]
pub struct PrivacyPageRust {
    loaded: bool,
    busy: bool,
    error: QString,
    fw_installed: bool,
    fw_running: bool,
    fw_zone: QString,
    fw_services: QStringList,
    fw_ports: QStringList,
    fw_available: QStringList,
    crash_enabled: bool,
    crash_has_server: bool,
}

struct Read {
    firewall: Result<(Status, Option<Rules>), Error>,
    crash_enabled: bool,
    crash_has_server: bool,
}

fn read() -> Read {
    let firewall = Firewall::new(&Bus::System).and_then(|f| {
        let status = f.status()?;
        let rules = if status.running { f.rules()? } else { None };
        Ok((status, rules))
    });
    Read {
        firewall,
        crash_enabled: crash::Settings::load().enabled,
        crash_has_server: crash::Endpoint::load().is_some(),
    }
}

fn qs(s: &str) -> QString {
    QString::from(s)
}

fn list<I: IntoIterator<Item = String>>(items: I) -> QStringList {
    let mut l = QStringList::default();
    for i in items {
        l.append(qs(&i));
    }
    l
}

/// "8080/tcp" as a port, `None` for anything else.
fn parse_port(text: &str) -> Option<Port> {
    let (port, protocol) = text.split_once('/')?;
    (firewall::valid_port(port) && firewall::valid_protocol(protocol)).then(|| Port {
        port: port.to_string(),
        protocol: protocol.to_string(),
    })
}

/// Saves the crash report setting as Atlas Updater does (the framework's
/// own file, with what it does when turning reports on or off).
fn save_crash_reports(on: bool) -> std::io::Result<()> {
    crash::Settings { enabled: on }.save()
}

impl qobject::PrivacyPage {
    fn show(mut self: Pin<&mut Self>, read: Option<Read>) {
        self.as_mut().set_loaded(true);
        let Some(read) = read else {
            self.as_mut()
                .set_error(qs("Settings couldn't read the privacy settings."));
            return;
        };
        self.as_mut().set_crash_enabled(read.crash_enabled);
        self.as_mut().set_crash_has_server(read.crash_has_server);
        let mut problem = None;
        match read.firewall {
            Ok((status, rules)) => {
                self.as_mut().set_fw_installed(status.installed);
                self.as_mut().set_fw_running(status.running);
                let rules = rules.unwrap_or(Rules {
                    zone: String::new(),
                    services: Vec::new(),
                    ports: Vec::new(),
                    available: Vec::new(),
                });
                self.as_mut().set_fw_zone(qs(&rules.zone));
                self.as_mut().set_fw_services(list(rules.services));
                self.as_mut().set_fw_ports(list(
                    rules
                        .ports
                        .into_iter()
                        .map(|p| format!("{}/{}", p.port, p.protocol)),
                ));
                self.as_mut().set_fw_available(list(rules.available));
            }
            Err(e) => {
                self.as_mut().set_fw_installed(false);
                self.as_mut().set_fw_running(false);
                if e.kind != ErrorKind::NotRunning {
                    problem = Some(worker::describe("reading the firewall", &e));
                }
            }
        }
        self.as_mut()
            .set_error(qs(problem.unwrap_or_default().as_str()));
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(qt, "privacy-read", read, |o, r| o.show(r));
    }

    /// Runs change `job`, then reads everything again.
    fn change<J>(mut self: Pin<&mut Self>, what: &'static str, job: J)
    where
        J: FnOnce() -> Result<(), Error> + Send + 'static,
    {
        if self.rust().busy {
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "privacy-change",
            move || {
                let r = job();
                (r, read())
            },
            move |mut o, r| {
                o.as_mut().set_busy(false);
                match r {
                    Some((result, read)) => {
                        o.as_mut().show(Some(read));
                        if let Err(e) = result {
                            let text = worker::describe(what, &e);
                            o.as_mut().set_error(qs(&text));
                        }
                    }
                    None => o.as_mut().show(None),
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
        }
    }

    pub fn change_firewall(self: Pin<&mut Self>, on: bool) {
        self.change("turning the firewall on or off", move || {
            Firewall::new(&Bus::System)?.set_enabled(on)
        });
    }

    pub fn add_service(self: Pin<&mut Self>, service: &QString) {
        let service = service.to_string();
        self.change("allowing the app", move || {
            Firewall::new(&Bus::System)?.add_service(&service)
        });
    }

    pub fn remove_service(self: Pin<&mut Self>, service: &QString) {
        let service = service.to_string();
        self.change("blocking the app", move || {
            Firewall::new(&Bus::System)?.remove_service(&service)
        });
    }

    pub fn add_port(self: Pin<&mut Self>, port: &QString) {
        let text = port.to_string();
        self.change("allowing the port", move || {
            let port =
                parse_port(&text).ok_or_else(|| Error::new(ErrorKind::Refused, "not a port"))?;
            Firewall::new(&Bus::System)?.add_port(&port)
        });
    }

    pub fn remove_port(self: Pin<&mut Self>, port: &QString) {
        let text = port.to_string();
        self.change("blocking the port", move || {
            let port =
                parse_port(&text).ok_or_else(|| Error::new(ErrorKind::Refused, "not a port"))?;
            Firewall::new(&Bus::System)?.remove_port(&port)
        });
    }

    pub fn change_crash_reports(mut self: Pin<&mut Self>, on: bool) {
        if self.rust().busy {
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "privacy-crash",
            move || {
                let saved = save_crash_reports(on);
                if saved.is_ok() {
                    // Updater's tray watches for crashes only while this is
                    // on: tell it, as Updater's own window does. It not
                    // answering is not a failure: the setting is saved and
                    // it reads it when it starts.
                    if let Err(e) = updater::reload_tray(&Bus::Session) {
                        log::warn!(
                            "telling Updater's tray about the crash setting: {}",
                            e.detail
                        );
                    }
                }
                (saved.map_err(|e| e.to_string()), read())
            },
            move |mut o, r| {
                o.as_mut().set_busy(false);
                match r {
                    Some((result, read)) => {
                        o.as_mut().show(Some(read));
                        if let Err(e) = result {
                            log::warn!("saving the crash report setting: {e}");
                            o.as_mut()
                                .set_error(qs("Settings couldn't save the crash report setting."));
                        }
                    }
                    None => o.as_mut().show(None),
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
        }
    }

    pub fn valid_port(&self, text: &QString) -> bool {
        firewall::valid_port(&text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_as_the_page_writes_them() {
        assert_eq!(
            parse_port("8080/tcp"),
            Some(Port {
                port: "8080".into(),
                protocol: "tcp".into()
            })
        );
        assert!(parse_port("8000-8100/udp").is_some());
        for bad in [
            "8080",
            "8080/icmp",
            "0/tcp",
            "/tcp",
            "80/tcp/x",
            "a/tcp",
            "",
        ] {
            assert!(parse_port(bad).is_none(), "{bad:?}");
        }
    }

    /// The crash report setting is the framework's own file, in the user's
    /// config folder, which Updater reads and writes too.
    #[test]
    fn crash_reports_are_written_where_updater_and_the_framework_read_them() {
        let dir = std::env::temp_dir().join(format!("settings-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        // SAFETY: the only test that touches the environment; it runs
        // alone in this process's test threads for these variables.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        }
        let file = dir.join("config/atlas/crash-reporting.toml");

        assert!(!crash::Settings::load().enabled, "off until turned on");
        save_crash_reports(true).expect("on");
        let text = std::fs::read_to_string(&file).expect("the file");
        assert!(text.lines().any(|l| l == "enabled = true"), "{text}");
        assert!(crash::Settings::load().enabled);

        save_crash_reports(false).expect("off");
        let text = std::fs::read_to_string(&file).expect("the file");
        assert!(text.lines().any(|l| l == "enabled = false"), "{text}");
        assert!(!crash::Settings::load().enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
