//! System's backend: the device name (hostnamed) and what the device is
//! (`settings_sys::sysinfo`). Lives only while the page is shown; every
//! call runs on a worker thread.

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
        /// A rename is under way.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        /// The name people see; "" when unknown.
        #[qproperty(QString, device_name, cxx_name = "deviceName")]
        /// The name on the network.
        #[qproperty(QString, hostname)]
        #[qproperty(QString, chassis)]
        #[qproperty(QString, os)]
        #[qproperty(QString, os_build, cxx_name = "osBuild")]
        #[qproperty(QString, cpu)]
        /// Bytes; 0 when unknown.
        #[qproperty(f64, memory)]
        #[qproperty(QString, graphics)]
        #[namespace = "telamon_settings"]
        type SystemInfo = super::SystemInfoRust;
    }

    extern "RustQt" {
        /// Reads everything again.
        #[qinvokable]
        fn refresh(self: Pin<&mut SystemInfo>);

        /// Renames the device; polkit may ask for a password.
        #[qinvokable]
        #[cxx_name = "renameDevice"]
        fn rename_device(self: Pin<&mut SystemInfo>, name: &QString);

        /// Whether `name` can be a device name.
        #[qinvokable]
        #[cxx_name = "validName"]
        fn valid_name(self: &SystemInfo, name: &QString) -> bool;
    }

    impl cxx_qt::Threading for SystemInfo {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn system_info_make_unique() -> UniquePtr<SystemInfo>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use settings_sys::hostname::{self, Hostname, Status};
use settings_sys::sysinfo::{self, Info};
use settings_sys::{Bus, Error};

#[derive(Default)]
pub struct SystemInfoRust {
    loaded: bool,
    busy: bool,
    error: QString,
    device_name: QString,
    hostname: QString,
    chassis: QString,
    os: QString,
    os_build: QString,
    cpu: QString,
    memory: f64,
    graphics: QString,
}

fn read_name() -> Result<Status, Error> {
    Hostname::new(&Bus::System)?.status()
}

impl qobject::SystemInfo {
    fn show_name(mut self: Pin<&mut Self>, r: Option<Result<Status, Error>>) {
        match r {
            Some(Ok(s)) => {
                self.as_mut()
                    .set_device_name(QString::from(s.name.as_str()));
                self.as_mut()
                    .set_hostname(QString::from(s.hostname.as_str()));
                self.as_mut().set_chassis(QString::from(s.chassis.as_str()));
            }
            Some(Err(e)) => {
                let text = worker::describe("reading hostnamed", &e);
                self.as_mut().set_error(QString::from(text.as_str()));
            }
            None => self
                .as_mut()
                .set_error(QString::from("Settings couldn't read the device name.")),
        }
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(
            qt,
            "system-read",
            || (read_name(), sysinfo::read()),
            |mut o, r| {
                o.as_mut().set_loaded(true);
                let (name, info) = match r {
                    Some((n, i)) => (Some(n), i),
                    None => (None, Info::default()),
                };
                o.as_mut().show_name(name);
                o.as_mut().set_os(QString::from(info.os.as_str()));
                o.as_mut()
                    .set_os_build(QString::from(info.os_build.as_str()));
                o.as_mut().set_cpu(QString::from(info.cpu.as_str()));
                o.as_mut().set_memory(info.memory as f64);
                o.as_mut()
                    .set_graphics(QString::from(info.graphics.as_str()));
            },
        );
    }

    pub fn rename_device(mut self: Pin<&mut Self>, name: &QString) {
        if self.rust().busy {
            return;
        }
        let name = name.to_string();
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "system-rename",
            move || {
                let r = Hostname::new(&Bus::System).and_then(|h| h.set_name(&name));
                (r, read_name())
            },
            |mut o, r| {
                o.as_mut().set_busy(false);
                match r {
                    Some((result, status)) => {
                        o.as_mut().show_name(Some(status));
                        if let Err(e) = result {
                            let text = worker::describe("renaming the device", &e);
                            o.as_mut().set_error(QString::from(text.as_str()));
                        }
                    }
                    None => o.as_mut().show_name(None),
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
        }
    }

    pub fn valid_name(&self, name: &QString) -> bool {
        hostname::valid_name(name.to_string().trim())
    }
}
