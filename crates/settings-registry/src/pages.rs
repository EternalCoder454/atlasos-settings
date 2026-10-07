//! The pages, in sidebar order, and the settings on each that search finds.
//!
//! Few pages, named for what people want to do (docs/DESIGN.md, "Pages"):
//! each shows the settings most people change, and the rest of what it
//! covers folds under Advanced on the same page ([`Item::advanced`]). Search
//! finds every setting, folded or not.
//!
//! Titles are English. No Atlas app ships translations yet; when they come,
//! these strings are extracted for Qt Linguist from here (docs/DESIGN.md).

/// How a page is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Drawn by Settings.
    Native,
    /// The list of the KCMs Settings has no page or setting for.
    MoreSettings,
}

/// One setting on a page: what search finds and what a deep link
/// (`atlas-settings <page> <item>`) scrolls to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: &'static str,
    pub title: &'static str,
    /// Other words people search with (lower case).
    pub keywords: &'static [&'static str],
    /// The KCM that holds this setting, when Settings has none of its own.
    pub kcm: Option<&'static str>,
    /// Folded under the page's Advanced section until opened (or until a
    /// search or link goes to it).
    pub advanced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    /// An Atlas.Ui `Symbols` name (`Symbols.codepoint(name)` in QML).
    pub symbol: &'static str,
    pub kind: Kind,
    /// Other words people search with (lower case).
    pub keywords: &'static [&'static str],
    pub items: &'static [Item],
    /// Pages people often want next, shown as links at the page's end.
    pub related: &'static [&'static str],
}

impl Page {
    pub fn item(&self, id: &str) -> Option<&'static Item> {
        self.items.iter().find(|i| i.id == id)
    }
}

/// The page with this ID.
pub fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.id == id)
}

/// Page IDs of earlier versions, which links may still use: the page each
/// became, and the setting on it that was the old page (`None` for its top).
pub static RENAMED: &[(&str, &str, Option<&str>)] = &[
    ("bluetooth", "devices", Some("bluetooth")),
    ("printers", "devices", Some("printers")),
    ("keyboard", "input", Some("layouts")),
    ("mouse", "input", Some("speed")),
    ("desktop", "appearance", Some("dock")),
    ("default-apps", "apps", Some("defaults")),
    ("app-permissions", "apps", Some("permissions")),
    ("firewall", "privacy", Some("firewall")),
    ("datetime", "time-language", None),
    ("region", "time-language", Some("language")),
    ("updates", "system", Some("updates")),
    ("about", "system", None),
];

/// Page `id` (or the page an earlier ID became), with setting `item` on it.
/// An item the page doesn't have falls back to where a renamed page landed.
pub fn find(id: &str, item: Option<&str>) -> Option<(&'static Page, Option<&'static Item>)> {
    if let Some(p) = page(id) {
        return Some((p, item.and_then(|i| p.item(i))));
    }
    let &(_, to, landing) = RENAMED.iter().find(|(from, _, _)| *from == id)?;
    let p = page(to)?;
    let item = item
        .and_then(|i| p.item(i))
        .or_else(|| landing.and_then(|i| p.item(i)));
    Some((p, item))
}

const fn item(id: &'static str, title: &'static str, keywords: &'static [&'static str]) -> Item {
    Item {
        id,
        title,
        keywords,
        kcm: None,
        advanced: false,
    }
}

/// A setting folded under Advanced.
const fn adv(id: &'static str, title: &'static str, keywords: &'static [&'static str]) -> Item {
    Item {
        advanced: true,
        ..item(id, title, keywords)
    }
}

/// A setting Plasma's KCM `kcm` holds (its row opens that KCM).
const fn kcm_item(
    id: &'static str,
    title: &'static str,
    keywords: &'static [&'static str],
    kcm: &'static str,
) -> Item {
    Item {
        kcm: Some(kcm),
        ..item(id, title, keywords)
    }
}

/// A KCM's setting folded under Advanced.
const fn adv_kcm(
    id: &'static str,
    title: &'static str,
    keywords: &'static [&'static str],
    kcm: &'static str,
) -> Item {
    Item {
        advanced: true,
        ..kcm_item(id, title, keywords, kcm)
    }
}

