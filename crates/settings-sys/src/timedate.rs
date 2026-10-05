//! systemd-timedated (`org.freedesktop.timedate1`): the time zone and
//! network time, for Date & Time. Changes are timedated's polkit actions
//! (`org.freedesktop.timedate1.*`).

use crate::bus::{Bus, INTERACTIVE_TIMEOUT};
use crate::{Error, ErrorKind};

#[zbus::proxy(
    interface = "org.freedesktop.timedate1",
    default_service = "org.freedesktop.timedate1",
    default_path = "/org/freedesktop/timedate1",
    gen_async = false,
    blocking_name = "TimedateProxy"
)]
trait Timedate {
    #[zbus(property)]
    fn timezone(&self) -> zbus::Result<String>;
    #[zbus(property, name = "LocalRTC")]
    fn local_rtc(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "CanNTP")]
    fn can_ntp(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "NTP")]
    fn ntp(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "NTPSynchronized")]
    fn ntp_synchronized(&self) -> zbus::Result<bool>;

    fn set_timezone(&self, timezone: &str, interactive: bool) -> zbus::Result<()>;
    #[zbus(name = "SetNTP")]
    fn set_ntp(&self, use_ntp: bool, interactive: bool) -> zbus::Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// An IANA name such as `Europe/Berlin`; empty when none is set or
    /// timedated returned something that isn't a valid name.
    pub timezone: String,
    pub local_rtc: bool,
    /// Whether network time can be turned on (a time sync service exists).
    pub can_ntp: bool,
    pub ntp: bool,
    pub ntp_synchronized: bool,
}

/// The longest time zone name accepted (the longest IANA name is 32).
pub const MAX_TIMEZONE: usize = 64;

/// Whether `tz` looks like an IANA time zone name. timedated checks it
/// against the zone database too; this keeps junk off the bus.
pub fn valid_timezone(tz: &str) -> bool {
    !tz.is_empty()
        && tz.len() <= MAX_TIMEZONE
        && !tz.starts_with('/')
        && !tz.split('/').any(|p| p.is_empty() || p == "." || p == "..")
        && tz
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'+'))
}

/// A time zone name read back from timedated: the name when it is valid,
/// otherwise an empty string, which means "unknown".
fn checked_timezone(tz: String) -> String {
    if valid_timezone(&tz) {
        tz
    } else {
        String::new()
    }
}

pub struct Timedate {
    conn: zbus::blocking::Connection,
    interactive: zbus::blocking::Connection,
}

impl Timedate {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Timedate {
            conn: bus.connect()?,
            interactive: bus.connect_with(INTERACTIVE_TIMEOUT)?,
        })
    }

    pub fn status(&self) -> Result<Status, Error> {
        let p = TimedateProxy::new(&self.conn)?;
        Ok(Status {
            timezone: checked_timezone(p.timezone()?),
            local_rtc: p.local_rtc()?,
            can_ntp: p.can_ntp()?,
            ntp: p.ntp()?,
            ntp_synchronized: p.ntp_synchronized()?,
        })
    }

    /// Sets the time zone; polkit may ask for a password.
    pub fn set_timezone(&self, tz: &str) -> Result<(), Error> {
        if !valid_timezone(tz) {
            return Err(Error::new(
                ErrorKind::Refused,
                format!("not a time zone name: {tz:?}"),
            ));
        }
        Ok(TimedateProxy::new(&self.interactive)?.set_timezone(tz, true)?)
    }

    /// Turns network time on or off; polkit may ask for a password.
    pub fn set_ntp(&self, on: bool) -> Result<(), Error> {
        Ok(TimedateProxy::new(&self.interactive)?.set_ntp(on, true)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{checked_timezone, valid_timezone};

    #[test]
    fn timezone_names() {
        for ok in [
            "UTC",
            "Europe/Berlin",
            "America/Argentina/Buenos_Aires",
            "Etc/GMT+5",
            "America/Port-au-Prince",
        ] {
            assert!(valid_timezone(ok), "{ok}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "Europe//Berlin",
            "Europe/",
            "a b",
            "Europe/Berlin\n",
            &"A".repeat(65),
        ] {
            assert!(!valid_timezone(bad), "{bad:?}");
        }
    }

    #[test]
    fn read_back_zone_is_validated() {
        assert_eq!(checked_timezone("Europe/Berlin".into()), "Europe/Berlin");
        for bad in ["", "../x", "a\u{1b}[0m", "\u{202E}UTC"] {
            assert_eq!(checked_timezone(bad.into()), "", "{bad:?}");
        }
        assert_eq!(checked_timezone("A".repeat(10_000)), "");
    }
}
