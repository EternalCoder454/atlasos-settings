//! The command line, for the first launch and for each later one the running
//! window is handed:
//!
//! ```text
//! telamon-settings                         the first page
//! telamon-settings <page> [<setting>]      a page, scrolled to one setting
//! telamon-settings --page <page>           the same
//! telamon-settings --kcm <name> [--args <text>]
//!                                        what that KCM name opens (kcm::resolve)
//! telamon-settings --search <text>         search
//! ```
//!
//! and the `org.freedesktop.Application.ActivateAction` calls other
//! programs make (the Launcher's `open` and `open-app`, [`action_args`]).
//!
//! Everything here is untrusted (any program can start Settings with any
//! arguments): lengths are capped, control characters refused, and page and
//! setting IDs must be ones the registry has.

use crate::{kcm, pages};

/// Arguments looked at; the rest are refused.
pub const MAX_ARGS: usize = 16;
/// Longest argument, in bytes.
pub const MAX_ARG: usize = 256;
/// Longest `--args` text, in bytes (KCMLauncher passes a device or
/// connection ID).
pub const MAX_KCM_ARGS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Home,
    Page {
        page: &'static str,
        item: Option<&'static str>,
    },
    /// A KCM Settings has no page for, to open in kcmshell6. `name` is
    /// [normalized](kcm::normalize).
    Kcm {
        name: String,
        args: Option<String>,
    },
    Search(String),
}

/// An argument that was not used, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The argument, shortened and escaped so it is safe to log or show.
    pub arg: String,
    pub reason: &'static str,
}

fn shown(arg: &str) -> String {
    let short: String = arg.chars().take(64).collect();
    let mut s = format!("{short:?}");
    if short.len() < arg.len() {
        s.push('…');
    }
    s
}

fn refuse(out: &mut Vec<Refused>, arg: &str, reason: &'static str) {
    out.push(Refused {
        arg: shown(arg),
        reason,
    });
}

/// Whether `arg` is acceptable text: at most `max` bytes and no control
/// characters. The reason when it is not.
pub fn clean(arg: &str, max: usize) -> Result<(), &'static str> {
    if arg.len() > max {
        Err("too long")
    } else if arg.chars().any(char::is_control) {
        Err("has control characters")
    } else {
        Ok(())
    }
}

