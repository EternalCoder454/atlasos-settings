//! The pages, in sidebar order, and the settings on each that search finds.
//!
//! Titles are English. No Atlas app ships translations yet; when they come,
//! these strings are extracted for Qt Linguist from here (docs/DESIGN.md).

/// A sidebar section. The order of [`Group::ALL`] is the sidebar's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Connections,
    Devices,
    Personalization,
    Apps,
    AccountsPrivacy,
    System,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Connections,
        Group::Devices,
        Group::Personalization,
        Group::Apps,
        Group::AccountsPrivacy,
        Group::System,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Group::Connections => "connections",
            Group::Devices => "devices",
            Group::Personalization => "personalization",
            Group::Apps => "apps",
            Group::AccountsPrivacy => "accounts-privacy",
            Group::System => "system",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Group::Connections => "Connections",
            Group::Devices => "Devices",
            Group::Personalization => "Personalization",
            Group::Apps => "Apps",
            Group::AccountsPrivacy => "Accounts & Privacy",
            Group::System => "System",
        }
    }
}

/// How a page is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Drawn by Settings.
    Native,
    /// Plasma's KCM of that name, in its own kcmshell6 window.
    Kcm(&'static str),
    /// Another program, by its command name (Updates opens Atlas Updater).
    App(&'static str),
    /// The list of the KCMs Settings has no page for.
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    pub group: Group,
    /// An Atlas.Ui `Symbols` name (`Symbols.codepoint(name)` in QML).
    pub symbol: &'static str,
    pub kind: Kind,
    /// Other words people search with (lower case).
    pub keywords: &'static [&'static str],
    pub items: &'static [Item],
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

const fn item(id: &'static str, title: &'static str, keywords: &'static [&'static str]) -> Item {
    Item {
        id,
        title,
        keywords,
        kcm: None,
    }
}

const fn kcm_item(
    id: &'static str,
    title: &'static str,
    keywords: &'static [&'static str],
    kcm: &'static str,
) -> Item {
    Item {
        id,
        title,
        keywords,
        kcm: Some(kcm),
    }
}

