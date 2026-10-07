//! fprintd (`net.reactivated.Fprint`): the fingerprint reader, and enrolling
//! a finger. Enrolling is fprintd's own polkit action
//! (`net.reactivated.fprint.device.enroll`); the scan itself happens in the
//! service, which tells us how it went through signals.

use crate::bus::Bus;
use crate::error::clean;
use crate::{Error, ErrorKind};
use async_io::Timer;
use futures_lite::{FutureExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, Proxy};
use zbus::proxy::MethodFlags;
use zbus::zvariant::OwnedObjectPath;

const NAME: &str = "net.reactivated.Fprint";
const MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
const MANAGER_IFACE: &str = "net.reactivated.Fprint.Manager";
const DEVICE_IFACE: &str = "net.reactivated.Fprint.Device";

/// How long one enrolment may take before it is given up.
pub const ENROLL_TIMEOUT: Duration = Duration::from_secs(120);

/// The fingers fprintd names, in the order a person counts them.
pub const FINGERS: [&str; 10] = [
    "right-index-finger",
    "right-middle-finger",
    "right-ring-finger",
    "right-little-finger",
    "right-thumb",
    "left-index-finger",
    "left-middle-finger",
    "left-ring-finger",
    "left-little-finger",
    "left-thumb",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reader {
    /// The device's object path (not shown).
    pub path: String,
    pub name: String,
    /// How many good scans an enrolment takes.
    pub stages: u32,
    /// A swipe sensor rather than a press one.
    pub swipe: bool,
}

/// How an enrolment is going.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// A scan was good: `done` of `of`.
    Stage {
        done: u32,
        of: u32,
    },
    /// A scan wasn't good; what to do, in plain words.
    Retry(&'static str),
    Completed,
    /// It ended without a saved fingerprint; why, in plain words.
    Failed(&'static str),
}

pub fn valid_finger(f: &str) -> bool {
    FINGERS.contains(&f)
}

fn remote_is(e: &zbus::Error, suffix: &str) -> bool {
    matches!(e, zbus::Error::MethodError(name, _, _) if name.as_str().ends_with(suffix))
}

/// What fprintd's `EnrollStatus` result means, in plain words.
fn status(result: &str, done: bool, scans: u32, of: u32) -> Progress {
    match result {
        "enroll-completed" => Progress::Completed,
        "enroll-stage-passed" => Progress::Stage {
            done: scans.min(of),
            of,
        },
        "enroll-retry-scan" => Progress::Retry("That scan didn't work. Try again."),
        "enroll-swipe-too-short" => Progress::Retry("Swipe your finger more slowly."),
        "enroll-finger-not-centered" => Progress::Retry("Center your finger on the reader."),
        "enroll-remove-and-retry" => Progress::Retry("Lift your finger, then try again."),
        "enroll-data-full" => Progress::Failed("The reader can't hold any more fingerprints."),
        "enroll-disconnected" => Progress::Failed("The fingerprint reader was unplugged."),
        "enroll-failed" => Progress::Failed("Settings couldn't read that finger."),
        _ if done => Progress::Failed("The fingerprint wasn't saved."),
        _ => Progress::Retry("That scan didn't work. Try again."),
    }
}

pub struct Fprint {
    conn: Connection,
}

impl Fprint {
    pub fn new(bus: &Bus) -> Result<Self, Error> {
        Ok(Fprint {
            conn: bus.connect_with(crate::bus::INTERACTIVE_TIMEOUT)?,
        })
    }

    fn device(&self, path: &str) -> Result<Proxy<'_>, Error> {
        Ok(Proxy::new(
            &self.conn,
            NAME,
            path.to_string(),
            DEVICE_IFACE,
        )?)
    }

    /// The default fingerprint reader, `None` when there is none (or no
    /// fprintd).
    pub fn reader(&self) -> Result<Option<Reader>, Error> {
        let m = Proxy::new(&self.conn, NAME, MANAGER_PATH, MANAGER_IFACE)?;
        let path: OwnedObjectPath = match m.call("GetDefaultDevice", &()) {
            Ok(p) => p,
            Err(e) if remote_is(&e, "NoSuchDevice") => return Ok(None),
            Err(e) => {
                let e = Error::from(e);
                return if e.kind == ErrorKind::NotRunning {
                    Ok(None)
                } else {
                    Err(e)
                };
            }
        };
        let d = self.device(path.as_str())?;
        let name: String = d.get_property("name").unwrap_or_default();
        let stages: i32 = d.get_property("num-enroll-stages").unwrap_or(5);
        let scan: String = d.get_property("scan-type").unwrap_or_default();
        Ok(Some(Reader {
            path: path.as_str().to_string(),
            name: clean(&name, 80),
            stages: u32::try_from(stages).unwrap_or(5).clamp(1, 30),
            swipe: scan == "swipe",
        }))
    }

    /// The fingers `user` has enrolled, in the order of [`FINGERS`].
    pub fn enrolled(&self, reader: &Reader, user: &str) -> Result<Vec<String>, Error> {
        let d = self.device(&reader.path)?;
        let listed: Vec<String> = match d.call("ListEnrolledFingers", &(user,)) {
            Ok(l) => l,
            Err(e) if remote_is(&e, "NoEnrolledPrints") => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        Ok(FINGERS
            .iter()
            .filter(|f| listed.iter().any(|l| l == **f))
            .map(|f| (*f).to_string())
            .collect())
    }

    fn claim(&self, reader: &Reader, user: &str) -> Result<(), Error> {
        self.device(&reader.path)?.call_with_flags::<_, _, ()>(
            "Claim",
            MethodFlags::AllowInteractiveAuth.into(),
            &(user,),
        )?;
        Ok(())
    }

    fn release(&self, reader: &Reader) {
        if let Ok(d) = self.device(&reader.path) {
            let _ = d.call::<_, _, ()>("Release", &());
        }
    }

    /// Enrols `finger` for `user`: claims the reader, asks it to scan until
    /// it has enough good scans, and tells `on` how each went. Stops (and
    /// says nothing more) when `cancel` is set, or after
    /// [`ENROLL_TIMEOUT`]. Returns whether a fingerprint was saved.
    pub fn enroll(
        &self,
        reader: &Reader,
        user: &str,
        finger: &str,
        cancel: &AtomicBool,
        mut on: impl FnMut(Progress),
    ) -> Result<bool, Error> {
        if !valid_finger(finger) {
            return Err(Error::new(ErrorKind::Refused, "not a finger"));
        }
        self.claim(reader, user)?;
        let result = self.enroll_claimed(reader, finger, cancel, &mut on);
        self.release(reader);
        result
    }

    fn enroll_claimed(
        &self,
        reader: &Reader,
        finger: &str,
        cancel: &AtomicBool,
        on: &mut impl FnMut(Progress),
    ) -> Result<bool, Error> {
        let async_conn = self.conn.inner().clone();
        let path = reader.path.clone();
        let device = self.device(&reader.path)?;
        let of = reader.stages;
        // Listen before asking, so no early answer is missed.
        let mut stream = async_io::block_on(async {
            let p = zbus::Proxy::new(&async_conn, NAME, path, DEVICE_IFACE).await?;
            p.receive_signal("EnrollStatus").await
        })?;
        device.call_with_flags::<_, _, ()>(
            "EnrollStart",
            MethodFlags::AllowInteractiveAuth.into(),
            &(finger,),
        )?;
        let started = Instant::now();
        let mut scans = 0u32;
        let outcome = loop {
            if cancel.load(Ordering::Relaxed) || started.elapsed() > ENROLL_TIMEOUT {
                break Ok(false);
            }
            // A quarter second at a time, to look at `cancel`.
            let next = async_io::block_on(async { Some(stream.next().await) }.or(async {
                Timer::after(Duration::from_millis(250)).await;
                None
            }));
            let Some(msg) = next else { continue };
            let Some(msg) = msg else {
                break Err(Error::new(ErrorKind::Bus, "the signal stream ended"));
            };
            let Ok((result, done)) = msg.body().deserialize::<(String, bool)>() else {
                continue;
            };
            if result == "enroll-stage-passed" {
                scans += 1;
            }
            let p = status(&result, done, scans, of);
            let finished = matches!(p, Progress::Completed | Progress::Failed(_));
            let saved = p == Progress::Completed;
            on(p);
            if finished {
                break Ok(saved);
            }
            if done {
                break Ok(false);
            }
        };
        // Always stop the scan: the reader stays busy otherwise.
        let _ = device.call::<_, _, ()>("EnrollStop", &());
        outcome
    }

    /// Removes every fingerprint of `user` from the reader.
    pub fn delete_all(&self, reader: &Reader, user: &str) -> Result<(), Error> {
        self.claim(reader, user)?;
        let r = self
            .device(&reader.path)?
            .call_with_flags::<_, _, ()>(
                "DeleteEnrolledFingers2",
                MethodFlags::AllowInteractiveAuth.into(),
                &(),
            )
            .map(|_| ())
            .map_err(Error::from);
        self.release(reader);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingers_are_fprintds_names() {
        assert!(valid_finger("left-thumb"));
        assert!(!valid_finger("any"));
        assert!(!valid_finger("left-thumb\n"));
        assert!(!valid_finger(""));
    }

    #[test]
    fn statuses_are_plain_words() {
        assert_eq!(
            status("enroll-stage-passed", false, 2, 5),
            Progress::Stage { done: 2, of: 5 }
        );
        assert_eq!(
            status("enroll-stage-passed", false, 9, 5),
            Progress::Stage { done: 5, of: 5 }
        );
        assert_eq!(status("enroll-completed", true, 5, 5), Progress::Completed);
        assert!(matches!(
            status("enroll-retry-scan", false, 1, 5),
            Progress::Retry(_)
        ));
        assert!(matches!(
            status("enroll-data-full", true, 0, 5),
            Progress::Failed(_)
        ));
        // Anything new fprintd adds still ends the enrolment when done.
        assert!(matches!(
            status("enroll-something-new", true, 0, 5),
            Progress::Failed(_)
        ));
        assert!(matches!(
            status("enroll-something-new", false, 0, 5),
            Progress::Retry(_)
        ));
    }
}
