//! Power: the power mode (power-profiles-daemon), the battery (UPower) and
//! PowerDevil's "read your settings again" call. PowerDevil's own settings
//! are `powerdevilrc`, written through KConfig on the C++ side
//! (`cpp/powerconfig.cpp`); nothing here is privileged.

use crate::bus::Bus;
use crate::error::clean;
use crate::{Error, ErrorKind};
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// power-profiles-daemon's names (0.20 and later: UPower's bus name; before
/// that its own).
const PPD: [(&str, &str); 2] = [
    ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
    ),
];

/// The modes Settings offers, in the order a picker shows them.
pub const PROFILES: [&str; 3] = ["power-saver", "balanced", "performance"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileState {
    /// One of [`PROFILES`] ("balanced" when the daemon says something else).
    pub active: String,
    /// The modes this computer has (a desktop often has no power saver).
    pub available: Vec<String>,
    /// Why Performance runs slower than it should ("lap-detected",
    /// "high-operating-temperature"); empty when it doesn't.
    pub degraded: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeState {
    Unknown,
    Charging,
    Discharging,
    Empty,
    Full,
    /// Waiting to charge or discharge (plugged in below the charge limit).
    Holding,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Battery {
    /// 0 to 100.
    pub percentage: f64,
    pub state: ChargeState,
    /// Seconds; 0 when UPower doesn't know.
    pub time_to_empty: u64,
    pub time_to_full: u64,
    /// How much of its original capacity the battery still holds, in
    /// percent; `None` when UPower doesn't know.
    pub health: Option<f64>,
    /// The battery can stop charging at a limit.
    pub limit_supported: bool,
    pub limit_enabled: bool,
}

pub struct Power {
    conn: Connection,
}

fn is_profile(p: &str) -> bool {
    PROFILES.contains(&p)
}

impl Power {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Power {
            conn: bus.connect()?,
        })
    }

    fn ppd(&self) -> Result<Proxy<'_>, Error> {
        let mut last = None;
        for (name, path) in PPD {
            let p = Proxy::new(&self.conn, name, path, name)?;
            match p.get_property::<String>("ActiveProfile") {
                Ok(_) => return Ok(p),
                Err(e) => last = Some(Error::from(e)),
            }
        }
        Err(last.unwrap_or_else(|| Error::new(ErrorKind::NotRunning, "no power profiles service")))
    }

    /// The power mode, or `None` when there is no power-profiles-daemon.
    pub fn profile(&self) -> Result<Option<ProfileState>, Error> {
        let p = match self.ppd() {
            Ok(p) => p,
            Err(e) if e.kind == ErrorKind::NotRunning => return Ok(None),
            Err(e) => return Err(e),
        };
        let active: String = p.get_property("ActiveProfile")?;
        let listed: Vec<HashMap<String, OwnedValue>> = p.get_property("Profiles")?;
        let mut available = Vec::new();
        for mode in PROFILES {
            let has = listed.iter().any(|m| {
                m.get("Profile")
                    .and_then(|v| String::try_from(v.clone()).ok())
                    .is_some_and(|n| n == mode)
            });
            if has {
                available.push(mode.to_string());
            }
        }
        let degraded: String = p.get_property("PerformanceDegraded").unwrap_or_default();
        Ok(Some(ProfileState {
            active: if is_profile(&active) {
                active
            } else {
                "balanced".into()
            },
            available,
            degraded: clean(&degraded, 40),
        }))
    }

    /// Sets the power mode; no password is asked.
    pub fn set_profile(&self, profile: &str) -> Result<(), Error> {
        if !is_profile(profile) {
            return Err(Error::new(
                ErrorKind::Refused,
                format!("not a power mode: {profile:?}"),
            ));
        }
        self.ppd()?.set_property("ActiveProfile", profile)?;
        Ok(())
    }

    fn upower(&self, path: &str) -> Result<Proxy<'_>, Error> {
        Ok(Proxy::new(
            &self.conn,
            "org.freedesktop.UPower",
            path.to_string(),
            "org.freedesktop.UPower.Device",
        )?)
    }

    /// The path of the computer's own battery, `None` on a desktop.
    fn battery_path(&self) -> Result<Option<String>, Error> {
        let up = Proxy::new(
            &self.conn,
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower",
            "org.freedesktop.UPower",
        )?;
        let devices: Vec<OwnedObjectPath> = up.call("EnumerateDevices", &())?;
        for d in devices.iter().take(32) {
            let dev = self.upower(d.as_str())?;
            // Type 2 is a battery; a mouse or phone is not "the" battery.
            let kind: u32 = dev.get_property("Type").unwrap_or(0);
            let supply: bool = dev.get_property("PowerSupply").unwrap_or(false);
            let present: bool = dev.get_property("IsPresent").unwrap_or(false);
            if kind == 2 && supply && present {
                return Ok(Some(d.as_str().to_string()));
            }
        }
        Ok(None)
    }

    /// The computer's battery, `None` when it has none (a desktop).
    pub fn battery(&self) -> Result<Option<Battery>, Error> {
        let Some(path) = self.battery_path()? else {
            return Ok(None);
        };
        let dev = self.upower(&path)?;
        let state = match dev.get_property::<u32>("State").unwrap_or(0) {
            1 => ChargeState::Charging,
            2 => ChargeState::Discharging,
            3 => ChargeState::Empty,
            4 => ChargeState::Full,
            5 | 6 => ChargeState::Holding,
            _ => ChargeState::Unknown,
        };
        let secs = |name: &str| -> u64 {
            dev.get_property::<i64>(name)
                .ok()
                .and_then(|s| u64::try_from(s).ok())
                // A day and a half: more is UPower guessing.
                .filter(|s| *s < 36 * 3600)
                .unwrap_or(0)
        };
        let percentage = dev
            .get_property::<f64>("Percentage")
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        let health = dev
            .get_property::<f64>("Capacity")
            .ok()
            .filter(|c| *c > 0.0 && *c <= 200.0)
            .map(|c| c.min(100.0));
        Ok(Some(Battery {
            percentage,
            state,
            time_to_empty: secs("TimeToEmpty"),
            time_to_full: secs("TimeToFull"),
            health,
            limit_supported: dev
                .get_property("ChargeThresholdSupported")
                .unwrap_or(false),
            limit_enabled: dev.get_property("ChargeThresholdEnabled").unwrap_or(false),
        }))
    }

    /// Whether the computer has a lid (a laptop).
    pub fn lid_present(&self) -> Result<bool, Error> {
        let up = Proxy::new(
            &self.conn,
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower",
            "org.freedesktop.UPower",
        )?;
        Ok(up.get_property("LidIsPresent")?)
    }

    /// Stops charging at the battery's limit (usually 80 %), or charges to
    /// full again. UPower's own call: polkit may ask for a password.
    pub fn set_charge_limit(&self, on: bool) -> Result<(), Error> {
        let path = self
            .battery_path()?
            .ok_or_else(|| Error::new(ErrorKind::NotRunning, "no battery"))?;
        self.upower(&path)?
            .call::<_, _, ()>("EnableChargeThreshold", &(on,))?;
        Ok(())
    }
}

