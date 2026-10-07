//! The Launcher's search index against the registry, and against the limits
//! the Launcher puts on the file (its settings_index.rs).

use serde_json::Value;
use settings_registry::index::{self, FALLBACK_ICON};
use settings_registry::launch::{self, Request, action_args, valid_link};
use settings_registry::{Kind, PAGES};
use std::collections::HashSet;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 5_000;
const MAX_TITLE_CHARS: usize = 128;
const MAX_KEYWORDS: usize = 64;
const MAX_KEYWORD_CHARS: usize = 64;

fn parsed() -> Value {
    serde_json::from_str(&index::to_json()).expect("the index is JSON")
}

fn entries(v: &Value) -> &Vec<Value> {
    v["entries"].as_array().expect("entries")
}

#[test]
fn the_file_has_the_launchers_shape() {
    let v = parsed();
    assert_eq!(v["version"], 1);
    assert_eq!(v["app"], "net.eterneon.telamon.settings");
    let json = index::to_json();
    assert!(json.len() < MAX_BYTES / 4, "{} bytes", json.len());
    let list = entries(&v);
    assert!(list.len() < MAX_ENTRIES / 4, "{} entries", list.len());
    let pages = PAGES.len();
    let settings: usize = PAGES.iter().map(|p| p.items.len()).sum();
    assert_eq!(list.len(), pages + settings);
    for e in list {
        for key in ["id", "kind", "page", "title", "icon", "keywords", "link"] {
            assert!(!e[key].is_null(), "{e}: no {key}");
        }
        assert_eq!(e["id"], e["link"]);
        assert!(matches!(e["kind"].as_str(), Some("page" | "setting")));
        assert_eq!(e["kind"] == "page", e["item"].is_null());
        // Text maps hold "C": the Launcher falls back to it.
        assert!(
            e["title"]["C"]
                .as_str()
                .is_some_and(|t| !t.trim().is_empty())
        );
        assert_eq!(e["parent"].is_null(), e["kind"] == "page");
    }
}

#[test]
fn every_page_and_setting_has_one_valid_unique_link() {
    let v = parsed();
    let mut seen = HashSet::new();
    for e in entries(&v) {
        let link = e["link"].as_str().unwrap();
        assert!(valid_link(link), "{link:?}");
        assert!(seen.insert(link.to_string()), "{link} twice");
    }
    for p in PAGES {
        assert!(seen.contains(p.id), "{} missing", p.id);
        for i in p.items {
            let link = format!("{}/{}", p.id, i.id);
            assert!(seen.contains(&link), "{link} missing");
        }
    }
    assert!(index::links_valid());
}

/// What the Launcher passes back to ActivateAction("open") is what we wrote:
/// every link lands on its page (and its setting).
#[test]
fn every_link_opens_its_page_and_setting() {
    for e in index::entries() {
        let (args, bad) = action_args("open", Some(&e.link));
        assert!(bad.is_empty(), "{}", e.link);
        let (requests, refused) = launch::parse(&args);
        assert!(refused.is_empty(), "{}: {refused:?}", e.link);
        assert_eq!(
            requests,
            vec![Request::Page {
                page: e.page,
                item: e.item
            }],
            "{}",
            e.link
        );
    }
}

#[test]
fn text_fits_the_launchers_caps() {
    let v = parsed();
    for e in entries(&v) {
        let title = e["title"]["C"].as_str().unwrap();
        assert!(title.chars().count() <= MAX_TITLE_CHARS, "{title}");
        if let Some(p) = e["parent"]["C"].as_str() {
            assert!(p.chars().count() <= MAX_TITLE_CHARS, "{p}");
        }
        let kws = e["keywords"]["C"].as_array().unwrap();
        assert!(kws.len() <= MAX_KEYWORDS, "{title}: {} keywords", kws.len());
        for k in kws {
            let k = k.as_str().unwrap();
            assert!(!k.trim().is_empty());
            assert!(k.chars().count() <= MAX_KEYWORD_CHARS, "{k}");
        }
        // No control or bidi characters: the Launcher would strip them.
        let all = format!("{title}{}", e["parent"]["C"].as_str().unwrap_or(""));
        assert!(
            !all.chars()
                .any(|c| c.is_control() || ('\u{202a}'..='\u{202e}').contains(&c))
        );
    }
}

#[test]
fn icons_are_theme_names_and_every_page_has_its_own() {
    for p in PAGES {
        let icon = index::icon(p.id);
        assert_ne!(icon, FALLBACK_ICON, "{} has no icon in index::icon", p.id);
    }
    let v = parsed();
    for e in entries(&v) {
        let icon = e["icon"].as_str().unwrap();
        // The Launcher takes theme names, never paths.
        assert!(
            !icon.is_empty()
                && icon.len() <= 64
                && icon
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.'),
            "{icon:?}"
        );
    }
}

#[test]
fn a_setting_says_where_it_lives() {
    let v = parsed();
    let find = |link: &str| {
        entries(&v)
            .iter()
            .find(|e| e["link"] == link)
            .unwrap_or_else(|| panic!("{link}"))
    };
    let night = find("displays/night-light");
    assert_eq!(night["parent"]["C"], "Displays");
    assert_eq!(night["title"]["C"], "Night Light");
    // Folded settings say so, as the in-app search does.
    let folded = PAGES
        .iter()
        .flat_map(|p| p.items.iter().map(move |i| (p, i)))
        .find(|(_, i)| i.advanced)
        .expect("some setting is folded");
    let e = find(&format!("{}/{}", folded.0.id, folded.1.id));
    assert_eq!(
        e["parent"]["C"],
        format!("{} \u{203a} Advanced", folded.0.title)
    );
    // Other Plasma Settings is searchable though not in the sidebar.
    assert!(PAGES.iter().any(|p| p.kind == Kind::MoreSettings));
    assert_eq!(find("more")["kind"], "page");
}
