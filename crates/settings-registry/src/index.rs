//! The search index the Telamon OS Launcher reads
//! (`/usr/share/telamon-settings/search-index.json`, format version 1).
//!
//! It lists every page and every setting of the registry, so the Launcher
//! finds "Night Light" or "Wi-Fi" and opens Settings there with
//! `org.freedesktop.Application.ActivateAction("open", [link])`; the links
//! are what [`launch::action_args`](crate::launch::action_args) reads. The
//! package build writes the file with `gen-search-index`, so it can't drift
//! from the pages.
//!
//! The Launcher treats the file as untrusted and caps it (2 MiB, 5,000
//! entries, links of 128 bytes, titles of 128 characters, 64 keywords of 64
//! characters); `tests/index.rs` keeps ours well inside those.

use crate::launch::valid_link;
use crate::pages::{PAGES, Page};

/// The D-Bus name and application ID of Settings, written into the file.
pub const APP_ID: &str = "net.eterneon.telamon.settings";

/// Where the package installs the file.
pub const INSTALL_PATH: &str = "/usr/share/telamon-settings/search-index.json";

/// The theme icon the Launcher shows for a page and its settings. A page
/// without a line here gets [`FALLBACK_ICON`]; `tests/index.rs` makes every
/// page have one.
pub fn icon(page: &str) -> &'static str {
    match page {
        "home" => "go-home",
        "network" => "network-wireless",
        "devices" => "preferences-system-bluetooth",
        "displays" => "preferences-desktop-display",
        "sound" => "audio-volume-high",
        "input" => "input-keyboard",
        "appearance" => "preferences-desktop-theme",
        "notifications" => "preferences-desktop-notification",
        "apps" => "preferences-desktop-default-applications",
        "privacy" => "preferences-system-privacy",
        "users" => "system-users",
        "power" => "preferences-system-power",
        "accessibility" => "preferences-desktop-accessibility",
        "time-language" => "preferences-system-time",
        "system" => "computer",
        "updates" => "system-software-update",
        "more" => "preferences-other",
        _ => FALLBACK_ICON,
    }
}

/// The icon of a page `icon` has no line for (the Launcher's own fallback).
pub const FALLBACK_ICON: &str = "preferences-system";

/// One line of the index: a page, or a setting on one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `page` or `page/item`.
    pub link: String,
    pub is_page: bool,
    pub page: &'static str,
    pub item: Option<&'static str>,
    pub title: &'static str,
    /// For a setting, where it lives: "Displays", or "Displays › Advanced"
    /// when the page folds it away.
    pub parent: Option<String>,
    pub icon: &'static str,
    pub keywords: Vec<&'static str>,
}

fn page_entries(page: &'static Page, out: &mut Vec<Entry>) {
    out.push(Entry {
        link: page.id.to_string(),
        is_page: true,
        page: page.id,
        item: None,
        title: page.title,
        parent: None,
        icon: icon(page.id),
        keywords: page.keywords.to_vec(),
    });
    for item in page.items {
        out.push(Entry {
            link: format!("{}/{}", page.id, item.id),
            is_page: false,
            page: page.id,
            item: Some(item.id),
            title: item.title,
            parent: Some(if item.advanced {
                format!("{} \u{203a} Advanced", page.title)
            } else {
                page.title.to_string()
            }),
            icon: icon(page.id),
            keywords: item.keywords.to_vec(),
        });
    }
}

/// Every page, each followed by its settings, in the registry's order.
pub fn entries() -> Vec<Entry> {
    let mut out = Vec::new();
    for page in PAGES {
        page_entries(page, &mut out);
    }
    out
}

/// `s` as a JSON string, quotes included.
fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A `{"C": ...}` map, the only locale there is until Settings is translated.
fn c_string(out: &mut String, s: &str) {
    out.push_str("{\"C\":");
    string(out, s);
    out.push('}');
}

/// The whole file, one entry per line.
pub fn to_json() -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str("{\"version\":1,\"app\":");
    string(&mut out, APP_ID);
    out.push_str(",\"entries\":[\n");
    let entries = entries();
    for (n, e) in entries.iter().enumerate() {
        out.push_str("{\"id\":");
        string(&mut out, &e.link);
        out.push_str(",\"kind\":");
        string(&mut out, if e.is_page { "page" } else { "setting" });
        out.push_str(",\"page\":");
        string(&mut out, e.page);
        out.push_str(",\"item\":");
        match e.item {
            Some(i) => string(&mut out, i),
            None => out.push_str("null"),
        }
        out.push_str(",\"title\":");
        c_string(&mut out, e.title);
        out.push_str(",\"parent\":");
        match &e.parent {
            Some(p) => c_string(&mut out, p),
            None => out.push_str("null"),
        }
        out.push_str(",\"icon\":");
        string(&mut out, e.icon);
        out.push_str(",\"keywords\":{\"C\":[");
        for (i, k) in e.keywords.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            string(&mut out, k);
        }
        out.push_str("]},\"link\":");
        string(&mut out, &e.link);
        out.push('}');
        if n + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("]}\n");
    out
}

/// Whether every entry has a link the Launcher accepts. For tests and the
/// generator, which refuses to write a file the Launcher would skip.
pub fn links_valid() -> bool {
    entries().iter().all(|e| valid_link(&e.link))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        let mut s = String::new();
        string(&mut s, "a\"b\\c\n\u{1}\u{203a}");
        assert_eq!(s, "\"a\\\"b\\\\c\\n\\u0001\u{203a}\"");
    }

    #[test]
    fn pages_come_before_their_settings() {
        let e = entries();
        assert_eq!(e[0].link, "home");
        assert!(e[0].is_page);
        let displays = e.iter().position(|x| x.link == "displays").unwrap();
        assert_eq!(e[displays + 1].page, "displays");
        assert!(!e[displays + 1].is_page);
    }
}