pub static PAGES: &[Page] = &[
    Page {
        id: "home",
        title: "Home",
        symbol: "Home",
        kind: Kind::Native,
        keywords: &["start", "overview", "quick settings"],
        items: &[],
        // The pages people use most, as links on Home.
        related: &[
            "network",
            "devices",
            "displays",
            "sound",
            "appearance",
            "system",
        ],
    },
    Page {
        id: "network",
        title: "Network",
        symbol: "Wifi",
        kind: Kind::Native,
        keywords: &[
            "wifi",
            "wlan",
            "ethernet",
            "internet",
            "networkmanager",
            "connection",
        ],
        items: &[
            item(
                "wifi",
                "Wi-Fi",
                &["wireless", "wlan", "hotspot", "password"],
            ),
            item("wired", "Wired", &["ethernet", "cable", "lan"]),
            item("vpn", "VPN", &["wireguard", "openvpn", "tunnel"]),
            item("hotspot", "Hotspot", &["tethering", "share connection"]),
            item("airplane", "Airplane Mode", &["flight mode", "radio"]),
            adv_kcm("proxy", "Proxy", &["http proxy", "socks"], "kcm_proxy"),
            adv_kcm(
                "cellular",
                "Mobile Broadband",
                &["cellular", "modem", "sim", "lte"],
                "kcm_cellular_network",
            ),
        ],
        related: &[],
    },
    Page {
        id: "devices",
        title: "Bluetooth & Devices",
        symbol: "Bluetooth",
        kind: Kind::Native,
        keywords: &["bluez", "pair", "headphones", "wireless", "peripherals"],
        items: &[
            item(
                "bluetooth",
                "Bluetooth",
                &["enable", "disable", "on", "off"],
            ),
            item(
                "devices",
                "Devices",
                &["pair", "connect", "headset", "keyboard", "mouse"],
            ),
            kcm_item(
                "printers",
                "Printers",
                &["printer", "cups", "scanner", "print queue"],
                "kcm_printer_manager",
            ),
            adv(
                "visibility",
                "Visible to Other Devices",
                &["discoverable", "name"],
            ),
            adv_kcm(
                "tablet",
                "Drawing Tablet",
                &["wacom", "pen", "stylus"],
                "kcm_tablet",
            ),
            adv_kcm("touchscreen", "Touchscreen", &["touch"], "kcm_touchscreen"),
            adv_kcm(
                "controller",
                "Game Controller",
                &["gamepad", "joystick"],
                "kcm_gamecontroller",
            ),
        ],
        related: &[],
    },
    Page {
        id: "displays",
        title: "Displays",
        symbol: "Monitor",
        kind: Kind::Native,
        keywords: &["monitor", "screen", "kscreen"],
        items: &[
            item("resolution", "Resolution", &["size", "pixels"]),
            item("scale", "Scale", &["zoom", "dpi", "hidpi", "text size"]),
            item(
                "arrangement",
                "Arrangement",
                &["position", "multiple monitors", "primary"],
            ),
            item(
                "night-light",
                "Night Light",
                &[
                    "blue light",
                    "warm",
                    "colour temperature",
                    "color temperature",
                ],
            ),
            adv(
                "refresh-rate",
                "Refresh Rate",
                &["hz", "fps", "vrr", "adaptive sync"],
            ),
            adv("orientation", "Orientation", &["rotate", "portrait"]),
            adv("hdr", "HDR", &["high dynamic range", "brightness"]),
        ],
        related: &[],
    },
    Page {
        id: "sound",
        title: "Sound",
        symbol: "VolumeUp",
        kind: Kind::Native,
        keywords: &["audio", "volume", "pipewire", "pulseaudio", "speakers"],
        items: &[
            item(
                "output",
                "Output Device",
                &["speakers", "headphones", "hdmi"],
            ),
            item("volume", "Volume", &["loud", "mute"]),
            item("input", "Input Device", &["microphone", "mic", "recording"]),
            item("apps", "App Volume", &["mixer", "per app"]),
            adv_kcm(
                "sounds",
                "System Sounds",
                &["alert", "event sounds", "theme"],
                "kcm_soundtheme",
            ),
        ],
        related: &[],
    },
    Page {
        id: "input",
        title: "Keyboard & Mouse",
        symbol: "Keyboard",
        kind: Kind::Native,
        keywords: &["typing", "keys", "pointer", "trackpad", "cursor", "click"],
        items: &[
            item("layouts", "Input Sources", &["layout", "language", "xkb"]),
            item("speed", "Pointer Speed", &["acceleration", "sensitivity"]),
            item(
                "scrolling",
                "Scrolling",
                &["natural scrolling", "scroll direction", "invert"],
            ),
            item("tap", "Tap to Click", &["touchpad", "tapping"]),
            kcm_item(
                "shortcuts",
                "Keyboard Shortcuts",
                &["hotkeys", "keybindings", "keys"],
                "kcm_keys",
            ),
            adv(
                "primary",
                "Primary Button",
                &["left handed", "right handed"],
            ),
            adv("repeat", "Key Repeat", &["delay", "rate"]),
            adv("numlock", "Num Lock on Startup", &["numpad"]),
            adv_kcm(
                "virtual",
                "Virtual Keyboard",
                &["on-screen keyboard", "osk"],
                "kcm_virtualkeyboard",
            ),
        ],
        related: &[],
    },
    Page {
        id: "appearance",
        title: "Appearance",
        symbol: "Palette",
        kind: Kind::Native,
        keywords: &["theme", "look", "colors", "colours", "style", "desktop"],
        items: &[
            item(
                "style",
                "Light or Dark",
                &["dark mode", "light mode", "night"],
            ),
            item("accent", "Accent Color", &["highlight", "colour", "color"]),
            item("wallpaper", "Wallpaper", &["background", "picture"]),
            item(
                "transparency",
                "Transparency",
                &["blur", "translucent", "opaque"],
            ),
            item("dock", "Dock", &["taskbar", "pinned apps", "panel"]),
            adv("top-bar", "Top Bar", &["menu bar", "clock", "panel"]),
            adv("hot-corners", "Hot Corners", &["screen edges", "corner"]),
            adv("desktops", "Virtual Desktops", &["workspaces", "pager"]),
            adv("theme", "Global Theme", &["look and feel", "atlasos theme"]),
            adv_kcm(
                "fonts",
                "Fonts",
                &["font size", "typeface", "text"],
                "kcm_fonts",
            ),
            adv_kcm("icons", "Icons", &["icon theme"], "kcm_icons"),
            adv_kcm(
                "cursor",
                "Pointer Theme",
                &["cursor", "mouse pointer"],
                "kcm_cursortheme",
            ),
            adv_kcm(
                "effects",
                "Desktop Effects",
                &["animations", "kwin"],
                "kcm_kwin_effects",
            ),
            adv_kcm(
                "windows",
                "Window Behavior",
                &["focus", "titlebar", "double click"],
                "kcm_kwinoptions",
            ),
        ],
        related: &[],
    },
    Page {
        id: "notifications",
        title: "Notifications",
        symbol: "Notifications",
        kind: Kind::Native,
        keywords: &["alerts", "popups", "banners"],
        items: &[
            item("dnd", "Do Not Disturb", &["quiet", "silence", "focus"]),
            item("apps", "App Notifications", &["per app", "allow"]),
            adv("popups", "Popups", &["banner", "position", "timeout"]),
            adv("lock-screen", "On the Lock Screen", &["privacy"]),
        ],
        related: &[],
    },
    Page {
        id: "apps",
        title: "Apps",
        symbol: "Apps",
        kind: Kind::Native,
        keywords: &["default", "startup", "flatpak", "sandbox", "flatseal"],
        items: &[
            item(
                "defaults",
                "Default Apps",
                &[
                    "web browser",
                    "email",
                    "file manager",
                    "terminal",
                    "music",
                    "video",
                ],
            ),
            item(
                "autostart",
                "Startup Apps",
                &["autostart", "login", "run at login"],
            ),
            item(
                "permissions",
                "App Permissions",
                &[
                    "access",
                    "portal",
                    "file access",
                    "camera",
                    "microphone",
                    "network access",
                    "background apps",
                    "screen sharing",
                ],
            ),
            adv_kcm(
                "file-types",
                "File Associations",
                &["mime", "open with", "file types"],
                "kcm_filetypes",
            ),
        ],
        related: &[],
    },
    Page {
        id: "privacy",
        title: "Privacy & Security",
        symbol: "Security",
        kind: Kind::Native,
        keywords: &["tracking", "history", "security"],
        items: &[
            item(
                "screen-lock",
                "Screen Lock",
                &["lock", "lock screen", "password"],
            ),
            item(
                "location",
                "Location Services",
                &["gps", "geoclue", "where"],
            ),
            item(
                "firewall",
                "Firewall",
                &["firewalld", "block", "enable", "disable"],
            ),
            item(
                "crash-reports",
                "Crash Reports",
                &["crash", "bug reports", "telemetry"],
            ),
            adv(
                "firewall-rules",
                "Allowed Apps and Ports",
                &["port", "service", "open", "zone"],
            ),
            adv_kcm(
                "recent-files",
                "Recent Files",
                &["history", "remember"],
                "kcm_recentFiles",
            ),
            adv_kcm(
                "file-search",
                "File Search",
                &["baloo", "indexing"],
                "kcm_baloofile",
            ),
        ],
        related: &[],
    },
    Page {
        id: "users",
        title: "Users",
        symbol: "Group",
        kind: Kind::Native,
        keywords: &["account", "accounts", "login", "accountsservice"],
        items: &[
            item("name", "Full Name", &["display name"]),
            item("picture", "Profile Picture", &["avatar", "photo"]),
            item("password", "Password", &["change password", "sign in"]),
            item(
                "fingerprint",
                "Fingerprint",
                &["fprintd", "biometric", "enroll", "enrol"],
            ),
            item("add-user", "Add User", &["new account", "family"]),
            adv("auto-login", "Automatic Login", &["autologin", "sign in"]),
            adv_kcm(
                "login-screen",
                "Login Screen",
                &["greeter", "plasmalogin"],
                "kcm_plasmalogin",
            ),
        ],
        related: &[],
    },
    Page {
        id: "power",
        title: "Power & Battery",
        symbol: "BatteryFull",
        kind: Kind::Native,
        keywords: &["energy", "battery", "sleep", "suspend", "powerdevil"],
        items: &[
            item(
                "profile",
                "Power Mode",
                &[
                    "performance",
                    "balanced",
                    "power saver",
                    "power-profiles-daemon",
                ],
            ),
            item("battery", "Battery", &["charge", "health", "percentage"]),
            item(
                "screen-off",
                "Turn Off the Screen",
                &["blank", "dim", "idle"],
            ),
            item("sleep", "Sleep", &["suspend", "idle", "hibernate"]),
            adv("lid", "When the Lid Is Closed", &["laptop", "close"]),
            adv("power-button", "Power Button", &["shutdown", "press"]),
            adv(
                "charge-limit",
                "Charge Limit",
                &["battery health", "threshold"],
            ),
        ],
        related: &[],
    },
    Page {
        id: "accessibility",
        title: "Accessibility",
        symbol: "Accessibility",
        kind: Kind::Native,
        keywords: &["a11y", "vision", "hearing", "screen reader", "contrast"],
        items: &[
            item("text-size", "Text Size", &["large text", "font size"]),
            item("contrast", "High Contrast", &["visibility"]),
            item("reduce-motion", "Reduce Motion", &["animations"]),
            item("screen-reader", "Screen Reader", &["orca", "speech"]),
            item("zoom", "Zoom", &["magnifier"]),
            adv(
                "sticky-keys",
                "Sticky Keys",
                &["modifier keys", "slow keys", "bounce keys"],
            ),
        ],
        related: &[],
    },
    Page {
        id: "time-language",
        title: "Time & Language",
        symbol: "Language",
        kind: Kind::Native,
        keywords: &[
            "clock",
            "date",
            "time zone",
            "timezone",
            "calendar",
            "locale",
            "region",
        ],
        items: &[
            item(
                "automatic",
                "Set Time Automatically",
                &["ntp", "network time"],
            ),
            item("timezone", "Time Zone", &["zone", "region"]),
            item("language", "Language", &["translation", "locale"]),
            item(
                "time-format",
                "24-Hour Time",
                &["12 hour", "am pm", "clock format"],
            ),
            adv(
                "formats",
                "Formats",
                &["numbers", "currency", "date format", "measurement", "units"],
            ),
            adv(
                "system-language",
                "Login Screen Language",
                &["system locale", "new users", "localed"],
            ),
        ],
        related: &["input", "system"],
    },
    Page {
        id: "system",
        title: "System",
        symbol: "Info",
        kind: Kind::Native,
        keywords: &["about", "version", "hardware", "computer"],
        items: &[
            item("device-name", "Device Name", &["hostname", "computer name"]),
            item(
                "updates",
                "Updates",
                &["upgrade", "software update", "atlas updater", "bootc"],
            ),
            item("version", "AtlasOS Version", &["image", "bootc", "release"]),
            item(
                "hardware",
                "Hardware",
                &[
                    "processor",
                    "cpu",
                    "memory",
                    "ram",
                    "graphics",
                    "gpu",
                    "disk",
                ],
            ),
            adv(
                "other",
                "Other Plasma Settings",
                &["kde", "kcm", "system settings", "more settings"],
            ),
        ],
        related: &["time-language", "users", "privacy"],
    },
    // Not in the sidebar: System's "Other Plasma Settings" opens it, and
    // search and links can.
    Page {
        id: "more",
        title: "Other Plasma Settings",
        symbol: "Tune",
        kind: Kind::MoreSettings,
        keywords: &["kde", "kcm", "advanced", "system settings"],
        items: &[],
        related: &[],
    },
];
