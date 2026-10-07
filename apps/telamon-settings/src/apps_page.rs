//! Apps' backend: the Flatpak apps and what each may reach (their metadata
//! and the user's override files, `settings_sys::flatpak`) and what the
//! desktop portals let them do (the portal PermissionStore,
//! `settings_sys::portal`). Default Apps and Startup Apps are KConfig and
//! KService on the C++ side. Lives only while the page is shown; every call
//! runs on a worker thread.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        /// The first read has answered.
        #[qproperty(bool, loaded)]
        /// A change is under way.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        /// The Flatpak apps, as JSON: [{id, name, icon}].
        #[qproperty(QString, apps_json, cxx_name = "appsJson")]
        /// The portals' permission store answers.
        #[qproperty(bool, portal_available, cxx_name = "portalAvailable")]
        /// One app's permissions, as JSON: {id, files, devices, network,
        /// background, camera, microphone, screen}; "" when none is loaded.
        #[qproperty(QString, app_json, cxx_name = "appJson")]
        #[namespace = "telamon_settings"]
        type AppsPage = super::AppsPageRust;
    }

    extern "RustQt" {
        /// Reads the app list again.
        #[qinvokable]
        fn refresh(self: Pin<&mut AppsPage>);

        /// Reads one app's permissions into `appJson`.
        #[qinvokable]
        #[cxx_name = "loadApp"]
        fn load_app(self: Pin<&mut AppsPage>, id: &QString);

        #[qinvokable]
        #[cxx_name = "changeNetwork"]
        fn change_network(self: Pin<&mut AppsPage>, id: &QString, on: bool);

        #[qinvokable]
        #[cxx_name = "changeDevices"]
        fn change_devices(self: Pin<&mut AppsPage>, id: &QString, on: bool);

        /// `files` is "none", "downloads", "home" or "everything".
        #[qinvokable]
        #[cxx_name = "changeFiles"]
        fn change_files(self: Pin<&mut AppsPage>, id: &QString, files: &QString);

        /// `what` is "background", "camera", "microphone" or "screen";
        /// `answer` "ask", "yes" or "no".
        #[qinvokable]
        #[cxx_name = "changePortal"]
        fn change_portal(self: Pin<&mut AppsPage>, id: &QString, what: &QString, answer: &QString);
    }

    impl cxx_qt::Threading for AppsPage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn apps_page_make_unique() -> UniquePtr<AppsPage>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use settings_sys::flatpak::{Files, Flatpak, Roots};
use settings_sys::portal::{Answer, Permission, Portal};
use settings_sys::{Bus, Error, ErrorKind};
use std::path::PathBuf;

#[derive(Default)]
pub struct AppsPageRust {
    loaded: bool,
    busy: bool,
    error: QString,
    apps_json: QString,
    portal_available: bool,
    app_json: QString,
}

fn qs(s: &str) -> QString {
    QString::from(s)
}

/// `$XDG_DATA_HOME`, else `~/.local/share`.
fn data_home() -> PathBuf {
    let abs = |v: Option<std::ffi::OsString>| v.map(PathBuf::from).filter(|p| p.is_absolute());
    abs(std::env::var_os("XDG_DATA_HOME")).unwrap_or_else(|| {
        abs(std::env::var_os("HOME"))
            .map_or_else(|| PathBuf::from("/nonexistent"), |h| h.join(".local/share"))
    })
}

fn flatpak() -> Flatpak {
    Flatpak::new(Roots::standard(&data_home()))
}

/// The apps and whether the portals answer.
fn read() -> (String, bool) {
    let apps: Vec<_> = flatpak()
        .apps()
        .into_iter()
        .map(|a| json!({"id": a.id, "name": a.name, "icon": a.icon}))
        .collect();
    let portal = Portal::new(&Bus::Session).is_ok_and(|p| p.available());
    (
        serde_json::to_string(&apps).unwrap_or_else(|_| "[]".into()),
        portal,
    )
}

