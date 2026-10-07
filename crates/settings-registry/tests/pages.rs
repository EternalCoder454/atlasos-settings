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
fn symbols_exist_in_atlas_ui_1_4() {
    let names: HashSet<&str> = include_str!("fixtures/symbols-atlas-ui-1.4.0.txt")
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
