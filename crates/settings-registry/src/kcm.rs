//! KCM names: which page each one opens, and which stay Plasma's.
//!
//! Plasma's applets, Dolphin and AtlasOS's own scripts open settings by KCM
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
/// Names that only exist in the AtlasOS image (plasma-firewall,
/// kinfocenter's energy page) are here too: their callers are.
pub static MAP: &[(&str, &str, Option<&str>)] = &[
    ("kcm_networkmanagement", "network", None),
    ("kcm_mobile_wifi", "network", Some("wifi")),
    ("kcm_mobile_wired", "network", Some("wired")),
    ("kcm_mobile_hotspot", "network", Some("hotspot")),
    ("kcm_bluetooth", "bluetooth", None),
    ("kcm_firewall", "firewall", None),
    ("kcm_kscreen", "displays", None),
    ("kcm_nightlight", "displays", Some("night-light")),
    ("kcm_pulseaudio", "sound", None),
    ("kcm_keyboard", "keyboard", None),
    ("kcm_mouse", "mouse", None),
    ("kcm_touchpad", "mouse", None),
    ("kcm_powerdevilprofilesconfig", "power", None),
    ("kcm_mobile_power", "power", None),
    ("kcm_energyinfo", "power", Some("battery")),
    ("kcm_lookandfeel", "appearance", Some("theme")),
    ("kcm_colors", "appearance", Some("accent")),
    ("kcm_wallpaper", "appearance", Some("wallpaper")),
    ("kcm_kwin_virtualdesktops", "desktop", Some("desktops")),
    ("kcm_notifications", "notifications", None),
    ("kcm_componentchooser", "default-apps", Some("defaults")),
    ("kcm_autostart", "default-apps", Some("autostart")),
    ("kcm_app-permissions", "app-permissions", None),
    ("kcm_users", "users", None),
    ("kcm_feedback", "privacy", Some("crash-reports")),
    ("kcm_access", "accessibility", None),
    ("kcm_clock", "datetime", None),
    ("kcm_regionandlang", "region", None),
    ("kcm_updates", "updates", None),
    ("kcm_about-distro", "about", None),
    // Printers stays Plasma's KCM, opened from its page.
    ("kcm_printer_manager", "printers", None),
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
/// path or another program.
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
    // A path is only taken for Plasma's plugin folders.
    let path_ok = !raw.contains('/') || raw.starts_with("plasma/kcms/");
    (ok && path_ok).then(|| base.to_string())
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

/// Whether `name` has a page of Settings: "More Settings" leaves these out.
pub fn has_page(name: &str) -> bool {
    match resolve(name) {
        Target::Home => true,
        Target::Kcm => false,
        // Printers is a page, but its KCM is all there is of it.
        Target::Page { page, .. } => {
            pages::page(page).is_some_and(|p| !matches!(p.kind, pages::Kind::Kcm(_)))
        }
    }
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
    fn printers_is_its_kcm() {
        assert_eq!(
            pages::page("printers").map(|p| p.kind),
            Some(pages::Kind::Kcm("kcm_printer_manager"))
        );
        assert!(!has_page("kcm_printer_manager"));
        assert!(has_page("kcm_kscreen"));
        assert!(!has_page("kcm_fonts"));
    }
}
