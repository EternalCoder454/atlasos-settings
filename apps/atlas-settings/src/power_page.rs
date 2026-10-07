//! Power & Battery's backend: the power mode (power-profiles-daemon), the
//! battery and its charge limit (UPower), and PowerDevil's "read your
//! settings again". The screen and sleep timeouts are PowerDevil's
//! `powerdevilrc`, written through KConfig (`cpp/powerconfig.cpp`). Lives
//! only while the page is shown; every call runs on a worker thread.

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
        /// power-profiles-daemon is there.
        #[qproperty(bool, has_profiles, cxx_name = "hasProfiles")]
        /// "power-saver", "balanced" or "performance".
        #[qproperty(QString, profile)]
        /// The modes this computer has, in picker order.
        #[qproperty(QStringList, profiles)]
        /// Why Performance is held back, in plain words; "" for not.
        #[qproperty(QString, degraded)]
        #[qproperty(bool, has_battery, cxx_name = "hasBattery")]
        #[qproperty(bool, has_lid, cxx_name = "hasLid")]
        /// 0 to 100.
        #[qproperty(i32, percentage)]
        /// "charging", "discharging", "full", "empty", "holding" or "unknown".
        #[qproperty(QString, charge_state, cxx_name = "chargeState")]
        /// "2 hours 5 minutes"; "" when unknown.
        #[qproperty(QString, time_left, cxx_name = "timeLeft")]
        /// Percent of the original capacity; -1 when unknown.
        #[qproperty(i32, health)]
        #[qproperty(bool, limit_supported, cxx_name = "limitSupported")]
        #[qproperty(bool, limit_enabled, cxx_name = "limitEnabled")]
        #[namespace = "atlas_settings"]
        type PowerPage = super::PowerPageRust;
    }

    extern "RustQt" {
        /// Reads everything again.
        #[qinvokable]
        fn refresh(self: Pin<&mut PowerPage>);

        #[qinvokable]
        #[cxx_name = "changeProfile"]
        fn change_profile(self: Pin<&mut PowerPage>, profile: &QString);

        #[qinvokable]
        #[cxx_name = "changeChargeLimit"]
        fn change_charge_limit(self: Pin<&mut PowerPage>, on: bool);

        /// PowerDevil reads `powerdevilrc` again (after the page wrote it).
        #[qinvokable]
        fn reconfigure(self: Pin<&mut PowerPage>);
    }

    impl cxx_qt::Threading for PowerPage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn power_page_make_unique() -> UniquePtr<PowerPage>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use settings_sys::power::{self, Battery, ChargeState, Power, ProfileState};
use settings_sys::{Bus, Error, ErrorKind};

#[derive(Default)]
pub struct PowerPageRust {
    loaded: bool,
    busy: bool,
    error: QString,
    has_profiles: bool,
    profile: QString,
    profiles: QStringList,
    degraded: QString,
    has_battery: bool,
    has_lid: bool,
    percentage: i32,
    charge_state: QString,
    time_left: QString,
    health: i32,
    limit_supported: bool,
    limit_enabled: bool,
}

/// What a read gives: the power mode, the battery and whether there is a
/// lid, each as UPower or the daemon answered.
struct Read {
    profile: Result<Option<ProfileState>, Error>,
    battery: Result<Option<Battery>, Error>,
    lid: bool,
}

fn read() -> Read {
    match Power::new(&Bus::System) {
        Ok(p) => Read {
            profile: p.profile(),
            battery: p.battery(),
            lid: p.lid_present().unwrap_or(false),
        },
        Err(e) => Read {
            profile: Err(e.clone()),
            battery: Err(e),
            lid: false,
        },
    }
}

/// A service that isn't there is no row, not an error; anything else is.
fn worth_saying(e: &Error) -> bool {
    e.kind != ErrorKind::NotRunning
}

fn state_word(s: ChargeState) -> &'static str {
    match s {
        ChargeState::Charging => "charging",
        ChargeState::Discharging => "discharging",
        ChargeState::Full => "full",
        ChargeState::Empty => "empty",
        ChargeState::Holding => "holding",
        ChargeState::Unknown => "unknown",
    }
}

/// Why Performance runs slower, in plain words.
fn degraded_words(reason: &str) -> &'static str {
    match reason {
        "" => "",
        "lap-detected" => "Performance is lowered while the computer is on your lap.",
        "high-operating-temperature" => "Performance is lowered while the computer is hot.",
        _ => "Performance is lowered for now.",
    }
}

impl qobject::PowerPage {
    fn show(mut self: Pin<&mut Self>, read: Option<Read>) {
        self.as_mut().set_loaded(true);
        let Some(read) = read else {
            self.as_mut()
                .set_error(QString::from("Settings couldn't read the power settings."));
            return;
        };
        let mut problem = None;
        match read.profile {
            Ok(Some(s)) => {
                self.as_mut().set_has_profiles(true);
                self.as_mut().set_profile(QString::from(s.active.as_str()));
                let mut list = QStringList::default();
                for p in &s.available {
                    list.append(QString::from(p.as_str()));
                }
                self.as_mut().set_profiles(list);
                self.as_mut()
                    .set_degraded(QString::from(degraded_words(&s.degraded)));
            }
            Ok(None) => self.as_mut().set_has_profiles(false),
            Err(e) => {
                self.as_mut().set_has_profiles(false);
                if worth_saying(&e) {
                    problem = Some(worker::describe("reading the power mode", &e));
                }
            }
        }
        match read.battery {
            Ok(Some(b)) => {
                self.as_mut().set_has_battery(true);
                self.as_mut().set_percentage(b.percentage.round() as i32);
                self.as_mut()
                    .set_charge_state(QString::from(state_word(b.state)));
                let secs = match b.state {
                    ChargeState::Charging => b.time_to_full,
                    ChargeState::Discharging => b.time_to_empty,
                    _ => 0,
                };
                let left = power::time_left(secs).unwrap_or_default();
                self.as_mut().set_time_left(QString::from(left.as_str()));
                self.as_mut()
                    .set_health(b.health.map_or(-1, |h| h.round() as i32));
                self.as_mut().set_limit_supported(b.limit_supported);
                self.as_mut().set_limit_enabled(b.limit_enabled);
            }
            Ok(None) => self.as_mut().set_has_battery(false),
            Err(e) => {
                self.as_mut().set_has_battery(false);
                if worth_saying(&e) && problem.is_none() {
                    problem = Some(worker::describe("reading the battery", &e));
                }
            }
        }
        self.as_mut().set_has_lid(read.lid);
        self.as_mut()
            .set_error(QString::from(problem.unwrap_or_default().as_str()));
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(qt, "power-read", read, |o, r| o.show(r));
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
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "power-change",
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
                            o.as_mut().set_error(QString::from(text.as_str()));
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

    pub fn change_profile(self: Pin<&mut Self>, profile: &QString) {
        let profile = profile.to_string();
        self.change("setting the power mode", move || {
            Power::new(&Bus::System)?.set_profile(&profile)
        });
    }

    pub fn change_charge_limit(self: Pin<&mut Self>, on: bool) {
        self.change("setting the charge limit", move || {
            Power::new(&Bus::System)?.set_charge_limit(on)
        });
    }

    pub fn reconfigure(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(
            qt,
            "powerdevil",
            || power::reconfigure_powerdevil(&Bus::Session),
            |_o, r| {
                // PowerDevil not running: it reads the file when it starts.
                if let Some(Err(e)) = r
                    && e.kind != ErrorKind::NotRunning
                {
                    log::warn!("telling PowerDevil about new settings: {}", e.detail);
                }
            },
        );
    }
}
