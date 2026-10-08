//! The computer's own firmware (BIOS or UEFI): vendor, version and date, as
//! the kernel publishes them from the SMBIOS tables in
//! `/sys/class/dmi/id/bios_{vendor,version,date}` (readable by everyone).
//! The Updates page shows it in its firmware part, apart from fwupd's list of
//! updates: it is there with no fwupd too.
//!
//! The values come from the firmware, so they are untrusted text: control and
//! invisible characters are dropped, runs of spaces folded, and the lengths
//! capped. The date is "mm/dd/yyyy" (or "mm/dd/yy") by the SMBIOS rules and
//! becomes an ISO date the page formats for the person's locale; one that
//! doesn't parse is left out rather than shown wrong.

use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use zbus::zvariant::{OwnedValue, Value};

/// Where the kernel publishes it.
pub const DMI_DIR: &str = "/sys/class/dmi/id";

const MAX_VERSION: usize = 64;
const MAX_VENDOR: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemFirmware {
    pub vendor: String,
    pub version: String,
    /// `YYYY-MM-DD`, or "" when the firmware gave none that makes sense.
    pub date: String,
    /// Who made the computer (the board's maker on a PC built from parts), in
    /// the name people know ("ASRock"); "" when the DMI says nothing useful.
    pub maker: String,
    /// The computer's or board's model, for a web search; may be "".
    pub model: String,
    /// Where to look for newer firmware: see [`support`].
    pub support_url: String,
}

/// Drops control and invisible characters, folds whitespace, keeps at most
/// `max` characters.
fn clean(s: &str, max: usize) -> String {
    let kept: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|&c| {
            !c.is_control()
                && !matches!(c, '\u{ad}' | '\u{61c}' | '\u{180e}'
                    | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                    | '\u{2060}'..='\u{206f}' | '\u{feff}')
        })
        .collect();
    kept.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

/// "05/05/2025" (or "05/05/25") to "2025-05-05"; `None` for anything else.
/// A two-digit year is 19yy from 70 on, else 20yy.
pub fn iso_date(raw: &str) -> Option<String> {
    let mut parts = raw.trim().split('/');
    let (m, d, y) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || ![m, d, y]
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let (m, d): (u32, u32) = (m.parse().ok()?, d.parse().ok()?);
    let y: u32 = match y.len() {
        4 => y.parse().ok()?,
        2 => {
            let yy: u32 = y.parse().ok()?;
            if yy >= 70 { 1900 + yy } else { 2000 + yy }
        }
        _ => return None,
    };
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&m) || d == 0 || d > days[m as usize - 1] || !(1970..=2100).contains(&y) {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// What a maker's support page is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Support {
    /// "ASRock", or "" when the maker is not one that is known.
    pub name: String,
    /// An `https://` page: the maker's support site, or a web search.
    pub url: String,
}

/// The makers whose support pages are known: the start of a lower-case word
/// of the DMI maker string ("asus" is "ASUSTeK"), the name to show, and the page.
const MAKERS: &[(&str, &str, &str)] = &[
    ("asrock", "ASRock", "https://www.asrock.com/support/"),
    ("asus", "ASUS", "https://www.asus.com/support/"),
    ("micro-star", "MSI", "https://www.msi.com/support"),
    ("msi", "MSI", "https://www.msi.com/support"),
    ("gigabyte", "Gigabyte", "https://www.gigabyte.com/Support"),
    ("dell", "Dell", "https://www.dell.com/support/home"),
    ("lenovo", "Lenovo", "https://support.lenovo.com/"),
    ("hewlett", "HP", "https://support.hp.com/"),
    ("hp", "HP", "https://support.hp.com/"),
    (
        "framework",
        "Framework",
        "https://knowledgebase.frame.work/",
    ),
];

