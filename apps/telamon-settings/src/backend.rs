//! The window's entry point: the page registry for QML, search, and what a
//! launch asks for. `main.cpp` calls `activate` with the first launch's
//! arguments and with each forwarded second launch's; QML listens to
//! `requested` and opens the page.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[namespace = "telamon_settings"]
        type Backend = super::BackendRust;

        /// Handles a launch's arguments (without the program name).
        #[qinvokable]
        fn activate(self: Pin<&mut Backend>, args: &QStringList);

        /// Handles an `org.freedesktop.Application.ActivateAction` call: the
        /// Launcher's `open` (a link of the search index) and `open-app`
        /// (a desktop file ID). `parameter` is "" when the call had none.
        #[qinvokable]
        #[cxx_name = "activateAction"]
        fn activate_action(self: Pin<&mut Backend>, action: &QString, parameter: &QString);

        /// The pages in sidebar order, as JSON: `[{id, title, symbol, kind,
        /// kcm, items: [{id, title, kcm, advanced}], related: [page id]}]`.
        /// `kind` is `native` or `more` (Other Plasma Settings, not in the sidebar); `kcm` the
        /// KCM a native page replaces, offered while the page isn't built.
        #[qinvokable]
        #[cxx_name = "pagesJson"]
        fn pages_json(self: &Backend) -> QString;

        /// The pages and settings matching `query`, best first, as JSON:
        /// `[{page, item, title, subtitle, symbol, kcm}]`.
        #[qinvokable]
        fn search(self: &Backend, query: &QString) -> QString;

        /// The command that opens KCM `name` in kcmshell6, or an empty list
        /// when `name` is not a KCM name or `args` is too long.
        #[qinvokable]
        #[cxx_name = "kcmCommand"]
        fn kcm_command(self: &Backend, name: &QString, args: &QString) -> QStringList;

        /// Whether KCM `name` has a page of Settings (More Settings leaves
        /// those out).
        #[qinvokable]
        #[cxx_name = "hasPage"]
        fn has_page(self: &Backend, name: &QString) -> bool;

        /// One request: `kind` is `home`, `page` (`first` the page,
        /// `second` the setting or ""), `kcm` (`first` the KCM name,
        /// `second` its `--args` or "") or `search` (`first` the text).
        #[qsignal]
        fn requested(self: Pin<&mut Backend>, kind: QString, first: QString, second: QString);

        /// An argument that was not used, already made safe to show.
        #[qsignal]
        fn refused(self: Pin<&mut Backend>, text: QString);
    }

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn backend_make_unique() -> UniquePtr<Backend>;
    }
}

use core::pin::Pin;
use cxx_qt_lib::{QString, QStringList};
use serde_json::{Value, json};
use settings_registry::launch::{self, Request};
use settings_registry::{Kind, PAGES, kcm, search};

/// Most search results shown.
const MAX_RESULTS: usize = 50;

#[derive(Default)]
pub struct BackendRust {}

impl qobject::Backend {
    pub fn activate(self: Pin<&mut Self>, args: &QStringList) {
        // parse() looks at MAX_ARGS and names a few past them; a flood
        // from another program isn't copied whole.
        let args: Vec<String> = args
            .iter()
            .take(launch::MAX_ARGS + 4)
            .map(|a| {
                // Longer than parse() accepts: refused there as too long
                // without copying megabytes first.
                if a.len() > launch::MAX_KCM_ARGS as isize + 8 {
                    "\u{FFFD}".repeat(launch::MAX_KCM_ARGS + 8)
                } else {
                    a.to_string()
                }
            })
            .collect();
        self.handle(args, Vec::new());
    }

    pub fn activate_action(self: Pin<&mut Self>, action: &QString, parameter: &QString) {
        // Another program's text: capped before anything reads it.
        let cap = |q: &QString| {
            if q.len() > launch::MAX_ARG as isize {
                "\u{FFFD}".repeat(launch::MAX_ARG + 1)
            } else {
                q.to_string()
            }
        };
        let action = cap(action);
        let parameter = cap(parameter);
        let (args, refused) =
            launch::action_args(&action, Some(parameter.as_str()).filter(|p| !p.is_empty()));
        self.handle(args, refused);
    }

    /// Reports what was refused, then asks for what `args` mean.
    fn handle(mut self: Pin<&mut Self>, args: Vec<String>, earlier: Vec<launch::Refused>) {
        let (requests, mut refused) = launch::parse(&args);
        refused.splice(0..0, earlier);
        for r in &refused {
            log::warn!("ignored argument {}: {}", r.arg, r.reason);
            self.as_mut()
                .refused(QString::from(format!("{} ({})", r.arg, r.reason).as_str()));
        }
        for request in requests {
            let (kind, first, second) = describe(&request);
            self.as_mut().requested(
                QString::from(kind),
                QString::from(first.as_str()),
                QString::from(second.as_str()),
            );
        }
    }

    pub fn pages_json(&self) -> QString {
        QString::from(pages_json().as_str())
    }

    pub fn search(&self, query: &QString) -> QString {
        QString::from(search_json(&query.to_string()).as_str())
    }

    pub fn kcm_command(&self, name: &QString, args: &QString) -> QStringList {
        let mut list = QStringList::default();
        for a in kcm_command(&name.to_string(), &args.to_string()) {
            list.append(QString::from(a.as_str()));
        }
        list
    }

    pub fn has_page(&self, name: &QString) -> bool {
        kcm::normalize(&name.to_string()).is_some_and(|n| kcm::has_page(&n))
    }
}

