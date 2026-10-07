//! Flatpak apps and their overrides, against fixture folders (never the
//! real Flatpak installations or the user's overrides).

use settings_sys::ErrorKind;
use settings_sys::flatpak::{Files, Flatpak, Roots};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "settings-flatpak-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Fixture { dir }
    }

    fn system(&self) -> PathBuf {
        self.dir.join("system")
    }
    fn user(&self) -> PathBuf {
        self.dir.join("user")
    }
    fn overrides(&self) -> PathBuf {
        self.user().join("overrides")
    }

    /// An installed app: its metadata and desktop file.
    fn app(&self, root: &Path, id: &str, name: &str, context: &str) {
        let dep = root.join("app").join(id).join("current/active");
        fs::create_dir_all(dep.join("export/share/applications")).unwrap();
        fs::write(
            dep.join("metadata"),
            format!("[Application]\nname={id}\nruntime=org.freedesktop.Platform/x86_64/24.08\n\n[Context]\n{context}"),
        )
        .unwrap();
        fs::write(
            dep.join(format!("export/share/applications/{id}.desktop")),
            format!("[Desktop Entry]\nName={name}\nIcon={id}\nType=Application\n"),
        )
        .unwrap();
    }

    fn flatpak(&self) -> Flatpak {
        Flatpak::new(Roots {
            installations: vec![self.system(), self.user()],
            overrides: self.overrides(),
        })
    }

    fn override_text(&self, id: &str) -> Option<String> {
        fs::read_to_string(self.overrides().join(id)).ok()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

const CHAT: &str = "org.example.Chat";

#[test]
fn lists_apps_not_runtimes_by_name() {
    let f = Fixture::new();
    f.app(&f.system(), CHAT, "Zed Chat", "shared=network;\n");
    f.app(&f.user(), "org.example.Alpha", "alpha", "");
    // A runtime has no [Application] group and no entry under app/.
    let rt = f
        .system()
        .join("runtime/org.freedesktop.Platform/x86_64/24.08/active");
    fs::create_dir_all(&rt).unwrap();
    fs::write(
        rt.join("metadata"),
        "[Runtime]\nname=org.freedesktop.Platform\n",
    )
    .unwrap();
    // A folder that isn't an app ID, and one with a runtime-style metadata.
    let odd = f.system().join("app/not-an-id/current/active");
    fs::create_dir_all(&odd).unwrap();
    fs::write(odd.join("metadata"), "[Application]\nname=x\n").unwrap();
    let broken = f.system().join("app/org.example.Broken/current/active");
    fs::create_dir_all(&broken).unwrap();
    fs::write(
        broken.join("metadata"),
        "[Runtime]\nname=org.example.Broken\n",
    )
    .unwrap();

    let apps = f.flatpak().apps();
    let names: Vec<_> = apps
        .iter()
        .map(|a| (a.id.as_str(), a.name.as_str(), a.icon.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("org.example.Alpha", "alpha", "org.example.Alpha"),
            (CHAT, "Zed Chat", CHAT)
        ]
    );
}

#[test]
fn an_icon_that_is_a_path_is_not_shown() {
    let f = Fixture::new();
    f.app(&f.system(), CHAT, "Chat", "");
    let desktop = f.system().join(format!(
        "app/{CHAT}/current/active/export/share/applications/{CHAT}.desktop"
    ));
    fs::write(desktop, "[Desktop Entry]\nName=Chat\nIcon=/etc/passwd\n").unwrap();
    assert_eq!(f.flatpak().apps()[0].icon, "application-x-executable");
}

#[test]
fn reads_what_the_app_may_reach() {
    let f = Fixture::new();
    f.app(
        &f.system(),
        CHAT,
        "Chat",
        "shared=network;ipc;\ndevices=dri;\nfilesystems=xdg-download;xdg-documents:ro;\n",
    );
    let p = f.flatpak().permissions(CHAT).unwrap();
    assert!(p.network && !p.devices);
    assert_eq!(p.files, Files::Downloads);

    // Overrides change it, and the global one applies to every app.
    fs::create_dir_all(f.overrides()).unwrap();
    fs::write(f.overrides().join("global"), "[Context]\ndevices=all;\n").unwrap();
    fs::write(
        f.overrides().join(CHAT),
        "[Context]\nshared=!network;\nfilesystems=home;\n",
    )
    .unwrap();
    let p = f.flatpak().permissions(CHAT).unwrap();
    assert!(!p.network && p.devices);
    assert_eq!(p.files, Files::Home);
}

#[test]
fn writes_only_what_differs_from_the_apps_own_permissions() {
    let f = Fixture::new();
    f.app(
        &f.system(),
        CHAT,
        "Chat",
        "shared=network;ipc;\nfilesystems=xdg-download;\n",
    );
    let fp = f.flatpak();

    // Network off: the override says "not network"; nothing else.
    fp.set_network(CHAT, false).unwrap();
    assert_eq!(
        f.override_text(CHAT).as_deref(),
        Some("[Context]\nshared=!network;\n")
    );
    assert!(!fp.permissions(CHAT).unwrap().network);
    // Back on: the override is gone altogether.
    fp.set_network(CHAT, true).unwrap();
    assert_eq!(f.override_text(CHAT), None);

    // Devices on: added.
    fp.set_devices(CHAT, true).unwrap();
    assert_eq!(
        f.override_text(CHAT).as_deref(),
        Some("[Context]\ndevices=all;\n")
    );
    fp.set_devices(CHAT, false).unwrap();
    assert_eq!(f.override_text(CHAT), None);

    // Files: the app has Downloads. Home adds home and takes Downloads away.
    fp.set_files(CHAT, Files::Home).unwrap();
    assert_eq!(
        f.override_text(CHAT).as_deref(),
        Some("[Context]\nfilesystems=home;!xdg-download;\n")
    );
    assert_eq!(fp.permissions(CHAT).unwrap().files, Files::Home);
    fp.set_files(CHAT, Files::None).unwrap();
    assert_eq!(
        f.override_text(CHAT).as_deref(),
        Some("[Context]\nfilesystems=!xdg-download;\n")
    );
    assert_eq!(fp.permissions(CHAT).unwrap().files, Files::None);
    fp.set_files(CHAT, Files::Everything).unwrap();
    assert_eq!(fp.permissions(CHAT).unwrap().files, Files::Everything);
    fp.set_files(CHAT, Files::Downloads).unwrap();
    assert_eq!(f.override_text(CHAT), None);
}

#[test]
fn keeps_the_rest_of_an_override_file() {
    let f = Fixture::new();
    f.app(&f.system(), CHAT, "Chat", "shared=network;\n");
    fs::create_dir_all(f.overrides()).unwrap();
    fs::write(
        f.overrides().join(CHAT),
        "[Context]\nsockets=x11;\nshared=!network;\n\n[Session Bus Policy]\norg.example.Foo=talk\n\n[Environment]\nFOO=bar\n",
    )
    .unwrap();
    f.flatpak().set_network(CHAT, true).unwrap();
    assert_eq!(
        f.override_text(CHAT).as_deref(),
        Some(
            "[Context]\nsockets=x11;\n\n[Session Bus Policy]\norg.example.Foo=talk\n\n[Environment]\nFOO=bar\n"
        )
    );
}

#[test]
fn global_overrides_are_never_written() {
    let f = Fixture::new();
    f.app(&f.system(), CHAT, "Chat", "");
    fs::create_dir_all(f.overrides()).unwrap();
    fs::write(f.overrides().join("global"), "[Context]\nshared=network;\n").unwrap();
    // Turning network off for the app records the difference only for it.
    f.flatpak().set_network(CHAT, false).unwrap();
    assert_eq!(
        f.override_text(CHAT).as_deref(),
        Some("[Context]\nshared=!network;\n")
    );
    assert_eq!(
        f.override_text("global").as_deref(),
        Some("[Context]\nshared=network;\n")
    );
}

#[test]
fn bad_ids_and_unknown_apps_are_refused_and_write_nothing() {
    let f = Fixture::new();
    f.app(&f.system(), CHAT, "Chat", "");
    let fp = f.flatpak();
    for bad in ["", "../global", "global", "a/b.c", "org.example.Nope"] {
        assert_eq!(
            fp.set_network(bad, false).unwrap_err().kind,
            ErrorKind::Refused,
            "{bad}"
        );
        assert!(fp.permissions(bad).is_err(), "{bad}");
    }
    assert!(!f.overrides().exists());
}

#[test]
fn files_names_round_trip() {
    for x in [
        Files::None,
        Files::Downloads,
        Files::Home,
        Files::Everything,
    ] {
        assert_eq!(Files::from_name(x.name()), Some(x));
    }
    assert_eq!(Files::from_name("all"), None);
}
