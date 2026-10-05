//! Errors from the system services, sorted into what the page can say about
//! them.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The service isn't installed or isn't running.
    NotRunning,
    /// polkit said no, or the person cancelled the password prompt.
    Denied,
    /// No answer in time.
    Timeout,
    /// The service refused the request (bad value, unsupported).
    Refused,
    /// The bus itself failed (no bus, a broken connection).
    Bus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// What the service or bus said: for the log, not for the page.
    pub detail: String,
}

/// The longest detail kept, in characters.
pub const MAX_DETAIL: usize = 200;

/// Format characters (Unicode category Cf and line/paragraph separators) that
/// can reorder or hide text: bidi overrides, zero-width marks, tags.
fn is_invisible(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}'
        | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}'
        | '\u{FFF9}'..='\u{FFFB}' | '\u{E0000}'..='\u{E007F}')
}

/// D-Bus text is untrusted: cap it and replace control and invisible format
/// characters with U+FFFD before it reaches a log or a page.
fn sanitize(s: &str) -> String {
    let mut out = String::new();
    for (n, c) in s.chars().enumerate() {
        if n == MAX_DETAIL {
            out.push('\u{2026}');
            break;
        }
        out.push(if c.is_control() || is_invisible(c) {
            '\u{FFFD}'
        } else {
            c
        });
    }
    out
}

impl Error {
    pub fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Error {
            kind,
            detail: sanitize(&detail.into()),
        }
    }

    /// One sentence for the page, in plain words.
    pub fn describe(&self) -> &'static str {
        match self.kind {
            ErrorKind::NotRunning => "The system service for this isn't running.",
            ErrorKind::Denied => {
                "You aren't allowed to change this, or the password prompt was cancelled."
            }
            ErrorKind::Timeout => "The system service didn't answer in time.",
            ErrorKind::Refused => "The system service didn't accept the change.",
            ErrorKind::Bus => "Settings couldn't reach the system services.",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.describe(), self.detail)
    }
}

impl std::error::Error for Error {}

fn kind_of_dbus_error(name: &str) -> ErrorKind {
    match name {
        "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner"
        | "org.freedesktop.DBus.Error.UnknownObject"
        | "org.freedesktop.DBus.Error.UnknownInterface"
        | "org.freedesktop.DBus.Error.Spawn.ServiceNotFound"
        | "org.freedesktop.systemd1.NoSuchUnit" => ErrorKind::NotRunning,
        "org.freedesktop.DBus.Error.AccessDenied"
        | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
        | "org.freedesktop.PolicyKit1.Error.NotAuthorized"
        | "org.freedesktop.PolicyKit1.Error.Cancelled" => ErrorKind::Denied,
        "org.freedesktop.DBus.Error.NoReply"
        | "org.freedesktop.DBus.Error.Timeout"
        | "org.freedesktop.DBus.Error.TimedOut" => ErrorKind::Timeout,
        _ => ErrorKind::Refused,
    }
}

impl From<zbus::Error> for Error {
    fn from(e: zbus::Error) -> Self {
        let kind = match &e {
            zbus::Error::MethodError(name, _, _) => kind_of_dbus_error(name.as_str()),
            zbus::Error::FDO(fdo) => return Error::from(fdo.as_ref().clone()),
            zbus::Error::InputOutput(io) if io.kind() == std::io::ErrorKind::TimedOut => {
                ErrorKind::Timeout
            }
            _ => ErrorKind::Bus,
        };
        Error::new(kind, e.to_string())
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(e: zbus::fdo::Error) -> Self {
        use zbus::fdo::Error as F;
        let kind = match &e {
            F::ServiceUnknown(_)
            | F::NameHasNoOwner(_)
            | F::UnknownObject(_)
            | F::UnknownInterface(_) => ErrorKind::NotRunning,
            F::AccessDenied(_) | F::InteractiveAuthorizationRequired(_) | F::AuthFailed(_) => {
                ErrorKind::Denied
            }
            F::NoReply(_) | F::Timeout(_) | F::TimedOut(_) => ErrorKind::Timeout,
            F::ZBus(z) => return Error::from(z.clone()),
            F::IOError(_) | F::Disconnected(_) | F::NoServer(_) => ErrorKind::Bus,
            _ => ErrorKind::Refused,
        };
        Error::new(kind, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbus_names_sort() {
        assert_eq!(
            kind_of_dbus_error("org.freedesktop.DBus.Error.ServiceUnknown"),
            ErrorKind::NotRunning
        );
        assert_eq!(
            kind_of_dbus_error("org.freedesktop.PolicyKit1.Error.NotAuthorized"),
            ErrorKind::Denied
        );
        assert_eq!(
            kind_of_dbus_error("org.freedesktop.DBus.Error.NoReply"),
            ErrorKind::Timeout
        );
        assert_eq!(
            kind_of_dbus_error("org.freedesktop.timedate1.NoSuchTimezone"),
            ErrorKind::Refused
        );
    }

    #[test]
    fn long_detail_is_capped() {
        let e = Error::new(ErrorKind::Bus, "é".repeat(500));
        assert_eq!(e.detail.chars().count(), MAX_DETAIL + 1);
        assert!(e.detail.ends_with('\u{2026}'));
        let exact = Error::new(ErrorKind::Bus, "a".repeat(MAX_DETAIL));
        assert_eq!(exact.detail.len(), MAX_DETAIL);
    }

    #[test]
    fn control_and_format_characters_are_replaced() {
        let e = Error::new(ErrorKind::Bus, "a\u{1b}[31mb\n\u{202E}gnp\u{200B}c");
        assert_eq!(e.detail, "a\u{FFFD}[31mb\u{FFFD}\u{FFFD}gnp\u{FFFD}c");
        assert!(!e.to_string().contains('\u{1b}'));
    }

    #[test]
    fn descriptions_are_sentences() {
        for k in [
            ErrorKind::NotRunning,
            ErrorKind::Denied,
            ErrorKind::Timeout,
            ErrorKind::Refused,
            ErrorKind::Bus,
        ] {
            let d = Error::new(k, "x").describe();
            assert!(
                d.ends_with('.') && d.chars().next().is_some_and(char::is_uppercase),
                "{d}"
            );
        }
    }
}
