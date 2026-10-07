//! The crash reports waiting for the person's decision, and the ones they
//! sent: Privacy & Security's "Crash Reports" sheet. This was Telamon
//! Updater's Crash Reports screens; the tray still collects the reports
//! (while the setting is on) and says when one is waiting, and the report
//! files, the exact text shown and the sending are the framework's
//! (`telamon-framework-system::crash`), the same code as before.
//!
//! Consent: a report is shown as exactly what `Report::payload()` is, and
//! leaves this computer only when the person presses Send for that report
//! (`crash::send`, which refuses when reporting is off). "Don't Send" deletes
//! it. Made when the sheet opens and gone with it; every call runs on a
//! worker thread.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        /// The pending reports have been read.
        #[qproperty(bool, loaded)]
        /// A report is being sent.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        /// A one-off confirmation ("Crash report sent"); "" for none.
        #[qproperty(QString, info)]
        /// A crash report server is set up (reports can be sent).
        #[qproperty(bool, has_server, cxx_name = "hasServer")]
        /// The pending reports as JSON (`view`), oldest first.
        #[qproperty(QString, reports_json, cxx_name = "reportsJson")]
        #[qproperty(i32, count)]
        /// The sent reports as JSON, newest first.
        #[qproperty(QString, sent_json, cxx_name = "sentJson")]
        #[namespace = "telamon_settings"]
        type CrashReports = super::CrashReportsRust;
    }

    extern "RustQt" {
        /// Looks for new reports (the tray does it too), then lists them.
        #[qinvokable]
        fn refresh(self: Pin<&mut CrashReports>);

        /// Lists the reports again, without looking for new ones (cheap: the
        /// page uses it for "2 waiting").
        #[qinvokable]
        fn peek(self: Pin<&mut CrashReports>);

        /// "Send": the person saw the exact data. `event_id` is the report's
        /// `eventId` in `reportsJson`.
        #[qinvokable]
        #[cxx_name = "sendReport"]
        fn send_report(self: Pin<&mut CrashReports>, event_id: &QString);

        /// "Don't Send". Ignored while a send is running.
        #[qinvokable]
        #[cxx_name = "discardReport"]
        fn discard_report(self: Pin<&mut CrashReports>, event_id: &QString);

        /// True for `https://` links: the only ones that may be opened.
        #[qinvokable]
        #[cxx_name = "isSafeLink"]
        fn is_safe_link(self: &CrashReports, link: &QString) -> bool;
    }

    impl cxx_qt::Threading for CrashReports {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn crash_reports_make_unique() -> UniquePtr<CrashReports>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use telamon_framework_ui::telamon_framework_system::crash::{self, Report};
use telamon_updater_core::base::crash::{REPO, collect, display_name};
use telamon_updater_core::notes::is_safe_link;

#[derive(Default)]
pub struct CrashReportsRust {
    loaded: bool,
    busy: bool,
    error: QString,
    info: QString,
    has_server: bool,
    reports_json: QString,
    count: i32,
    sent_json: QString,

    // Not exposed to QML.
    pending: Vec<Report>,
    /// Bumped whenever the pending list changes by another route, so a
    /// slower read is not shown over it.
    generation: u64,
}

fn qs(s: &str) -> QString {
    QString::from(s)
}

/// "3 h 2 min", "2 d 5 h".
fn uptime_text(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{m} min"),
        (0, _) => format!("{h} h {m} min"),
        _ => format!("{d} d {h} h"),
    }
}

/// Everything the sheet shows of a report, in plain fields (never markup).
fn view(r: &Report, with_github: bool) -> Value {
    let or_unknown = |v: &Option<String>| v.clone().unwrap_or_default();
    json!({
        "eventId": r.event_id,
        "sentEventId": r.sent_event_id.clone().unwrap_or_default(),
        "issueUrl": r.issue_url.clone().filter(|u| crash::is_issue_url(u)).unwrap_or_default(),
        "type": r.report_type,
        "time": r.time,
        "appName": display_name(&r.app_name),
        "appVersion": or_unknown(&r.app_version),
        "category": r.category,
        "osVersion": or_unknown(&r.atlasos_version),
        "channel": or_unknown(&r.channel),
        "previousVersion": or_unknown(&r.previous_version),
        "kernel": or_unknown(&r.kernel),
        "gpu": or_unknown(&r.gpu),
        "gpuDriver": or_unknown(&r.gpu_driver),
        "uptime": uptime_text(r.uptime_secs),
        "message": r.message,
        "stacktrace": r.stacktrace,
        "payload": serde_json::to_string_pretty(&r.payload()).unwrap_or_default(),
        "githubUrl": if with_github { crash::github_issue_url(r, REPO) } else { String::new() },
    })
}

/// What a read finds.
struct Found {
    pending: Vec<Report>,
    sent: Vec<Report>,
    has_server: bool,
}

