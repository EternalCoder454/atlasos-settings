//! Invariants of the page table.

use settings_registry::pages::{self, RENAMED};
use settings_registry::{Kind, PAGES};
use std::collections::HashSet;

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
}

#[test]
fn ids_are_unique_and_plain() {
    let mut seen = HashSet::new();
    for p in PAGES {
        assert!(is_id(p.id), "page id {:?}", p.id);
        assert!(seen.insert(p.id), "page {} twice", p.id);
        let mut items = HashSet::new();
        for i in p.items {
            assert!(is_id(i.id), "{}/{:?}", p.id, i.id);
            assert!(items.insert(i.id), "{}/{} twice", p.id, i.id);
            assert!(!i.title.is_empty());
            assert!(
                i.keywords.iter().all(|k| k.to_lowercase() == *k),
                "{}/{}",
                p.id,
                i.id
            );
        }
        assert!(!p.title.is_empty() && !p.symbol.is_empty());
        assert!(
            p.keywords.iter().all(|k| k.to_lowercase() == *k),
            "{}",
            p.id
        );
    }
}

/// The sidebar: few pages, in this order (docs/DESIGN.md, "Pages").
#[test]
fn the_sidebar_is_short() {
    let sidebar: Vec<_> = PAGES
        .iter()
        .filter(|p| p.kind != Kind::MoreSettings)
        .map(|p| p.id)
        .collect();
    assert_eq!(
        sidebar,
        [
            "home",
            "network",
            "devices",
            "displays",
            "sound",
            "input",
            "appearance",
            "notifications",
            "apps",
            "privacy",
            "users",
            "power",
            "accessibility",
            "time-language",
            "system",
            "updates",
        ]
    );
    assert!(pages::page("more").is_some());
    // Every page looks different in the sidebar.
    let mut symbols = HashSet::new();
    for p in PAGES {
        assert!(
            symbols.insert(p.symbol),
            "{}: symbol {} twice",
            p.id,
            p.symbol
        );
    }
}

/// A page shows at most six settings before its Advanced section.
#[test]
fn pages_show_few_settings_unfolded() {
    for p in PAGES {
        let shown = p.items.iter().filter(|i| !i.advanced).count();
        assert!(shown <= 6, "{} shows {shown} settings unfolded", p.id);
        // Advanced comes last, so the fold is one section.
        let first_adv = p.items.iter().position(|i| i.advanced);
        if let Some(n) = first_adv {
            assert!(
                p.items[n..].iter().all(|i| i.advanced),
                "{}: an unfolded setting after Advanced",
                p.id
            );
        }
    }
}

#[test]
fn renamed_pages_land_on_real_pages() {
    for &(old, to, item) in RENAMED {
        assert!(pages::page(old).is_none(), "{old} is still a page");
        let p = pages::page(to).unwrap_or_else(|| panic!("{old}: no page {to}"));
        if let Some(i) = item {
            assert!(p.item(i).is_some(), "{old}: no {to}/{i}");
        }
        assert_eq!(pages::find(old, None).map(|(p, _)| p.id), Some(to));
    }
}

#[test]
fn symbols_exist_in_telamon_ui_2_0() {
    let names: HashSet<&str> = include_str!("fixtures/symbols-telamon-ui-2.0.0.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect();
    for p in PAGES {
        assert!(
            names.contains(p.symbol),
            "{}: no Symbols.{}",
            p.id,
            p.symbol
        );
    }
}

/// Related links go to other pages in the sidebar, each once.
#[test]
fn related_pages_are_real() {
    for p in PAGES {
        let mut seen = HashSet::new();
        for r in p.related {
            let to = pages::page(r).unwrap_or_else(|| panic!("{}: no page {r}", p.id));
            assert_ne!(to.id, p.id, "{} links to itself", p.id);
            assert_eq!(to.kind, Kind::Native, "{}: {r} isn't in the sidebar", p.id);
            assert!(seen.insert(*r), "{}: {r} twice", p.id);
        }
    }
}

/// Updates is a page of its own, not a row of System that opens another app.
#[test]
fn updates_is_a_page_of_its_own() {
    let (page, item) = pages::find("updates", Some("check")).expect("the Updates page");
    assert_eq!(page.id, "updates");
    assert_eq!(item.map(|i| i.id), Some("check"));
    // Neither a renamed ID nor a row of System any more.
    assert!(RENAMED.iter().all(|(old, _, _)| *old != "updates"));
    assert!(pages::page("system").is_some_and(|p| p.item("updates").is_none()));
    // The tray opens it with `telamon-settings updates check`.
    let (requests, refused) = settings_registry::launch::parse(&["updates".into(), "check".into()]);
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(
        requests,
        [settings_registry::launch::Request::Page {
            page: "updates",
            item: Some("check"),
        }]
    );
    // System's links lead to it.
    assert!(pages::page("system").is_some_and(|p| p.related.contains(&"updates")));
}