pub static PAGES: &[Page] = &[
    // Connections
    Page {
        id: "network",
        title: "Wi-Fi & Network",
        group: Group::Connections,
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
            kcm_item("proxy", "Proxy", &["http proxy", "socks"], "kcm_proxy"),
            kcm_item(
                "cellular",
                "Mobile Broadband",
                &["cellular", "modem", "sim", "lte"],
                "kcm_cellular_network",
            ),
        ],
    },
    Page {
        id: "bluetooth",
        title: "Bluetooth",
        group: Group::Connections,
        symbol: "Bluetooth",
        kind: Kind::Native,
        keywords: &["bluez", "pair", "headphones", "wireless"],
        items: &[
            item("power", "Bluetooth On or Off", &["enable", "disable"]),
            item(
                "devices",
                "Devices",
                &["pair", "connect", "headset", "keyboard", "mouse"],
            ),
            item(
                "visibility",
                "Visible to Other Devices",
                &["discoverable", "name"],
            ),
        ],
    },
    Page {
        id: "firewall",
        title: "Firewall",
        group: Group::Connections,
        symbol: "Security",
        kind: Kind::Native,
        keywords: &["firewalld", "ports", "zone", "block"],
        items: &[
            item("enabled", "Firewall On or Off", &["enable", "disable"]),
            item(
                "rules",
                "Allowed Apps and Ports",
                &["port", "service", "open"],
            ),
        ],
    },
    // Devices
    Page {
        id: "displays",
        title: "Displays",
        group: Group::Devices,
        symbol: "Monitor",
        kind: Kind::Native,
        keywords: &["monitor", "screen", "kscreen"],
        items: &[
            item("resolution", "Resolution", &["size", "pixels"]),
            item(
                "refresh-rate",
                "Refresh Rate",
                &["hz", "fps", "vrr", "adaptive sync"],
            ),
            item("scale", "Scale", &["zoom", "dpi", "hidpi", "text size"]),
            item(
                "arrangement",
                "Arrangement",
                &["position", "multiple monitors", "primary"],
            ),
            item("orientation", "Orientation", &["rotate", "portrait"]),
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
            item("hdr", "HDR", &["high dynamic range", "brightness"]),
        ],
    },
    Page {
        id: "sound",
        title: "Sound",
        group: Group::Devices,
        symbol: "VolumeUp",
        kind: Kind::Native,
        keywords: &["audio", "volume", "pipewire", "pulseaudio", "speakers"],
        items: &[
            item(
                "output",
                "Output Device",
                &["speakers", "headphones", "hdmi"],
            ),
            item("input", "Input Device", &["microphone", "mic", "recording"]),
            item("volume", "Volume", &["loud", "mute"]),
            item("apps", "App Volume", &["mixer", "per app"]),
            kcm_item(
                "sounds",
                "System Sounds",
                &["alert", "event sounds", "theme"],
                "kcm_soundtheme",
            ),
        ],
    },
    Page {
        id: "printers",
        title: "Printers",
        group: Group::Devices,
        symbol: "Print",
        kind: Kind::Kcm("kcm_printer_manager"),
        keywords: &["printer", "cups", "scanner", "print queue"],
        items: &[],
    },
    Page {
        id: "keyboard",
        title: "Keyboard",
        group: Group::Devices,
        symbol: "Keyboard",
        kind: Kind::Native,
        keywords: &["typing", "keys"],
        items: &[
            item("layouts", "Input Sources", &["layout", "language", "xkb"]),
            item("repeat", "Key Repeat", &["delay", "rate"]),
            item("numlock", "Num Lock on Startup", &["numpad"]),
            kcm_item(
                "shortcuts",
                "Keyboard Shortcuts",
                &["hotkeys", "keybindings", "keys"],
                "kcm_keys",
            ),
            kcm_item(
                "virtual",
                "Virtual Keyboard",
                &["on-screen keyboard", "osk"],
                "kcm_virtualkeyboard",
            ),
        ],
    },
    Page {
        id: "mouse",
        title: "Mouse & Touchpad",
        group: Group::Devices,
        symbol: "Mouse",
        kind: Kind::Native,
        keywords: &["pointer", "trackpad", "cursor", "click"],
        items: &[
            item("speed", "Pointer Speed", &["acceleration", "sensitivity"]),
            item(
                "scrolling",
                "Scrolling",
                &["natural scrolling", "scroll direction", "invert"],
            ),
            item("tap", "Tap to Click", &["touchpad", "tapping"]),
            item(
                "primary",
                "Primary Button",
                &["left handed", "right handed"],
            ),
            kcm_item(
                "tablet",
                "Drawing Tablet",
                &["wacom", "pen", "stylus"],
                "kcm_tablet",
            ),
            kcm_item("touchscreen", "Touchscreen", &["touch"], "kcm_touchscreen"),
            kcm_item(
                "controller",
                "Game Controller",
                &["gamepad", "joystick"],
                "kcm_gamecontroller",
            ),
        ],
    },
    Page {
        id: "power",
        title: "Power & Battery",
        group: Group::Devices,
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
            item("lid", "When the Lid Is Closed", &["laptop", "close"]),
            item("power-button", "Power Button", &["shutdown", "press"]),
            item(
                "charge-limit",
                "Charge Limit",
                &["battery health", "threshold"],
            ),
        ],
    },
    // Personalization
    Page {
        id: "appearance",
        title: "Appearance",
        group: Group::Personalization,
        symbol: "Palette",
        kind: Kind::Native,
        keywords: &["theme", "look", "colors", "colours", "style"],
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
            item("theme", "Global Theme", &["look and feel", "atlasos theme"]),
            kcm_item(
                "fonts",
                "Fonts",
                &["font size", "typeface", "text"],
                "kcm_fonts",
            ),
            kcm_item("icons", "Icons", &["icon theme"], "kcm_icons"),
            kcm_item(
                "cursor",
                "Pointer Theme",
                &["cursor", "mouse pointer"],
                "kcm_cursortheme",
            ),
        ],
    },
    Page {
        id: "desktop",
        title: "Desktop & Dock",
        group: Group::Personalization,
        symbol: "DesktopWindows",
        kind: Kind::Native,
        keywords: &["panel", "taskbar", "dock", "top bar", "workspace"],
        items: &[
            item("dock", "Dock", &["taskbar", "pinned apps", "panel"]),
            item("top-bar", "Top Bar", &["menu bar", "clock", "panel"]),
            item("hot-corners", "Hot Corners", &["screen edges", "corner"]),
            item("desktops", "Virtual Desktops", &["workspaces", "pager"]),
            kcm_item(
                "effects",
                "Desktop Effects",
                &["animations", "kwin"],
                "kcm_kwin_effects",
            ),
            kcm_item(
                "windows",
                "Window Behavior",
                &["focus", "titlebar", "double click"],
                "kcm_kwinoptions",
            ),
        ],
    },
    Page {
        id: "notifications",
        title: "Notifications",
        group: Group::Personalization,
        symbol: "Notifications",
        kind: Kind::Native,
        keywords: &["alerts", "popups", "do not disturb", "banners"],
        items: &[
            item("dnd", "Do Not Disturb", &["quiet", "silence", "focus"]),
            item("popups", "Popups", &["banner", "position", "timeout"]),
            item("apps", "App Notifications", &["per app", "allow"]),
            item("lock-screen", "On the Lock Screen", &["privacy"]),
        ],
    },
    // Apps
    Page {
        id: "default-apps",
        title: "Default Apps & Autostart",
        group: Group::Apps,
        symbol: "Apps",
        kind: Kind::Native,
        keywords: &[
            "default",
            "browser",
            "email",
            "file associations",
            "startup",
        ],
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
                "Autostart",
                &["startup", "login", "run at login"],
            ),
            kcm_item(
                "file-types",
                "File Associations",
                &["mime", "open with", "file types"],
                "kcm_filetypes",
            ),
        ],
    },
    Page {
        id: "app-permissions",
        title: "App Permissions",
        group: Group::Apps,
        symbol: "Shield",
        kind: Kind::Native,
        keywords: &["flatpak", "sandbox", "access", "portal", "flatseal"],
        items: &[
            item("files", "File Access", &["home", "folders", "filesystem"]),
            item(
                "devices",
                "Devices",
                &["camera", "microphone", "gpu", "usb"],
            ),
            item("network", "Network Access", &["internet"]),
            item("background", "Background Apps", &["run in background"]),
            item("screen", "Screen Sharing", &["screencast", "screenshot"]),
        ],
    },
    // Accounts & Privacy
    Page {
        id: "users",
        title: "Users",
        group: Group::AccountsPrivacy,
        symbol: "Group",
        kind: Kind::Native,
        keywords: &["account", "accounts", "login", "accountsservice"],
        items: &[
            item("password", "Password", &["change password", "sign in"]),
            item(
                "fingerprint",
                "Fingerprint",
                &["fprintd", "biometric", "enroll", "enrol"],
            ),
            item("picture", "Profile Picture", &["avatar", "photo"]),
            item("name", "Full Name", &["display name"]),
            item("auto-login", "Automatic Login", &["autologin", "sign in"]),
            item("add-user", "Add User", &["new account", "family"]),
            kcm_item(
                "login-screen",
                "Login Screen",
                &["greeter", "plasmalogin"],
                "kcm_plasmalogin",
            ),
        ],
    },
    Page {
        id: "privacy",
        title: "Privacy",
        group: Group::AccountsPrivacy,
        symbol: "PrivacyTip",
        kind: Kind::Native,
        keywords: &["tracking", "history"],
        items: &[
            item(
                "crash-reports",
                "Crash Reports",
                &["crash", "bug reports", "telemetry"],
            ),
            item(
                "location",
                "Location Services",
                &["gps", "geoclue", "where"],
            ),
            item(
                "screen-lock",
                "Screen Lock",
                &["lock", "lock screen", "password"],
            ),
            kcm_item(
                "recent-files",
                "Recent Files",
                &["history", "remember"],
                "kcm_recentFiles",
            ),
            kcm_item(
                "file-search",
                "File Search",
                &["baloo", "indexing"],
                "kcm_baloofile",
            ),
        ],
    },
    // System
    Page {
        id: "accessibility",
        title: "Accessibility",
        group: Group::System,
        symbol: "Accessibility",
        kind: Kind::Native,
        keywords: &["a11y", "vision", "hearing", "screen reader", "contrast"],
        items: &[
            item("text-size", "Text Size", &["large text", "font size"]),
            item("contrast", "High Contrast", &["visibility"]),
            item("reduce-motion", "Reduce Motion", &["animations"]),
            item("screen-reader", "Screen Reader", &["orca", "speech"]),
            item("zoom", "Zoom", &["magnifier"]),
            item(
                "sticky-keys",
                "Sticky Keys",
                &["modifier keys", "slow keys", "bounce keys"],
            ),
        ],
    },
    Page {
        id: "datetime",
        title: "Date & Time",
        group: Group::System,
        symbol: "Schedule",
        kind: Kind::Native,
        keywords: &["clock", "time zone", "timezone", "ntp", "calendar"],
        items: &[
            item(
                "automatic",
                "Set Time Automatically",
                &["ntp", "network time"],
            ),
            item("timezone", "Time Zone", &["zone", "region"]),
            item(
                "format",
                "24-Hour Time",
                &["12 hour", "am pm", "clock format"],
            ),
        ],
    },
    Page {
        id: "region",
        title: "Language & Region",
        group: Group::System,
        symbol: "Language",
        kind: Kind::Native,
        keywords: &["locale", "translation", "formats", "units"],
        items: &[
            item("language", "Language", &["translation", "locale"]),
            item(
                "formats",
                "Formats",
                &["numbers", "currency", "date format", "measurement"],
            ),
        ],
    },
    Page {
        id: "updates",
        title: "Updates",
        group: Group::System,
        symbol: "Update",
        kind: Kind::App("atlas-updater"),
        keywords: &["upgrade", "software update", "atlas updater", "bootc"],
        items: &[],
    },
    Page {
        id: "about",
        title: "About",
        group: Group::System,
        symbol: "Info",
        kind: Kind::Native,
        keywords: &[
            "system",
            "version",
            "hardware",
            "computer",
            "hostname",
            "device name",
        ],
        items: &[
            item("device-name", "Device Name", &["hostname", "computer name"]),
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
        ],
    },
    Page {
        id: "more",
        title: "More Settings",
        group: Group::System,
        symbol: "Tune",
        kind: Kind::MoreSettings,
        keywords: &["kde", "kcm", "advanced", "system settings"],
        items: &[],
    },
];