fn describe(request: &Request) -> (&'static str, String, String) {
    match request {
        Request::Home => ("home", String::new(), String::new()),
        Request::Page { page, item } => (
            "page",
            page.to_string(),
            item.unwrap_or_default().to_string(),
        ),
        Request::Kcm { name, args } => ("kcm", name.clone(), args.clone().unwrap_or_default()),
        Request::Search(text) => ("search", text.clone(), String::new()),
    }
}

/// The KCM a native page replaces: the first one mapped to the page as a
/// whole, else the first mapped to a setting on it (Appearance has no one
/// KCM), so every page that isn't built yet has a way to its settings.
fn replaced_kcm(page: &str) -> Option<&'static str> {
    let mapped = || kcm::MAP.iter().filter(move |(_, p, _)| *p == page);
    mapped()
        .find(|(_, _, item)| item.is_none())
        .or_else(|| mapped().next())
        .map(|(k, _, _)| *k)
}

fn pages_json() -> String {
    let pages: Vec<Value> = PAGES
        .iter()
        .map(|p| {
            let kind = match p.kind {
                Kind::Native => "native",
                Kind::MoreSettings => "more",
            };
            let items: Vec<Value> = p
                .items
                .iter()
                .map(|i| {
                    json!({
                        "id": i.id,
                        "title": i.title,
                        "kcm": i.kcm.unwrap_or_default(),
                        "advanced": i.advanced,
                    })
                })
                .collect();
            json!({
                "id": p.id,
                "title": p.title,
                "symbol": p.symbol,
                "kind": kind,
                "kcm": replaced_kcm(p.id).unwrap_or_default(),
                "items": items,
                "related": p.related,
            })
        })
        .collect();
    Value::Array(pages).to_string()
}

fn search_json(query: &str) -> String {
    let hits: Vec<Value> = search::search(query, MAX_RESULTS)
        .into_iter()
        .map(|h| match h.item {
            None => json!({
                "page": h.page.id,
                "item": "",
                "title": h.page.title,
                "subtitle": "",
                "symbol": h.page.symbol,
                "kcm": "",
            }),
            Some(i) => json!({
                "page": h.page.id,
                "item": i.id,
                "title": i.title,
                // Where it is, as macOS shows it: "Displays › Advanced".
                "subtitle": if i.advanced {
                    format!("{} › Advanced", h.page.title)
                } else {
                    h.page.title.to_owned()
                },
                "symbol": h.page.symbol,
                "kcm": i.kcm.unwrap_or_default(),
            }),
        })
        .collect();
    Value::Array(hits).to_string()
}

fn kcm_command(name: &str, args: &str) -> Vec<String> {
    let Some(name) = kcm::normalize(name) else {
        log::warn!("not a KCM name: {name:?}");
        return Vec::new();
    };
    if kcm::HOME_NAMES.contains(&name.as_str()) {
        return Vec::new();
    }
    if args.len() > launch::MAX_KCM_ARGS || args.chars().any(char::is_control) {
        log::warn!("KCM arguments refused for {name}");
        return Vec::new();
    }
    let mut cmd = vec!["kcmshell6".to_string(), name];
    if !args.is_empty() {
        // One argument, so text starting with `-` can't be read as an option.
        cmd.push(format!("--args={args}"));
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_json_is_complete() {
        let v: Value = serde_json::from_str(&pages_json()).unwrap();
        let pages = v.as_array().unwrap();
        assert_eq!(pages.len(), PAGES.len());
        let network = pages.iter().find(|p| p["id"] == "network").unwrap();
        assert_eq!(network["kind"], "native");
        assert_eq!(network["kcm"], "kcm_networkmanagement");
        let devices = pages.iter().find(|p| p["id"] == "devices").unwrap();
        let printers = &devices["items"].as_array().unwrap()[2];
        assert_eq!(printers["id"], "printers");
        assert_eq!(printers["kcm"], "kcm_printer_manager");
        assert_eq!(printers["advanced"], false);
        let more = pages.iter().find(|p| p["id"] == "more").unwrap();
        assert_eq!(more["kind"], "more");
        let appearance = pages.iter().find(|p| p["id"] == "appearance").unwrap();
        assert_eq!(appearance["kcm"], "kcm_lookandfeel");
        // No native page is a dead end while it is pending (Home has
        // nothing of Plasma's to stand in for).
        for p in pages
            .iter()
            .filter(|p| p["kind"] == "native" && p["id"] != "home")
        {
            assert_ne!(p["kcm"], "", "{} has no KCM to open", p["id"]);
        }
    }

    #[test]
    fn search_json_shapes() {
        let v: Value = serde_json::from_str(&search_json("fonts")).unwrap();
        let first = &v[0];
        assert_eq!(first["page"], "appearance");
        assert_eq!(first["kcm"], "kcm_fonts");
        assert_eq!(first["subtitle"], "Appearance › Advanced");
        assert_eq!(
            serde_json::from_str::<Value>(&search_json("")).unwrap(),
            json!([])
        );
    }

    #[test]
    fn kcm_commands() {
        assert_eq!(kcm_command("kcm_fonts", ""), ["kcmshell6", "kcm_fonts"]);
        assert_eq!(
            kcm_command("kcm_fonts.desktop", "a"),
            ["kcmshell6", "kcm_fonts", "--args=a"]
        );
        assert!(kcm_command("../../bin/sh", "").is_empty());
        assert!(kcm_command("kcm_fonts --help", "").is_empty());
        assert!(kcm_command("kcm_fonts", "a\nb").is_empty());
        assert!(kcm_command("kcm_landingpage", "").is_empty());
        assert_eq!(
            kcm_command("kcm_fonts", "--help"),
            ["kcmshell6", "kcm_fonts", "--args=--help"]
        );
    }
}
