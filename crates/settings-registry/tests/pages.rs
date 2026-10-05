//! Invariants of the page table.

use settings_registry::{Group, PAGES};
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

#[test]
fn groups_are_contiguous_and_in_order() {
    let order: Vec<Group> = PAGES.iter().map(|p| p.group).collect();
    let mut groups = order.clone();
    groups.dedup();
    assert_eq!(
        groups,
        Group::ALL.to_vec(),
        "every group once, in sidebar order"
    );
}

#[test]
fn the_planned_pages_are_there() {
    let ids: Vec<_> = PAGES.iter().map(|p| p.id).collect();
    for want in [
        "network",
        "bluetooth",
        "firewall",
        "displays",
        "sound",
        "printers",
        "keyboard",
        "mouse",
        "power",
        "appearance",
        "desktop",
        "notifications",
        "default-apps",
        "app-permissions",
        "users",
        "privacy",
        "accessibility",
        "datetime",
        "region",
        "updates",
        "about",
        "more",
    ] {
        assert!(ids.contains(&want), "{want}");
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