fn read(collect_first: bool) -> Found {
    if collect_first {
        // One collector at a time across processes (the tray's too).
        let _ = collect();
    }
    let mut sent = crash::sent();
    sent.reverse(); // newest first
    Found {
        pending: crash::pending(),
        sent,
        has_server: crash::Endpoint::load().is_some(),
    }
}

impl qobject::CrashReports {
    fn show(mut self: Pin<&mut Self>, found: Found) {
        self.as_mut().set_loaded(true);
        self.as_mut().set_has_server(found.has_server);
        let sent: Vec<Value> = found.sent.iter().map(|r| view(r, false)).collect();
        self.as_mut()
            .set_sent_json(qs(&Value::Array(sent).to_string()));
        self.show_pending(found.pending);
    }

    fn show_pending(mut self: Pin<&mut Self>, reports: Vec<Report>) {
        let with_github = !*self.has_server();
        let views: Vec<Value> = reports.iter().map(|r| view(r, with_github)).collect();
        self.as_mut().set_count(reports.len() as i32);
        self.as_mut()
            .set_reports_json(qs(&Value::Array(views).to_string()));
        self.as_mut().rust_mut().pending = reports;
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().generation += 1;
        let generation = self.rust().generation;
        worker::run(
            self.qt_thread(),
            "telamon-crash-reports",
            || read(true),
            move |mut this, found| {
                // Another change came in meanwhile: that one is newer.
                if this.rust().generation != generation {
                    return;
                }
                match found {
                    Some(found) => this.as_mut().show(found),
                    None => {
                        this.as_mut().set_loaded(true);
                        this.as_mut()
                            .set_error(qs("Settings couldn't read the crash reports."));
                    }
                }
            },
        );
    }

    pub fn send_report(mut self: Pin<&mut Self>, event_id: &QString) {
        // One send at a time; it also blocks "Don't Send".
        if *self.busy() {
            return;
        }
        let id = event_id.to_string();
        let Some(report) = self
            .rust()
            .pending
            .iter()
            .find(|r| r.event_id == id)
            .cloned()
        else {
            return;
        };
        self.as_mut().set_error(QString::default());
        self.as_mut().set_info(QString::default());
        self.as_mut().set_busy(true);
        let started = worker::run(
            self.qt_thread(),
            "telamon-crash-send",
            move || crash::send(&report),
            move |mut this, result| {
                this.as_mut().set_busy(false);
                let result = result.unwrap_or_else(|| Err(std::io::Error::other("internal error")));
                match result {
                    Ok(()) => {
                        this.as_mut().drop_pending(&id);
                        this.as_mut().set_info(qs(
                            "Crash report sent to the Telamon OS GitHub project. Thank you.",
                        ));
                        // A read dropped as stale may have found newer reports.
                        this.peek();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => this.set_error(qs(
                        "No crash report server is set up on this computer, so the report can't be sent. It stays here until you decide.",
                    )),
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => this.set_error(
                        qs("Crash reports are turned off, so nothing was sent."),
                    ),
                    Err(e) => {
                        log::warn!("sending a crash report: {e}");
                        this.set_error(qs("Settings couldn't send the crash report. Try again later."))
                    }
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
            self.set_error(qs("Settings couldn't start sending the report."));
        }
    }

    pub fn discard_report(self: Pin<&mut Self>, event_id: &QString) {
        // A send is running: don't change the list under it.
        if *self.busy() {
            return;
        }
        let id = event_id.to_string();
        if let Some(report) = self.rust().pending.iter().find(|r| r.event_id == id)
            && let Err(e) = crash::discard(report)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            // Still on disk: keep it listed rather than let it come back later.
            log::warn!("deleting a crash report: {e}");
            self.set_error(qs("Settings couldn't delete the crash report."));
            return;
        }
        self.drop_pending(&id);
    }

    pub fn is_safe_link(&self, link: &QString) -> bool {
        is_safe_link(&link.to_string())
    }

    /// Removes one pending report, by ID, from the list the sheet shows.
    fn drop_pending(mut self: Pin<&mut Self>, id: &str) {
        self.as_mut().rust_mut().generation += 1;
        let mut list = self.rust().pending.clone();
        list.retain(|r| r.event_id != id);
        self.show_pending(list);
    }

    /// Reads the folders again without collecting.
    pub fn peek(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().generation += 1;
        let generation = self.rust().generation;
        worker::run(
            self.qt_thread(),
            "telamon-crash-reports",
            || read(false),
            move |mut this, found| {
                if let Some(found) = found
                    && this.rust().generation == generation
                {
                    this.as_mut().show(found);
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime() {
        assert_eq!(uptime_text(90), "1 min");
        assert_eq!(uptime_text(3 * 3600 + 120), "3 h 2 min");
        assert_eq!(uptime_text(2 * 86_400 + 5 * 3600), "2 d 5 h");
    }
}