/// What the arguments (without the program name) ask for, in order, and
/// what was refused. Nothing usable asks for the first page.
pub fn parse(args: &[String]) -> (Vec<Request>, Vec<Refused>) {
    let mut out = Vec::new();
    let mut refused = Vec::new();
    if args.len() > MAX_ARGS {
        // Only the first few: the rest could be any size.
        let shown: Vec<&str> = args[MAX_ARGS..]
            .iter()
            .take(4)
            .map(String::as_str)
            .collect();
        refuse(&mut refused, &shown.join(" "), "too many arguments");
    }
    let mut it = args.iter().take(MAX_ARGS).map(String::as_str).peekable();
    let mut positional: Vec<&str> = Vec::new();

    // `--name value` or `--name=value`.
    fn value<'a>(
        arg: &'a str,
        name: &str,
        it: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
    ) -> Option<Option<&'a str>> {
        if arg == name {
            Some(it.next())
        } else {
            arg.strip_prefix(name)
                .and_then(|r| r.strip_prefix('='))
                .map(Some)
        }
    }

    while let Some(arg) = it.next() {
        if let Err(why) = clean(arg, MAX_ARG.max(MAX_KCM_ARGS + 7)) {
            refuse(&mut refused, arg, why);
            continue;
        }
        if let Some(v) = value(arg, "--kcm", &mut it) {
            let Some(raw) = v else {
                refuse(&mut refused, arg, "needs a KCM name");
                continue;
            };
            if let Err(why) = clean(raw, MAX_ARG) {
                refuse(&mut refused, raw, why);
                continue;
            }
            let mut kcm_args = None;
            if let Some(next) = it.peek().copied()
                && let Some(a) = value(next, "--args", &mut std::iter::empty().peekable())
            {
                it.next();
                kcm_args = match a {
                    Some(a) => Some(a),
                    None => it.next(),
                };
                if kcm_args.is_none() {
                    refuse(&mut refused, next, "needs text");
                }
            }
            if let Some(a) = kcm_args
                && let Err(why) = clean(a, MAX_KCM_ARGS)
            {
                refuse(&mut refused, a, why);
                kcm_args = None;
            }
            match kcm::normalize(raw) {
                None => refuse(&mut refused, raw, "not a KCM name"),
                Some(name) => out.push(match kcm::resolve(&name) {
                    kcm::Target::Home => Request::Home,
                    kcm::Target::Page { page, item } => Request::Page { page, item },
                    kcm::Target::Kcm => Request::Kcm {
                        name,
                        args: kcm_args.map(str::to_string),
                    },
                }),
            }
            continue;
        }
        if let Some(v) = value(arg, "--search", &mut it) {
            match v.map(|t| (t, clean(t, MAX_ARG))) {
                Some((t, Ok(()))) if !t.trim().is_empty() => {
                    out.push(Request::Search(t.trim().to_string()))
                }
                Some((t, Err(why))) => refuse(&mut refused, t, why),
                _ => refuse(&mut refused, arg, "needs text to search for"),
            }
            continue;
        }
        if let Some(v) = value(arg, "--page", &mut it) {
            match v.map(|p| (p, clean(p, MAX_ARG))) {
                Some((p, Ok(()))) => positional.push(p),
                Some((p, Err(why))) => refuse(&mut refused, p, why),
                None => refuse(&mut refused, arg, "needs a page"),
            }
            continue;
        }
        // An empty argument (a launcher's unused %u, say) asks for nothing.
        if arg.trim().is_empty() {
            continue;
        }
        if arg.starts_with('-') && arg != "-" {
            refuse(&mut refused, arg, "unknown option");
            continue;
        }
        if arg.len() > MAX_ARG {
            refuse(&mut refused, arg, "too long");
            continue;
        }
        positional.push(arg);
    }

    let mut pos = positional.into_iter();
    if let Some(p) = pos.next() {
        // Page IDs of earlier versions still work (pages::RENAMED).
        let wanted = pos.next();
        match pages::find(p, wanted) {
            None => refuse(&mut refused, p, "no such page"),
            Some((page, item)) => {
                if let Some(i) = wanted
                    && page.item(i).is_none()
                {
                    refuse(&mut refused, i, "no such setting on that page");
                }
                out.push(Request::Page {
                    page: page.id,
                    item: item.map(|i| i.id),
                });
            }
        }
    }
    for extra in pos {
        refuse(&mut refused, extra, "one page at a time");
    }
    if out.is_empty() {
        out.push(Request::Home);
    }
    (out, refused)
}

/// Longest link in the search index, in bytes (the Launcher's cap too).
pub const MAX_LINK: usize = 128;
/// Longest desktop file ID `open-app` accepts, in bytes.
pub const MAX_DESKTOP_ID: usize = 200;

