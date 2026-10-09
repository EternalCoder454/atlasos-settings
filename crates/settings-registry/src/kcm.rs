//! KCM names: which page each one opens, and which stay Plasma's.
//!
//! Plasma's applets, Dolphin and Telamon OS's own scripts open settings by KCM
//! name (`systemsettings kcm_x`, `kcmshell6 kcm_x`, KCMLauncher). Settings
//! answers those names: a KCM in [`MAP`] opens that page; any other valid
//! name opens the KCM itself in kcmshell6.

use crate::pages;

/// Where a KCM name leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Settings' first page (System Settings' own start, its landing page).
    Home,
    /// A page, and the setting on it when there is one.
    Page {
        page: &'static str,
        item: Option<&'static str>,
    },
    /// No page of Settings: the KCM in kcmshell6.
    Kcm,
}

/// The KCMs a Settings page replaces, and the setting each one shows.
/// Names that only exist in the Telamon OS image (plasma-firewall,
/// kinfocenter's energy page) are here too: their callers are.
pub static MAP: &[(&str, &str, Option<&str>)] = &[
    ("kcm_networkmanagement", "network", None),
    ("kcm_mobile_wifi", "network", Some("wifi")),
    ("kcm_mobile_wired", "network", Some("wired")),
    ("kcm_mobile_hotspot", "network", Some("hotspot")),
    ("kcm_bluetooth", "devices", Some("bluetooth")),
    ("kcm_firewall", "privacy", Some("firewall")),
    ("kcm_kscreen", "displays", None),
    ("kcm_nightlight", "displays", Some("night-light")),
    ("kcm_pulseaudio", "sound", None),
    ("kcm_keyboard", "input", Some("layouts")),
    ("kcm_mouse", "input", Some("speed")),
    ("kcm_touchpad", "input", Some("tap")),
    ("kcm_powerdevilprofilesconfig", "power", None),
    ("kcm_mobile_power", "power", None),
    ("kcm_energyinfo", "power", Some("battery")),
    ("kcm_lookandfeel", "appearance", Some("theme")),
    ("kcm_colors", "appearance", Some("accent")),
    ("kcm_wallpaper", "appearance", Some("wallpaper")),
    ("kcm_kwin_virtualdesktops", "appearance", Some("desktops")),
    ("kcm_notifications", "notifications", None),
    ("kcm_componentchooser", "apps", Some("defaults")),
    ("kcm_autostart", "apps", Some("autostart")),
    ("kcm_app-permissions", "apps", Some("permissions")),
    ("kcm_users", "users", None),
    ("kcm_feedback", "privacy", Some("crash-reports")),
    ("kcm_access", "accessibility", None),
    ("kcm_clock", "time-language", None),
    ("kcm_regionandlang", "time-language", Some("language")),
    ("kcm_updates", "updates", None),
    ("kcm_about-distro", "system", None),
];

/// Names that mean "System Settings itself": its landing page, and the
/// desktop ID the task manager passes.
pub static HOME_NAMES: &[&str] = &[
    "kcm_landingpage",
    "systemsettings",
    "org.kde.systemsettings",
];

/// Longest KCM name accepted, after `kcm`.
pub const MAX_NAME: usize = 64;

/// The plain KCM name in `raw`, or `None` when it is not one.
///
/// Callers pass `kcm_x`, `kcm_x.desktop` or a plugin path
/// (`plasma/kcms/systemsettings/kcm_x`); all give `kcm_x`. A name is `kcm`
/// then 1 to 64 of `A-Z a-z 0-9 _ -`, a letter or digit first after an
/// optional `_` (Plasma has `kcm_recentFiles` and
/// `kcmspellchecking`): no dots, slashes or spaces, so it can never name a
/// path or another program. A path is taken only in Plasma's plugin form
/// (`plugin_path_ok`).
pub fn normalize(raw: &str) -> Option<String> {
    if HOME_NAMES.contains(&raw) {
        return Some(raw.to_string());
    }
    let base = raw.rsplit('/').next().unwrap_or(raw);
    let base = base.strip_suffix(".desktop").unwrap_or(base);
    let rest = base.strip_prefix("kcm")?;
    // After `kcm` or `kcm_`, a letter or digit first.
    let body = rest.strip_prefix('_').unwrap_or(rest);
    let ok = (1..=MAX_NAME).contains(&rest.len())
        && body.starts_with(|c: char| c.is_ascii_alphanumeric())
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    (ok && plugin_path_ok(raw)).then(|| base.to_string())
}

