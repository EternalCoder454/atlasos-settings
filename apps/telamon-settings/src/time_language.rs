//! Time & Language's backend: network time and the time zone (timedated),
//! and the system's language (localed). The user's own language and
//! formats are Plasma's, written through KConfig (`cpp/localeconfig.cpp`).
//! Lives only while the page is shown; every call runs on a worker thread.

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
        /// timedated answered the last read.
        #[qproperty(bool, available)]
        /// A change is under way.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        /// An IANA name; "" when unknown.
        #[qproperty(QString, timezone)]
        #[qproperty(bool, ntp)]
        #[qproperty(bool, can_ntp, cxx_name = "canNtp")]
        #[qproperty(bool, synced)]
        /// localed's LANG; "" when unknown or unset.
        #[qproperty(QString, system_language, cxx_name = "systemLanguage")]
        /// The time zones timedated accepts, once `loadZones` answered.
        #[qproperty(QStringList, zones)]
        #[namespace = "telamon_settings"]
        type TimeLanguage = super::TimeLanguageRust;
    }

    extern "RustQt" {
        /// Reads everything again.
        #[qinvokable]
        fn refresh(self: Pin<&mut TimeLanguage>);

        #[qinvokable]
        #[cxx_name = "changeNtp"]
        fn change_ntp(self: Pin<&mut TimeLanguage>, on: bool);

        #[qinvokable]
        #[cxx_name = "changeTimezone"]
        fn change_timezone(self: Pin<&mut TimeLanguage>, zone: &QString);

        #[qinvokable]
        #[cxx_name = "changeSystemLanguage"]
        fn change_system_language(self: Pin<&mut TimeLanguage>, locale: &QString);

        /// Fills `zones` (once).
        #[qinvokable]
        #[cxx_name = "loadZones"]
        fn load_zones(self: Pin<&mut TimeLanguage>);

        /// The languages offered, as glibc locale names.
        #[qinvokable]
        fn languages(self: &TimeLanguage) -> QStringList;

        /// Whether `locale` is a locale name Settings writes.
        #[qinvokable]
        #[cxx_name = "validLocale"]
        fn valid_locale(self: &TimeLanguage, locale: &QString) -> bool;
    }

    impl cxx_qt::Threading for TimeLanguage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn time_language_make_unique() -> UniquePtr<TimeLanguage>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use settings_sys::locale::{self, Localed};
use settings_sys::timedate::{Status, Timedate};
use settings_sys::{Bus, Error};

#[derive(Default)]
pub struct TimeLanguageRust {
    loaded: bool,
    available: bool,
    busy: bool,
    error: QString,
    timezone: QString,
    ntp: bool,
    can_ntp: bool,
    synced: bool,
    system_language: QString,
    zones: QStringList,
    zones_asked: bool,
}

/// What a read gives: timedated's status, and localed's language ("" when
/// localed didn't answer: it is only shown under Advanced).
type Read = (Result<Status, Error>, String);

fn read() -> Read {
    let status = Timedate::new(&Bus::System).and_then(|t| t.status());
    let lang = Localed::new(&Bus::System)
        .and_then(|l| l.language())
        .unwrap_or_default();
    (status, lang)
}

impl qobject::TimeLanguage {
    fn show(mut self: Pin<&mut Self>, read: Option<Read>) {
        self.as_mut().set_loaded(true);
        let Some((status, lang)) = read else {
            self.as_mut()
                .set_error(QString::from("Settings couldn't read the time settings."));
            return;
        };
        match status {
            Ok(s) => {
                self.as_mut().set_available(true);
                self.as_mut()
                    .set_timezone(QString::from(s.timezone.as_str()));
                self.as_mut().set_ntp(s.ntp);
                self.as_mut().set_can_ntp(s.can_ntp);
                self.as_mut().set_synced(s.ntp_synchronized);
            }
            Err(e) => {
                self.as_mut().set_available(false);
                let text = worker::describe("reading timedated", &e);
                self.as_mut().set_error(QString::from(text.as_str()));
            }
        }
        self.as_mut()
            .set_system_language(QString::from(lang.as_str()));
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(qt, "time-read", read, |o, r| o.show(r));
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
            "time-change",
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

    pub fn change_ntp(self: Pin<&mut Self>, on: bool) {
        self.change("setting network time", move || {
            Timedate::new(&Bus::System)?.set_ntp(on)
        });
    }

    pub fn change_timezone(self: Pin<&mut Self>, zone: &QString) {
        let zone = zone.to_string();
        self.change("setting the time zone", move || {
            Timedate::new(&Bus::System)?.set_timezone(&zone)
        });
    }

    pub fn change_system_language(self: Pin<&mut Self>, locale: &QString) {
        let locale = locale.to_string();
        self.change("setting the system language", move || {
            Localed::new(&Bus::System)?.set_language(&locale)
        });
    }

    pub fn load_zones(mut self: Pin<&mut Self>) {
        if self.rust().zones_asked {
            return;
        }
        self.as_mut().rust_mut().zones_asked = true;
        let qt = self.qt_thread();
        worker::run(
            qt,
            "time-zones",
            || Timedate::new(&Bus::System).and_then(|t| t.timezones()),
            |mut o, r| match r {
                Some(Ok(zones)) => {
                    let mut list = QStringList::default();
                    for z in &zones {
                        list.append(QString::from(z.as_str()));
                    }
                    o.as_mut().set_zones(list);
                }
                Some(Err(e)) => {
                    o.as_mut().rust_mut().zones_asked = false;
                    let text = worker::describe("listing time zones", &e);
                    o.as_mut().set_error(QString::from(text.as_str()));
                }
                None => o.as_mut().rust_mut().zones_asked = false,
            },
        );
    }

    pub fn languages(&self) -> QStringList {
        let mut list = QStringList::default();
        for l in locale::LANGUAGES {
            list.append(QString::from(*l));
        }
        list
    }

    pub fn valid_locale(&self, locale: &QString) -> bool {
        locale::valid_locale(&locale.to_string())
    }
}