/// DMI strings that mean "the maker did not fill this in".
fn placeholder(s: &str) -> bool {
    let l = s.to_lowercase();
    l.is_empty()
        || [
            "to be filled",
            "system manufacturer",
            "system product name",
            "default string",
            "not specified",
            "o.e.m.",
            "unknown",
            "none",
            "n/a",
        ]
        .iter()
        .any(|p| l.contains(p))
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The support page for a computer from its DMI maker and model. The maker
/// words are matched whole (`hp` is not the middle of "chp"), the PC's own
/// maker first and then the board's. A maker that is not known gets a web
/// search for "<maker> <model> BIOS update".
pub fn support(sys_vendor: &str, board_vendor: &str, model: &str) -> Support {
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '-')
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    for candidate in [sys_vendor, board_vendor] {
        if placeholder(candidate) {
            continue;
        }
        let w = words(candidate);
        if let Some((_, name, url)) = MAKERS
            .iter()
            .find(|(key, _, _)| w.iter().any(|x| x.starts_with(key)))
        {
            return Support {
                name: (*name).into(),
                url: (*url).into(),
            };
        }
    }
    let maker = [sys_vendor, board_vendor]
        .into_iter()
        .find(|v| !placeholder(v))
        .unwrap_or("");
    let query = [
        maker,
        if placeholder(model) { "" } else { model },
        "BIOS update",
    ]
    .iter()
    .filter(|p| !p.is_empty())
    .copied()
    .collect::<Vec<_>>()
    .join(" ");
    Support {
        name: String::new(),
        url: format!("https://duckduckgo.com/?q={}", percent_encode(&query)),
    }
}

/// From the raw values. `None` without a version: there is nothing to show then.
pub fn parse(vendor: &str, version: &str, date: &str) -> Option<SystemFirmware> {
    let version = clean(version, MAX_VERSION);
    if version.is_empty() {
        return None;
    }
    Some(SystemFirmware {
        vendor: clean(vendor, MAX_VENDOR),
        version,
        date: iso_date(date).unwrap_or_default(),
        maker: String::new(),
        model: String::new(),
        support_url: String::new(),
    })
}

/// Reads the files of `dir`; `None` when there is no version.
pub fn read(dir: &Path) -> Option<SystemFirmware> {
    let get = |name: &str| {
        clean(
            &std::fs::read_to_string(dir.join(name)).unwrap_or_default(),
            MAX_VENDOR,
        )
    };
    let mut f = parse(&get("bios_vendor"), &get("bios_version"), &get("bios_date"))?;
    let (sys, board) = (get("sys_vendor"), get("board_vendor"));
    let model = if placeholder(&get("product_name")) {
        get("board_name")
    } else {
        get("product_name")
    };
    let help = support(&sys, &board, &model);
    f.maker = help.name;
    f.support_url = help.url;
    f.model = if placeholder(&model) {
        String::new()
    } else {
        model
    };
    Some(f)
}

impl SystemFirmware {
    /// The JSON the page reads: `{vendor, version, date, maker, model}`.
    pub fn to_json(&self) -> String {
        json!({"vendor": self.vendor, "version": self.version, "date": self.date,
               "maker": self.maker, "model": self.model, "supportUrl": self.support_url})
        .to_string()
    }
}

// ---------------------------------------------------- what fwupd knows

/// What the Updates page may say about the computer's own firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// fwupd has releases for it and none is newer: it is up to date.
    UpToDate,
    /// fwupd offers a newer release.
    UpdateAvailable,
    /// fwupd has no release metadata for it at all (the maker does not publish
    /// to the Linux Vendor Firmware Service): nothing here can say whether it
    /// is up to date.
    NoMetadata,
    /// fwupd could not be asked, or does not list the firmware as a device.
    Unknown,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::UpToDate => "upToDate",
            Verdict::UpdateAvailable => "updateAvailable",
            Verdict::NoMetadata => "noMetadata",
            Verdict::Unknown => "unknown",
        }
    }
}

/// What `GetReleases` for the device answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Releases {
    /// That many releases (the installed one among them, if fwupd knows it).
    Found(usize),
    /// `org.freedesktop.fwupd.NothingToDo`: "No releases found".
    NothingToDo,
    /// Any other error, or no answer.
    Failed,
}

/// The decision. `device` is whether fwupd lists the system firmware,
/// `offered` whether its check found a newer release for it.
pub fn verdict(device: bool, offered: bool, releases: Releases) -> Verdict {
    if !device {
        return Verdict::Unknown;
    }
    if offered {
        return Verdict::UpdateAvailable;
    }
    match releases {
        Releases::Found(n) if n > 0 => Verdict::UpToDate,
        Releases::Found(_) | Releases::NothingToDo => Verdict::NoMetadata,
        Releases::Failed => Verdict::Unknown,
    }
}