/// One app's permissions as JSON. A portal that doesn't answer leaves its
/// rows at "ask" (the page hides them).
fn read_app(id: &str) -> Result<String, Error> {
    let p = flatpak().permissions(id)?;
    let portal = Portal::new(&Bus::Session).ok();
    let answer = |what: Permission| {
        portal
            .as_ref()
            .and_then(|p| p.get(what, id).ok())
            .unwrap_or(Answer::Ask)
            .name()
    };
    Ok(json!({
        "id": id,
        "files": p.files.name(),
        "devices": p.devices,
        "network": p.network,
        "background": answer(Permission::Background),
        "camera": answer(Permission::Camera),
        "microphone": answer(Permission::Microphone),
        "screen": answer(Permission::Screen),
    })
    .to_string())
}

impl qobject::AppsPage {
    fn show(mut self: Pin<&mut Self>, read: Option<(String, bool)>) {
        self.as_mut().set_loaded(true);
        let Some((apps, portal)) = read else {
            self.as_mut()
                .set_error(qs("Settings couldn't read the installed apps."));
            return;
        };
        self.as_mut().set_apps_json(qs(&apps));
        self.as_mut().set_portal_available(portal);
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(qt, "apps-read", read, |o, r| o.show(r));
    }

    pub fn load_app(self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        let qt = self.qt_thread();
        worker::run(
            qt,
            "app-read",
            move || read_app(&id),
            |mut o, r| match r {
                Some(Ok(json)) => {
                    o.as_mut().set_error(QString::default());
                    o.as_mut().set_app_json(qs(&json));
                }
                Some(Err(e)) => {
                    let text = worker::describe("reading the app's permissions", &e);
                    o.as_mut().set_app_json(QString::default());
                    o.as_mut().set_error(qs(&text));
                }
                None => o.as_mut().set_app_json(QString::default()),
            },
        );
    }

    /// Runs change `job` for app `id`, then reads its permissions again.
    fn change<J>(mut self: Pin<&mut Self>, id: String, what: &'static str, job: J)
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
            "apps-change",
            move || {
                let r = job();
                (r, read_app(&id))
            },
            move |mut o, r| {
                o.as_mut().set_busy(false);
                match r {
                    Some((result, read)) => {
                        if let Ok(json) = read {
                            o.as_mut().set_app_json(qs(&json));
                        }
                        if let Err(e) = result {
                            let text = worker::describe(what, &e);
                            o.as_mut().set_error(qs(&text));
                        }
                    }
                    None => o.as_mut().set_error(qs("Settings couldn't change that.")),
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
        }
    }

    pub fn change_network(self: Pin<&mut Self>, id: &QString, on: bool) {
        let id = id.to_string();
        let app = id.clone();
        self.change(id, "changing network access", move || {
            flatpak().set_network(&app, on)
        });
    }

    pub fn change_devices(self: Pin<&mut Self>, id: &QString, on: bool) {
        let id = id.to_string();
        let app = id.clone();
        self.change(id, "changing device access", move || {
            flatpak().set_devices(&app, on)
        });
    }

    pub fn change_files(self: Pin<&mut Self>, id: &QString, files: &QString) {
        let id = id.to_string();
        let app = id.clone();
        let files = files.to_string();
        self.change(id, "changing file access", move || {
            let files = Files::from_name(&files)
                .ok_or_else(|| Error::new(ErrorKind::Refused, "not a file access level"))?;
            flatpak().set_files(&app, files)
        });
    }

    pub fn change_portal(self: Pin<&mut Self>, id: &QString, what: &QString, answer: &QString) {
        let id = id.to_string();
        let app = id.clone();
        let (what, answer) = (what.to_string(), answer.to_string());
        self.change(id, "changing the permission", move || {
            let what = Permission::from_name(&what)
                .ok_or_else(|| Error::new(ErrorKind::Refused, "not a permission"))?;
            let answer = Answer::from_name(&answer)
                .ok_or_else(|| Error::new(ErrorKind::Refused, "not an answer"))?;
            Portal::new(&Bus::Session)?.set(what, &app, answer)
        });
    }
}