/// Whether `raw` is no path, or Plasma's plugin path for a KCM:
/// `plasma/kcms/<name>` or `plasma/kcms/<folder>/<name>`, where `<folder>` is
/// lower case letters, digits and `_` (`systemsettings`,
/// `systemsettings_qwidgets`, `kinfocenter`). No `.`, `..` or empty parts, so
/// the name can't be reached by walking out of the folder; only the last part
/// is ever used.
fn plugin_path_ok(raw: &str) -> bool {
    if !raw.contains('/') {
        return true;
    }
    let parts: Vec<&str> = raw.split('/').collect();
    let folder_ok = |f: &str| {
        !f.is_empty()
            && f.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    match parts.as_slice() {
        ["plasma", "kcms", _name] => true,
        ["plasma", "kcms", folder, _name] => folder_ok(folder),
        _ => false,
    }
}

/// Where `name` (already [normalized](normalize)) leads.
pub fn resolve(name: &str) -> Target {
    if HOME_NAMES.contains(&name) {
        return Target::Home;
    }
    match MAP.iter().find(|(kcm, _, _)| *kcm == name) {
        Some(&(_, page, item)) => Target::Page { page, item },
        None => Target::Kcm,
    }
}

/// Whether `name` has a place in Settings: a page it maps to, or a setting
/// whose row opens it. "Other Plasma Settings" leaves these out.
pub fn has_page(name: &str) -> bool {
    !matches!(resolve(name), Target::Kcm)
        || pages::PAGES
            .iter()
            .flat_map(|p| p.items)
            .any(|i| i.kcm == Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_plasma_forms() {
        assert_eq!(normalize("kcm_kscreen").as_deref(), Some("kcm_kscreen"));
        assert_eq!(
            normalize("kcm_kscreen.desktop").as_deref(),
            Some("kcm_kscreen")
        );
        assert_eq!(
            normalize("plasma/kcms/systemsettings/kcm_kscreen").as_deref(),
            Some("kcm_kscreen")
        );
        assert_eq!(
            normalize("plasma/kcms/systemsettings_qwidgets/kcm_fonts.desktop").as_deref(),
            Some("kcm_fonts")
        );
        assert_eq!(normalize("plasma/kcms/kcm_x").as_deref(), Some("kcm_x"));
        assert_eq!(
            normalize("kcm_recentFiles").as_deref(),
            Some("kcm_recentFiles")
        );
        assert_eq!(
            normalize("kcmspellchecking").as_deref(),
            Some("kcmspellchecking")
        );
        assert_eq!(
            normalize("kcm_about-distro").as_deref(),
            Some("kcm_about-distro")
        );
        assert_eq!(
            normalize("org.kde.systemsettings").as_deref(),
            Some("org.kde.systemsettings")
        );
    }

    #[test]
    fn normalize_refuses_anything_else() {
        for bad in [
            "",
            "kcm",
            "kcm_",
            "kscreen",
            "kcm_a b",
            "kcm_a;rm",
            "kcm_../x",
            "../kcm_x",
            "/usr/lib/kcm_x",
            "kcm_x.so",
            "kcm_é",
            "-kcm_x",
            "kcm_x\n",
            "--args",
            // Paths other than Plasma's plugin folders, or walking out of them.
            "plasma/kcms/../../../bin/kcm_x",
            "plasma/kcms/systemsettings/../kcm_x",
            "plasma/kcms/./kcm_x",
            "plasma/kcms//kcm_x",
            "plasma/kcms/",
            "plasma/kcms/systemsettings/",
            "plasma/kcms/a/b/kcm_x",
            "plasma/kcms/Sys/kcm_x",
            "plasma/kcms/sys-tem/kcm_x",
            "plasma/kcm_x",
            "/plasma/kcms/kcm_x",
            "plasma/kcms/systemsettings/kcm_x/",
            "kde/plasma/kcms/kcm_x",
        ] {
            assert_eq!(normalize(bad), None, "{bad:?}");
        }
        let long = format!("kcm{}", "a".repeat(MAX_NAME + 1));
        assert_eq!(normalize(&long), None);
        let longest = format!("kcm{}", "a".repeat(MAX_NAME));
        assert_eq!(normalize(&longest), Some(longest.clone()));
    }

    #[test]
    fn every_mapped_page_and_item_exists() {
        for &(kcm, page, item) in MAP {
            let p = pages::page(page).unwrap_or_else(|| panic!("{kcm}: no page {page}"));
            if let Some(item) = item {
                assert!(p.item(item).is_some(), "{kcm}: no item {page}/{item}");
            }
            assert_eq!(normalize(kcm).as_deref(), Some(kcm));
        }
    }

    #[test]
    fn kcms_with_a_row_are_not_listed_again() {
        // Printers and Fonts are rows that open their KCM.
        assert!(has_page("kcm_printer_manager"));
        assert!(has_page("kcm_fonts"));
        assert!(has_page("kcm_kscreen"));
        assert!(!has_page("kcm_kwinrules"));
        // A direct request for a row's KCM still opens the KCM itself.
        assert_eq!(resolve("kcm_printer_manager"), Target::Kcm);
    }
}
