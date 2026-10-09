//! Guards of the QML that shows or opens what other programs sent
//! (docs/SECURITY.md, "Rich text" and "Opening links"). They read the QML
//! source, so a new `Text` or link that skips the rule fails here, not in a
//! review three releases later.
//!
//! - Text from outside (user and SSID names, wallpaper names, error text) is
//!   drawn as plain text: every `Text` and `QQC2.Label` says
//!   `textFormat: Text.PlainText`, nothing asks for rich, styled, Markdown or
//!   auto-detected text, and `TelamonLabel` (plain by default) is never
//!   switched away from it. `NotesText` (the release notes, which the
//!   updater's own sanitizer produced) is the one rich text, in one file.
//! - Every `Qt.openUrlExternally` is on a list: a new one needs a look at what
//!   the URL can be.
//! - QML that runs text as code or fetches from the network is not used.
//! - A password field is emptied (`<id>.text = ""`) in its file.

use std::fs;
use std::path::PathBuf;

fn qml_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/telamon-settings/qml")
}

/// (file name, contents) of every QML and JS file of the app.
fn sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(qml_dir()).expect("the qml folder") {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "qml" || ext == "js" {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    out.sort();
    assert!(out.len() > 20, "found only {} QML files", out.len());
    out
}

/// The text of every `Type {` block of `src` whose type is one of `types`
/// (a line that starts with the type), braces matched, strings and comments
/// skipped. With the line number it starts on.
fn blocks<'a>(src: &'a str, types: &[&str]) -> Vec<(usize, &'a str)> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut offset = 0;
    for (n, line) in src.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_start();
        let is_start = types.iter().any(|t| {
            trimmed
                .strip_prefix(t)
                .is_some_and(|rest| rest.trim_start().starts_with('{'))
        });
        if is_start {
            let open = offset + line.find('{').unwrap();
            let mut depth = 0i32;
            let mut i = open;
            let mut end = bytes.len();
            while i < bytes.len() {
                match bytes[i] {
                    b'"' | b'\'' => {
                        let quote = bytes[i];
                        i += 1;
                        while i < bytes.len() && bytes[i] != quote {
                            if bytes[i] == b'\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                    }
                    b'/' if bytes.get(i + 1) == Some(&b'/') => {
                        while i < bytes.len() && bytes[i] != b'\n' {
                            i += 1;
                        }
                        continue;
                    }
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            out.push((n + 1, &src[offset..end.min(src.len())]));
        }
        offset += line.len();
    }
    out
}

/// The block's own `textFormat:` lines (not those of items inside it are
/// told apart: a block is checked as a whole, which is stricter).
fn text_formats(block: &str) -> Vec<&str> {
    block
        .lines()
        .filter_map(|l| l.trim().strip_prefix("textFormat:"))
        .map(|v| v.trim().trim_end_matches(';').trim())
        .collect()
}

#[test]
fn drawn_text_is_plain() {
    for (file, src) in sources() {
        for (line, block) in blocks(
            &src,
            &[
                "Text",
                "Label",
                "QQC2.Label",
                "Controls.Label",
                "QQC2.TextArea",
                "TextEdit",
            ],
        ) {
            let formats = text_formats(block);
            assert!(
                formats.iter().any(|f| f.ends_with("PlainText")),
                "{file}:{line}: a Text or Label without `textFormat: Text.PlainText` shows \
                 <b>, <a href> and <img src> of the text it is given as markup"
            );
        }
    }
}

#[test]
fn nothing_asks_for_markup() {
    for (file, src) in sources() {
        for (n, line) in src.lines().enumerate() {
            let l = line.trim();
            if l.starts_with("//") {
                continue;
            }
            for bad in [
                "RichText",
                "StyledText",
                "MarkdownText",
                "AutoText",
                "Text.Markdown",
            ] {
                assert!(
                    !l.contains(bad),
                    "{file}:{}: {bad} draws text as markup; user and remote text is plain",
                    n + 1
                );
            }
        }
    }
}

#[test]
fn only_the_release_notes_are_rich_text() {
    let mut users = Vec::new();
    for (file, src) in sources() {
        let count = blocks(&src, &["NotesText"]).len();
        if count > 0 {
            users.push((file, count));
        }
    }
    // UpdatesPage shows the update's notes and the history's: html built by
    // telamon-updater-core's sanitizer (notes.rs), links opened only when
    // `isSafeLink` (https) says so.
    assert_eq!(users, [("UpdatesPage.qml".to_string(), 2)]);
}

#[test]
fn telamon_label_stays_plain() {
    for (file, src) in sources() {
        for (line, block) in blocks(&src, &["TelamonLabel"]) {
            for f in text_formats(block) {
                assert!(
                    f.ends_with("PlainText"),
                    "{file}:{line}: TelamonLabel is plain unless told otherwise; this one is {f}"
                );
            }
        }
    }
}

#[test]
fn links_are_opened_in_known_places_only() {
    // (file, number of `Qt.openUrlExternally` calls). Each is behind a check
    // (`isSafeLink`, `supportUrlOk`) on a URL that Rust built or validated.
    let allowed = [("CrashReportsSheet.qml", 2), ("UpdatesPage.qml", 3)];
    let mut found = Vec::new();
    for (file, src) in sources() {
        let count = src.matches("Qt.openUrlExternally").count();
        if count > 0 {
            found.push((file, count));
        }
    }
    let want: Vec<(String, usize)> = allowed.iter().map(|(f, n)| (f.to_string(), *n)).collect();
    assert_eq!(
        found, want,
        "a new Qt.openUrlExternally needs its URL checked in Rust and a line in docs/SECURITY.md"
    );
    // The crash sheet shows a button only when `isSafeLink` agrees.
    let (_, sheet) = sources()
        .into_iter()
        .find(|(f, _)| f == "CrashReportsSheet.qml")
        .unwrap();
    assert_eq!(sheet.matches("isSafeLink").count(), 2);
    let (_, updates) = sources()
        .into_iter()
        .find(|(f, _)| f == "UpdatesPage.qml")
        .unwrap();
    assert!(updates.contains("supportUrlOk"));
    assert!(updates.matches("isSafeLink(link)").count() >= 2);
}

#[test]
fn no_qml_runs_text_as_code_or_fetches() {
    for (file, src) in sources() {
        for (n, line) in src.lines().enumerate() {
            let l = line.trim();
            if l.starts_with("//") {
                continue;
            }
            for bad in [
                "createQmlObject",
                "Qt.include",
                "eval(",
                "new Function",
                "XMLHttpRequest",
                "fetch(",
                "WebView",
                "WebEngine",
                "Qt.createComponent",
                "Qt.callLater(eval",
            ] {
                assert!(
                    !l.contains(bad),
                    "{file}:{}: {bad}: QML here neither runs text as code nor fetches",
                    n + 1
                );
            }
        }
    }
}

#[test]
fn password_fields_are_emptied() {
    for (file, src) in sources() {
        let fields = blocks(&src, &["TelamonPasswordField"]);
        for (line, block) in fields {
            let id = block
                .lines()
                .find_map(|l| l.trim().strip_prefix("id:"))
                .map(|v| v.trim().trim_end_matches(';').to_string())
                .unwrap_or_else(|| panic!("{file}:{line}: a password field needs an id"));
            let cleared = format!("{id}.text = \"\"");
            assert!(
                src.contains(&cleared),
                "{file}:{line}: `{cleared}` not found: the password must not stay in the field \
                 after the sheet closes"
            );
        }
    }
}
