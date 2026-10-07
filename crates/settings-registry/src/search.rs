//! Search over the pages and the settings on them.
//!
//! Every word typed must match: the start of a word in the title scores
//! highest, then anywhere in the title, then a keyword, then (for a setting)
//! its page's title. Pages come before settings that score the same, and
//! otherwise the sidebar order holds, so results don't jump between
//! keystrokes.

use crate::pages::{Item, PAGES, Page};
use std::sync::OnceLock;

/// The longest query looked at, in characters; the rest is ignored.
pub const MAX_QUERY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub page: &'static Page,
    /// `None` for the page itself.
    pub item: Option<&'static Item>,
    pub score: u32,
}

struct Entry {
    page: &'static Page,
    item: Option<&'static Item>,
    order: usize,
    title_words: Vec<String>,
    title_compact: String,
    keyword_words: Vec<String>,
    keywords_compact: Vec<String>,
    page_words: Vec<String>,
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn compact(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn index() -> &'static [Entry] {
    static INDEX: OnceLock<Vec<Entry>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut out = Vec::new();
        for page in PAGES {
            let mut add = |item: Option<&'static Item>, title: &str, keywords: &[&str]| {
                out.push(Entry {
                    page,
                    item,
                    order: out.len(),
                    title_words: words(title),
                    title_compact: compact(title),
                    keyword_words: keywords.iter().flat_map(|k| words(k)).collect(),
                    keywords_compact: keywords.iter().map(|k| compact(k)).collect(),
                    page_words: if item.is_some() {
                        words(page.title)
                    } else {
                        Vec::new()
                    },
                });
            };
            add(None, page.title, page.keywords);
            for item in page.items {
                add(Some(item), item.title, item.keywords);
            }
        }
        out
    })
}

fn term_score(e: &Entry, term: &str) -> Option<u32> {
    if let Some(i) = e.title_words.iter().position(|w| w.starts_with(term)) {
        return Some(if i == 0 { 6 } else { 5 });
    }
    if e.title_compact.contains(term) {
        return Some(4);
    }
    if e.keyword_words.iter().any(|w| w.starts_with(term)) {
        return Some(3);
    }
    if e.keywords_compact.iter().any(|k| k.contains(term)) {
        return Some(2);
    }
    if e.page_words.iter().any(|w| w.starts_with(term)) {
        return Some(1);
    }
    None
}

/// The pages and settings matching `query`, best first, at most `limit`.
/// An empty query finds nothing.
pub fn search(query: &str, limit: usize) -> Vec<Hit> {
    let query: String = query.chars().take(MAX_QUERY).collect();
    let terms: Vec<String> = query
        .split_whitespace()
        .map(compact)
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut hits: Vec<(u32, usize, Hit)> = index()
        .iter()
        .filter_map(|e| {
            let mut score = 0;
            for t in &terms {
                score += term_score(e, t)?;
            }
            // A page outranks its own settings on the same score.
            let rank = score * 2 + u32::from(e.item.is_none());
            Some((
                rank,
                e.order,
                Hit {
                    page: e.page,
                    item: e.item,
                    score,
                },
            ))
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.truncate(limit);
    hits.into_iter().map(|(_, _, h)| h).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(q: &str) -> (&'static str, Option<&'static str>) {
        let h = search(q, 10);
        let h = h.first().unwrap_or_else(|| panic!("nothing for {q:?}"));
        (h.page.id, h.item.map(|i| i.id))
    }

    #[test]
    fn finds_pages_and_settings() {
        assert_eq!(first("blue"), ("devices", None));
        assert_eq!(first("wifi"), ("network", Some("wifi")));
        assert_eq!(first("wi-fi"), ("network", Some("wifi")));
        assert_eq!(first("night light"), ("displays", Some("night-light")));
        assert_eq!(first("dark"), ("appearance", Some("style")));
        assert_eq!(first("fingerprint"), ("users", Some("fingerprint")));
        assert_eq!(first("timezone"), ("time-language", Some("timezone")));
        assert_eq!(first("SHORTCUTS"), ("input", Some("shortcuts")));
        assert_eq!(first("printer"), ("devices", Some("printers")));
        // Folded settings are found too.
        assert_eq!(first("refresh rate"), ("displays", Some("refresh-rate")));
    }

    #[test]
    fn every_term_must_match() {
        assert!(search("bluetooth zebra", 10).is_empty());
        let h = search("sound volume", 10);
        assert!(h.iter().all(|h| h.page.id == "sound"), "{h:?}");
    }

    #[test]
    fn empty_and_silly_queries() {
        assert!(search("", 10).is_empty());
        assert!(search("   ", 10).is_empty());
        assert!(search("&&&", 10).is_empty());
        assert!(search("wifi", 0).is_empty());
        let long = "a".repeat(100_000);
        assert!(search(&long, 10).is_empty());
        assert!(search("\u{0}\u{202e}", 10).is_empty());
    }

    #[test]
    fn limit_and_order_are_stable() {
        let a = search("a", 5);
        assert_eq!(a.len(), 5);
        assert_eq!(a, search("a", 5));
        let all = search("a", usize::MAX);
        assert!(all.windows(2).all(|w| w[0].score >= w[1].score));
    }

    #[test]
    fn every_entry_finds_itself() {
        for p in PAGES {
            assert!(
                search(p.title, 50)
                    .iter()
                    .any(|h| h.page.id == p.id && h.item.is_none()),
                "{}",
                p.id
            );
            for i in p.items {
                assert!(
                    search(i.title, 50)
                        .iter()
                        .any(|h| h.page.id == p.id && h.item == Some(i)),
                    "{}/{}",
                    p.id,
                    i.id
                );
            }
        }
    }
}