pub type Dict = HashMap<String, OwnedValue>;

fn plain<'a>(d: &'a Dict, k: &str) -> Option<&'a Value<'static>> {
    let mut v: &Value<'static> = d.get(k)?;
    for _ in 0..2 {
        if let Value::Value(inner) = v {
            v = inner;
        }
    }
    Some(v)
}

fn text<'a>(d: &'a Dict, k: &str) -> Option<&'a str> {
    match plain(d, k)? {
        Value::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Whether a `GetDevices` row is the computer's own firmware: fwupd's UEFI
/// capsule device (plugin `uefi_capsule`, the host firmware's), or a device
/// that calls itself "System Firmware" with the computer icon.
pub fn is_system_firmware(d: &Dict) -> bool {
    let plugin = text(d, "Plugin").unwrap_or("");
    let icon_computer = matches!(plain(d, "Icon"), Some(Value::Array(a))
        if a.iter().any(|v| matches!(v, Value::Str(s) if s.as_str() == "computer")
            || matches!(v, Value::Value(i) if matches!(&**i, Value::Str(s) if s.as_str() == "computer"))));
    matches!(plugin, "uefi_capsule" | "uefi-capsule")
        || (text(d, "Name") == Some("System Firmware") && icon_computer)
}

/// The device ID of a `GetDevices` row, if it is one fwupd could be asked about.
pub fn device_id(d: &Dict) -> Option<&str> {
    text(d, "DeviceId").filter(|id| {
        !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// Asks fwupd (on `conn`, the system bus) about the system firmware.
/// `offered` are the device IDs a check found updates for.
pub async fn ask_fwupd(conn: &zbus::Connection, offered: &[String]) -> Verdict {
    const WAIT: Duration = Duration::from_secs(15);
    async fn bounded<T>(
        f: impl std::future::Future<Output = zbus::Result<T>>,
    ) -> Option<zbus::Result<T>> {
        tokio::time::timeout(WAIT, f).await.ok()
    }
    let Ok(builder) = zbus::proxy::Builder::<zbus::Proxy>::new(conn)
        .destination("org.freedesktop.fwupd")
        .and_then(|b| b.path("/"))
        .and_then(|b| b.interface("org.freedesktop.fwupd"))
        .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
    else {
        return Verdict::Unknown;
    };
    let Ok(p) = builder.build().await else {
        return Verdict::Unknown;
    };
    let Some(Ok(rows)) = bounded(p.call::<_, _, Vec<Dict>>("GetDevices", &())).await else {
        return Verdict::Unknown;
    };
    let Some(id) = rows
        .iter()
        .filter(|d| is_system_firmware(d))
        .find_map(device_id)
        .map(str::to_string)
    else {
        return verdict(false, false, Releases::Failed);
    };
    let releases = match bounded(p.call::<_, _, Vec<Dict>>("GetReleases", &(id.as_str(),))).await {
        Some(Ok(r)) => Releases::Found(r.len()),
        Some(Err(zbus::Error::MethodError(n, _, _)))
            if n.as_str() == "org.freedesktop.fwupd.NothingToDo" =>
        {
            Releases::NothingToDo
        }
        _ => Releases::Failed,
    };
    verdict(true, offered.contains(&id), releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(rows: &[(&str, Value<'static>)]) -> Dict {
        rows.iter()
            .map(|(k, v)| ((*k).to_string(), OwnedValue::try_from(v.clone()).unwrap()))
            .collect()
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("telamon-dmi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_owners_firmware() {
        let f = parse(
            "American Megatrends International, LLC.\n",
            "11.02\n",
            "05/05/2025\n",
        )
        .unwrap();
        assert_eq!(f.vendor, "American Megatrends International, LLC.");
        assert_eq!(f.version, "11.02");
        assert_eq!(f.date, "2025-05-05");
    }

    #[test]
    fn dates() {
        assert_eq!(iso_date("12/31/1999").as_deref(), Some("1999-12-31"));
        assert_eq!(iso_date("01/02/98").as_deref(), Some("1998-01-02"));
        assert_eq!(iso_date("01/02/09").as_deref(), Some("2009-01-02"));
        assert_eq!(iso_date("02/29/2024").as_deref(), Some("2024-02-29"));
        for bad in [
            "",
            "2025-05-05",
            "13/01/2025",
            "00/10/2025",
            "02/30/2025",
            "02/29/2023",
            "05/05",
            "05/05/2025/1",
            "5/5/202",
            "aa/bb/cccc",
            "05/05/1969",
            "05/00/2025",
            "-1/01/2025",
            "05/05/ 2025",
        ] {
            assert_eq!(iso_date(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn no_version_nothing_to_show_and_a_bad_date_is_left_out() {
        assert_eq!(parse("Acme", "", "05/05/2025"), None);
        assert_eq!(parse("Acme", " \n\t", "05/05/2025"), None);
        let f = parse("", "F.12", "garbage").unwrap();
        assert_eq!(
            (f.vendor.as_str(), f.version.as_str(), f.date.as_str()),
            ("", "F.12", "")
        );
    }

    #[test]
    fn firmware_text_is_untrusted() {
        let f = parse(
            "Ev\u{202e}il\u{1b}[31m  Corp\r\n",
            "1.\u{0}0\u{200b}.3   beta",
            "05/05/2025",
        )
        .unwrap();
        assert_eq!(f.vendor, "Evil[31m Corp");
        assert_eq!(f.version, "1.0.3 beta");
        let long = parse(&"v".repeat(500), &"9".repeat(500), "").unwrap();
        assert_eq!(long.vendor.chars().count(), MAX_VENDOR);
        assert_eq!(long.version.chars().count(), MAX_VERSION);
    }

    #[test]
    fn read_the_owners_board() {
        let d = temp("asrock");
        for (k, v) in [
            ("bios_vendor", "American Megatrends International, LLC.\n"),
            ("bios_version", "11.02\n"),
            ("bios_date", "05/05/2025\n"),
            ("sys_vendor", "To Be Filled By O.E.M.\n"),
            ("product_name", "To Be Filled By O.E.M.\n"),
            ("board_vendor", "ASRock\n"),
            ("board_name", "Z790 Lightning WiFi\n"),
        ] {
            std::fs::write(d.join(k), v).unwrap();
        }
        let f = read(&d).unwrap();
        assert_eq!(
            (f.version.as_str(), f.date.as_str()),
            ("11.02", "2025-05-05")
        );
        assert_eq!(f.maker, "ASRock");
        assert_eq!(f.model, "Z790 Lightning WiFi");
        assert_eq!(f.support_url, "https://www.asrock.com/support/");
        let json: serde_json::Value = serde_json::from_str(&f.to_json()).unwrap();
        assert_eq!(json["version"], "11.02");
        assert_eq!(json["supportUrl"], "https://www.asrock.com/support/");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn read_with_missing_files() {
        let d = temp("none");
        assert_eq!(read(&d), None, "no files");
        std::fs::write(d.join("bios_version"), "11.02\n").unwrap();
        let f = read(&d).unwrap();
        assert_eq!(
            (
                f.version.as_str(),
                f.vendor.as_str(),
                f.date.as_str(),
                f.maker.as_str()
            ),
            ("11.02", "", "", "")
        );
        assert!(f.support_url.starts_with("https://"), "{}", f.support_url);
        assert_eq!(read(Path::new("/nonexistent/dmi")), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn makers_have_support_pages() {
        for (sys, board, name, host) in [
            (
                "ASUSTeK COMPUTER INC.",
                "ASUSTeK COMPUTER INC.",
                "ASUS",
                "www.asus.com",
            ),
            (
                "Micro-Star International Co., Ltd.",
                "Micro-Star International Co., Ltd.",
                "MSI",
                "www.msi.com",
            ),
            ("MSI", "", "MSI", "www.msi.com"),
            (
                "Gigabyte Technology Co., Ltd.",
                "",
                "Gigabyte",
                "www.gigabyte.com",
            ),
            ("Dell Inc.", "Dell Inc.", "Dell", "www.dell.com"),
            ("LENOVO", "LENOVO", "Lenovo", "support.lenovo.com"),
            ("HP", "HP", "HP", "support.hp.com"),
            ("Hewlett-Packard", "", "HP", "support.hp.com"),
            (
                "Framework",
                "Framework",
                "Framework",
                "knowledgebase.frame.work",
            ),
            // a PC built from parts: the board's maker
            ("System manufacturer", "ASRock", "ASRock", "www.asrock.com"),
            (
                "To Be Filled By O.E.M.",
                "ASRock",
                "ASRock",
                "www.asrock.com",
            ),
        ] {
            let s = support(sys, board, "Model 1");
            assert_eq!(s.name, name, "{sys} / {board}");
            assert!(s.url.starts_with(&format!("https://{host}/")), "{}", s.url);
        }
    }

    #[test]
    fn an_unknown_maker_gets_a_web_search() {
        let s = support("Acme Computers Ltd.", "Acme", "Rocket 9 & Co");
        assert_eq!(s.name, "");
        assert_eq!(
            s.url,
            "https://duckduckgo.com/?q=Acme%20Computers%20Ltd.%20Rocket%209%20%26%20Co%20BIOS%20update"
        );
        // nothing known at all: still a search, never an empty or odd link
        let s = support(
            "To Be Filled By O.E.M.",
            "Default string",
            "System Product Name",
        );
        assert_eq!(s.url, "https://duckduckgo.com/?q=BIOS%20update");
        // a maker's name only inside a word is not that maker
        assert_eq!(support("Chpwidgets", "", "").name, "");
        assert_eq!(support("Shell Dell Co", "", "").name, "Dell");
    }

    #[test]
    fn the_decision_is_honest_about_what_fwupd_knows() {
        use Releases::*;
        use Verdict::*;
        // up to date only when fwupd has releases and offers none newer
        assert_eq!(verdict(true, false, Found(3)), UpToDate);
        assert_eq!(verdict(true, false, Found(1)), UpToDate);
        // an update wins over everything
        assert_eq!(verdict(true, true, Found(3)), UpdateAvailable);
        assert_eq!(verdict(true, true, NothingToDo), UpdateAvailable);
        // the owner's ASRock: listed, updatable, no releases in any remote
        assert_eq!(verdict(true, false, NothingToDo), NoMetadata);
        assert_eq!(verdict(true, false, Found(0)), NoMetadata);
        // could not tell
        assert_eq!(verdict(true, false, Failed), Unknown);
        assert_eq!(verdict(false, false, Found(5)), Unknown);
        assert_eq!(verdict(false, true, NothingToDo), Unknown);
        for v in [UpToDate, UpdateAvailable, NoMetadata, Unknown] {
            assert!(!v.word().is_empty());
        }
    }

    #[test]
    fn the_system_firmware_device_in_fwupds_list() {
        let s = |t: &str| Value::from(t.to_string());
        let icon = |names: &[&str]| {
            Value::from(
                names
                    .iter()
                    .map(|n| Value::from((*n).to_string()))
                    .collect::<Vec<_>>(),
            )
        };
        // as fwupd lists the owner's board
        let board = dict(&[
            ("Name", s("System Firmware")),
            ("DeviceId", s("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678")),
            ("Plugin", s("uefi_capsule")),
            ("Version", s("1102")),
        ]);
        assert!(is_system_firmware(&board));
        assert_eq!(
            device_id(&board),
            Some("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678")
        );
        // by its name and icon when the plugin is not given
        let named = dict(&[
            ("Name", s("System Firmware")),
            ("Icon", icon(&["computer"])),
            ("DeviceId", s("abc123")),
        ]);
        assert!(is_system_firmware(&named));
        // others are not it, whatever they call themselves
        let ssd = dict(&[
            ("Name", s("System Firmware")),
            ("Plugin", s("nvme")),
            ("DeviceId", s("def456")),
        ]);
        assert!(!is_system_firmware(&ssd));
        let dock = dict(&[
            ("Name", s("Dock")),
            ("Icon", icon(&["computer"])),
            ("Plugin", s("synaptics_mst")),
        ]);
        assert!(!is_system_firmware(&dock));
        // an ID that could be anything is not asked about
        let odd = dict(&[("DeviceId", s("../x")), ("Plugin", s("uefi_capsule"))]);
        assert_eq!(device_id(&odd), None);
        assert_eq!(device_id(&dict(&[])), None);
        // values wrapped in a variant read the same
        let wrapped = dict(&[("Plugin", Value::Value(Box::new(s("uefi_capsule"))))]);
        assert!(is_system_firmware(&wrapped));
    }
}
