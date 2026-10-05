//! The registry against the KCM names Plasma 6.7 really has
//! (`fixtures/kcms-plasma-6.7.txt`) and the ones its applets and AtlasOS's
//! scripts open.

use settings_registry::kcm::{self, Target};
use settings_registry::{Kind, PAGES};

fn installed() -> Vec<&'static str> {
    include_str!("fixtures/kcms-plasma-6.7.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect()
}

/// In the AtlasOS image but not in the fixture's Plasma: plasma-firewall,
/// and kinfocenter's energy page (kinfocenter is removed from the image,
/// but the battery applet still asks for it).
const IMAGE_ONLY: &[&str] = &["kcm_firewall", "kcm_energyinfo"];

#[test]
fn every_installed_name_is_accepted() {
    let names = installed();
    assert!(names.len() > 60, "fixture looks truncated: {}", names.len());
    for name in names {
        assert_eq!(kcm::normalize(name).as_deref(), Some(name), "{name}");
        assert_eq!(
            kcm::normalize(&format!("{name}.desktop")).as_deref(),
            Some(name)
        );
    }
}

#[test]
fn every_mapped_or_linked_kcm_exists() {
    let names = installed();
    let exists = |k: &str| names.contains(&k) || IMAGE_ONLY.contains(&k);
    for &(k, _, _) in kcm::MAP {
        assert!(exists(k), "{k} is mapped but not installed");
    }
    for p in PAGES {
        if let Kind::Kcm(k) = p.kind {
            assert!(exists(k), "page {} opens missing {k}", p.id);
        }
        for i in p.items {
            if let Some(k) = i.kcm {
                assert!(exists(k), "{}/{} opens missing {k}", p.id, i.id);
            }
        }
    }
}

/// What Plasma's applets (KCMLauncher), Dolphin and AtlasOS's scripts ask
/// for, and where each must land.
#[test]
fn callers_land_on_their_page() {
    let page = |p| Target::Page {
        page: p,
        item: None,
    };
    let cases: &[(&str, Target)] = &[
        ("kcm_networkmanagement", page("network")),
        ("kcm_bluetooth", page("bluetooth")),
        ("kcm_pulseaudio", page("sound")),
        ("kcm_powerdevilprofilesconfig", page("power")),
        (
            "kcm_energyinfo",
            Target::Page {
                page: "power",
                item: Some("battery"),
            },
        ),
        (
            "kcm_nightlight",
            Target::Page {
                page: "displays",
                item: Some("night-light"),
            },
        ),
        ("kcm_landingpage", Target::Home),
        ("kcm_kscreen", page("displays")),
        ("kcm_clock", page("datetime")),
        ("kcm_regionandlang", page("region")),
        ("kcm_keyboard", page("keyboard")),
        ("kcm_notifications", page("notifications")),
        ("kcm_printer_manager", page("printers")),
        (
            "kcm_kwin_virtualdesktops",
            Target::Page {
                page: "desktop",
                item: Some("desktops"),
            },
        ),
        ("kcm_users", page("users")),
        ("org.kde.systemsettings", Target::Home),
        // Kept on Plasma's KCM.
        ("kcm_device_automounter", Target::Kcm),
        ("kcm_plasmasearch", Target::Kcm),
        ("kcm_trash", Target::Kcm),
        ("kcm_keys", Target::Kcm),
    ];
    for &(name, want) in cases {
        let n = kcm::normalize(name).unwrap_or_else(|| panic!("{name} refused"));
        assert_eq!(kcm::resolve(&n), want, "{name}");
    }
}

#[test]
fn more_settings_lists_the_rest() {
    let rest: Vec<_> = installed()
        .into_iter()
        .filter(|k| !kcm::has_page(k))
        .collect();
    assert!(rest.contains(&"kcm_fonts"));
    assert!(rest.contains(&"kcm_printer_manager"));
    assert!(!rest.contains(&"kcm_kscreen"));
    assert!(!rest.contains(&"kcm_landingpage"));
}