/// Tells PowerDevil (in the user's session) to read `powerdevilrc` again,
/// as its settings page does after saving. `NotRunning` when it isn't up:
/// the file is still right, and it is read at its next start.
pub fn reconfigure_powerdevil(bus: &Bus) -> Result<(), Error> {
    let conn = bus.connect()?;
    let p = Proxy::new(
        &conn,
        "org.kde.Solid.PowerManagement",
        "/org/kde/Solid/PowerManagement",
        "org.kde.Solid.PowerManagement",
    )?;
    p.call::<_, _, ()>("reparseConfiguration", &())?;
    Ok(())
}

/// "12 minutes", "2 hours 5 minutes": a time left in plain words, for the
/// battery row. `None` for 0 (unknown).
pub fn time_left(seconds: u64) -> Option<String> {
    if seconds == 0 {
        return None;
    }
    let minutes = (seconds + 30) / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    let hours = |n: u64| format!("{n} hour{}", if n == 1 { "" } else { "s" });
    let mins = |n: u64| format!("{n} minute{}", if n == 1 { "" } else { "s" });
    Some(match (h, m) {
        (0, 0) => "less than a minute".into(),
        (0, m) => mins(m),
        (h, 0) => hours(h),
        (h, m) => format!("{} {}", hours(h), mins(m)),
    })
}

#[cfg(test)]
mod tests {
    use super::time_left;

    #[test]
    fn times_are_plain_words() {
        assert_eq!(time_left(0), None);
        assert_eq!(time_left(20).as_deref(), Some("less than a minute"));
        assert_eq!(time_left(60).as_deref(), Some("1 minute"));
        assert_eq!(time_left(45 * 60).as_deref(), Some("45 minutes"));
        assert_eq!(time_left(3600).as_deref(), Some("1 hour"));
        assert_eq!(
            time_left(2 * 3600 + 5 * 60).as_deref(),
            Some("2 hours 5 minutes")
        );
    }
}