/// Whether `link` is one the search index can hold: `page` or `page/item`
/// style, `[a-z0-9][a-z0-9-]*` segments joined by `/`, at most
/// [`MAX_LINK`] bytes.
pub fn valid_link(link: &str) -> bool {
    !link.is_empty()
        && link.len() <= MAX_LINK
        && link.split('/').all(|seg| {
            let b = seg.as_bytes();
            b.first()
                .is_some_and(|f| f.is_ascii_lowercase() || f.is_ascii_digit())
                && b.iter()
                    .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}

/// Whether `id` can be a desktop file ID (`org.example.App.desktop`): 1 to
/// [`MAX_DESKTOP_ID`] of `A-Z a-z 0-9 . _ - +`, not starting with `-` or `.`.
pub fn valid_desktop_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_DESKTOP_ID
        && !id.starts_with(['-', '.'])
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
}

/// The launch arguments (without the program name) that an
/// `org.freedesktop.Application.ActivateAction(action, [parameter])` call
/// stands for, and what was refused.
///
/// - `open` with a link from the search index (`displays`,
///   `displays/night-light`): that page, scrolled to that setting.
/// - `open-app` with an app's desktop file ID: the Apps page, at App
///   Permissions (the ID only has to be well formed; permissions are listed
///   for Flatpak apps and picked there).
/// - Any other action asks for nothing more than the window: no arguments
///   and nothing refused (what a link or ID is not is reported for the two
///   above).
pub fn action_args(action: &str, parameter: Option<&str>) -> (Vec<String>, Vec<Refused>) {
    let mut refused = Vec::new();
    let args = match (action, parameter) {
        ("open", Some(link)) if valid_link(link) => link.split('/').map(str::to_string).collect(),
        ("open", Some(link)) => {
            refuse(&mut refused, link, "not a link");
            Vec::new()
        }
        ("open-app", Some(id)) if valid_desktop_id(id) => {
            vec!["apps".to_string(), "permissions".to_string()]
        }
        ("open-app", Some(id)) => {
            refuse(&mut refused, id, "not a desktop file ID");
            Vec::new()
        }
        ("open" | "open-app", None) => {
            refuse(&mut refused, action, "needs a parameter");
            Vec::new()
        }
        // Another program's idea of an action: the window only, with
        // nothing to tell the user about.
        _ => Vec::new(),
    };
    (args, refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> (Vec<Request>, Vec<Refused>) {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn page(page: &'static str, item: Option<&'static str>) -> Request {
        Request::Page { page, item }
    }

    #[test]
    fn pages_and_items() {
        assert_eq!(p(&[]), (vec![Request::Home], vec![]));
        assert_eq!(p(&["sound"]).0, vec![page("sound", None)]);
        assert_eq!(
            p(&["displays", "night-light"]).0,
            vec![page("displays", Some("night-light"))]
        );
        assert_eq!(p(&["--page", "system"]).0, vec![page("system", None)]);
        assert_eq!(p(&["--page=system"]).0, vec![page("system", None)]);
        // Earlier page IDs land where their page went.
        assert_eq!(p(&["about"]).0, vec![page("system", None)]);
        assert_eq!(p(&["mouse"]).0, vec![page("input", Some("speed"))]);
        assert_eq!(p(&["mouse", "tap"]).0, vec![page("input", Some("tap"))]);
        assert_eq!(p(&["printers"]).0, vec![page("devices", Some("printers"))]);
        let (r, bad) = p(&["displays", "nope"]);
        assert_eq!(r, vec![page("displays", None)]);
        assert_eq!(bad.len(), 1);
        let (r, bad) = p(&["nope"]);
        assert_eq!(r, vec![Request::Home]);
        assert_eq!(bad[0].reason, "no such page");
        let (r, bad) = p(&["sound", "volume", "extra"]);
        assert_eq!(r, vec![page("sound", Some("volume"))]);
        assert_eq!(bad[0].reason, "one page at a time");
    }

    #[test]
    fn empty_arguments_ask_for_nothing() {
        assert_eq!(p(&[""]), (vec![Request::Home], vec![]));
        assert_eq!(
            p(&["", " ", "devices"]),
            (vec![page("devices", None)], vec![])
        );
    }

    #[test]
    fn kcm_names() {
        assert_eq!(p(&["--kcm", "kcm_kscreen"]).0, vec![page("displays", None)]);
        assert_eq!(
            p(&["--kcm=kcm_nightlight"]).0,
            vec![page("displays", Some("night-light"))]
        );
        assert_eq!(p(&["--kcm", "kcm_landingpage"]).0, vec![Request::Home]);
        assert_eq!(
            p(&["--kcm", "kcm_fonts", "--args", "x=1"]).0,
            vec![Request::Kcm {
                name: "kcm_fonts".into(),
                args: Some("x=1".into())
            }]
        );
        assert_eq!(
            p(&["--kcm", "kcm_fonts", "--args=x"]).0,
            vec![Request::Kcm {
                name: "kcm_fonts".into(),
                args: Some("x".into())
            }]
        );
        let (r, bad) = p(&["--kcm", "../../bin/sh"]);
        assert_eq!(r, vec![Request::Home]);
        assert_eq!(bad[0].reason, "not a KCM name");
        let (_, bad) = p(&["--kcm"]);
        assert_eq!(bad[0].reason, "needs a KCM name");
        let (r, bad) = p(&["--kcm", "kcm_fonts", "--args"]);
        assert_eq!(
            r,
            vec![Request::Kcm {
                name: "kcm_fonts".into(),
                args: None
            }]
        );
        assert_eq!(bad[0].reason, "needs text");
        let long = "a".repeat(MAX_KCM_ARGS + 1);
        let (r, bad) = p(&["--kcm", "kcm_fonts", "--args", &long]);
        assert_eq!(
            r,
            vec![Request::Kcm {
                name: "kcm_fonts".into(),
                args: None
            }]
        );
        assert_eq!(bad.len(), 1);
    }

    #[test]
    fn search() {
        assert_eq!(
            p(&["--search", " wifi "]).0,
            vec![Request::Search("wifi".into())]
        );
        assert_eq!(p(&["--search", ""]).1[0].reason, "needs text to search for");
    }

    #[test]
    fn hostile_input() {
        let (r, bad) = p(&["--evil"]);
        assert_eq!(r, vec![Request::Home]);
        assert_eq!(bad[0].reason, "unknown option");
        let (_, bad) = p(&["sound\u{1b}[2J"]);
        assert_eq!(bad[0].reason, "has control characters");
        let (_, bad) = p(&[&"x".repeat(MAX_ARG + 1)]);
        assert_eq!(bad[0].reason, "too long");
        assert!(bad[0].arg.chars().count() < 80);
        let many: Vec<&str> = std::iter::repeat_n("sound", MAX_ARGS + 5).collect();
        let (r, bad) = p(&many);
        assert_eq!(r, vec![page("sound", None)]);
        assert!(bad.iter().any(|b| b.reason == "too many arguments"));
        let (_, bad) = p(&[&"y".repeat(10 * MAX_KCM_ARGS)]);
        assert_eq!(bad[0].reason, "too long");
        // Option values are checked like any argument.
        let (_, bad) = p(&["--page", "sound\u{7}"]);
        assert_eq!(bad[0].reason, "has control characters");
        let (_, bad) = p(&["--kcm", &"k".repeat(MAX_ARG + 1)]);
        assert_eq!(bad[0].reason, "too long");
        // A flood of arguments is reported, not copied whole.
        let huge = "z".repeat(MAX_ARG);
        let flood: Vec<&str> = std::iter::repeat_n(huge.as_str(), 10_000).collect();
        let (_, bad) = p(&flood);
        assert!(bad[0].arg.chars().count() < 80);
    }

    fn act(action: &str, parameter: Option<&str>) -> (Vec<String>, Vec<Refused>) {
        action_args(action, parameter)
    }

    #[test]
    fn open_links() {
        assert_eq!(
            act("open", Some("displays")),
            (vec!["displays".into()], vec![])
        );
        let (args, bad) = act("open", Some("displays/night-light"));
        assert_eq!(args, ["displays", "night-light"]);
        assert!(bad.is_empty());
        // What the arguments then mean:
        assert_eq!(
            parse(&args).0,
            vec![Request::Page {
                page: "displays",
                item: Some("night-light")
            }]
        );
        // Not links: the window only, and a reason.
        for bad_link in [
            "",
            "Displays",
            "a//b",
            "/a",
            "a/",
            "-a",
            "a/-b",
            "a b",
            "../x",
            "a_b",
            "--kcm",
            "x\u{1b}[2J",
        ] {
            let (args, bad) = act("open", Some(bad_link));
            assert!(args.is_empty(), "{bad_link:?}");
            assert_eq!(bad[0].reason, "not a link", "{bad_link:?}");
        }
        let long = "a".repeat(MAX_LINK + 1);
        assert_eq!(act("open", Some(&long)).1[0].reason, "not a link");
        assert!(valid_link(&"a".repeat(MAX_LINK)));
        assert_eq!(act("open", None).1[0].reason, "needs a parameter");
        // A link for a page or setting that does not exist still opens the
        // window, at the first page (parse reports the rest).
        let (args, _) = act("open", Some("nope/nothing"));
        assert_eq!(parse(&args).0, vec![Request::Home]);
    }

    #[test]
    fn open_app() {
        for id in [
            "org.kde.kate.desktop",
            "com.brave.Browser.desktop",
            "a-b_c+d.desktop",
        ] {
            let (args, bad) = act("open-app", Some(id));
            assert!(bad.is_empty(), "{id}");
            assert_eq!(
                parse(&args).0,
                vec![Request::Page {
                    page: "apps",
                    item: Some("permissions")
                }]
            );
        }
        for id in [
            "",
            "-x.desktop",
            ".hidden",
            "a b",
            "a/b.desktop",
            "a;b",
            "é.desktop",
            "a\nb",
        ] {
            let (args, bad) = act("open-app", Some(id));
            assert!(args.is_empty(), "{id:?}");
            assert_eq!(bad[0].reason, "not a desktop file ID", "{id:?}");
        }
        assert_eq!(
            act("open-app", Some(&"x".repeat(MAX_DESKTOP_ID + 1))).1[0].reason,
            "not a desktop file ID"
        );
        assert_eq!(act("open-app", None).1[0].reason, "needs a parameter");
    }

    #[test]
    fn other_actions_only_raise() {
        for action in ["new-window", "", "Open", &"z".repeat(5000)] {
            for parameter in [None, Some("displays"), Some("\u{1b}")] {
                assert_eq!(act(action, parameter), (vec![], vec![]), "{action:.20}");
            }
        }
    }
}
