//! Users' backend: the signed-in user's account and the other people on
//! this computer (AccountsService), and fingerprints (fprintd). Passwords
//! go straight to AccountsService as a hash and are held only for the call
//! (overwritten after), never logged. Lives only while the page is shown;
//! every call runs on a worker thread.

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
        /// The first read has answered (or failed).
        #[qproperty(bool, loaded)]
        /// AccountsService answered the last read.
        #[qproperty(bool, available)]
        /// A change is under way.
        #[qproperty(bool, busy)]
        /// The last failure in plain words; "" for none.
        #[qproperty(QString, error)]
        /// The signed-in user's ID, -1 when unknown.
        #[qproperty(i32, me_uid, cxx_name = "meUid")]
        /// Sign-in name.
        #[qproperty(QString, me_name, cxx_name = "meName")]
        #[qproperty(QString, me_real_name, cxx_name = "meRealName")]
        /// A picture's path; "" for none.
        #[qproperty(QString, me_picture, cxx_name = "mePicture")]
        #[qproperty(bool, me_admin, cxx_name = "meAdmin")]
        /// Signs in without a password at start-up.
        #[qproperty(bool, auto_login, cxx_name = "autoLogin")]
        /// The other people, as JSON: [{uid, name, realName, picture, admin}].
        #[qproperty(QString, others_json, cxx_name = "othersJson")]
        /// There is a fingerprint reader.
        #[qproperty(bool, has_reader, cxx_name = "hasReader")]
        #[qproperty(QString, reader_name, cxx_name = "readerName")]
        #[qproperty(bool, swipe)]
        /// The fingers enrolled, fprintd's names.
        #[qproperty(QStringList, fingers)]
        /// "idle", "scanning", "saved" or "failed".
        #[qproperty(QString, enroll_state, cxx_name = "enrollState")]
        /// The scans done and needed, while enrolling.
        #[qproperty(i32, enroll_done, cxx_name = "enrollDone")]
        #[qproperty(i32, enroll_of, cxx_name = "enrollOf")]
        /// What to do or what went wrong, while enrolling.
        #[qproperty(QString, enroll_message, cxx_name = "enrollMessage")]
        #[namespace = "telamon_settings"]
        type UsersPage = super::UsersPageRust;
    }

    extern "RustQt" {
        /// Reads everything again.
        #[qinvokable]
        fn refresh(self: Pin<&mut UsersPage>);

        #[qinvokable]
        #[cxx_name = "changeName"]
        fn change_name(self: Pin<&mut UsersPage>, uid: i32, name: &QString);

        #[qinvokable]
        #[cxx_name = "changePicture"]
        fn change_picture(self: Pin<&mut UsersPage>, uid: i32, path: &QString);

        #[qinvokable]
        #[cxx_name = "changePassword"]
        fn change_password(self: Pin<&mut UsersPage>, uid: i32, password: &QString);

        #[qinvokable]
        #[cxx_name = "changeAdmin"]
        fn change_admin(self: Pin<&mut UsersPage>, uid: i32, admin: bool);

        #[qinvokable]
        #[cxx_name = "changeAutoLogin"]
        fn change_auto_login(self: Pin<&mut UsersPage>, on: bool);

        #[qinvokable]
        #[cxx_name = "addUser"]
        fn add_user(
            self: Pin<&mut UsersPage>,
            name: &QString,
            real_name: &QString,
            admin: bool,
            password: &QString,
        );

        #[qinvokable]
        #[cxx_name = "removeUser"]
        fn remove_user(self: Pin<&mut UsersPage>, uid: i32, remove_files: bool);

        /// Scans `finger` (fprintd's name) until it is enrolled.
        #[qinvokable]
        #[cxx_name = "startEnroll"]
        fn start_enroll(self: Pin<&mut UsersPage>, finger: &QString);

        /// Stops a scan that is under way; the page goes back to "idle".
        #[qinvokable]
        #[cxx_name = "cancelEnroll"]
        fn cancel_enroll(self: Pin<&mut UsersPage>);

        /// Removes every fingerprint of the signed-in user.
        #[qinvokable]
        #[cxx_name = "removeFingerprints"]
        fn remove_fingerprints(self: Pin<&mut UsersPage>);

        #[qinvokable]
        #[cxx_name = "validUserName"]
        fn valid_user_name(self: &UsersPage, name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "validRealName"]
        fn valid_real_name(self: &UsersPage, name: &QString) -> bool;

        /// A sign-in name for a full name.
        #[qinvokable]
        #[cxx_name = "suggestUserName"]
        fn suggest_user_name(self: &UsersPage, real_name: &QString) -> QString;

        /// The words for a password's strength, 0 (very weak) to 4 (strong),
        /// -1 for nothing typed.
        #[qinvokable]
        #[cxx_name = "passwordScore"]
        fn password_score(self: &UsersPage, password: &QString) -> i32;

        /// Whether `password` is long enough to be used (and not too long).
        #[qinvokable]
        #[cxx_name = "validPassword"]
        fn valid_password(self: &UsersPage, password: &QString) -> bool;
    }

    impl cxx_qt::Threading for UsersPage {}

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn users_page_make_unique() -> UniquePtr<UsersPage>;
    }
}

use crate::worker;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use serde_json::json;
use settings_sys::accounts::{self, Accounts, User};
use settings_sys::fprint::{self, Fprint, Progress, Reader};
use settings_sys::{Bus, Error, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A password for the length of a call: overwritten when dropped.
struct Secret(String);

impl Secret {
    fn of(q: &QString) -> Secret {
        Secret(q.to_string())
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // SAFETY: zero bytes are valid UTF-8.
        for b in unsafe { self.0.as_bytes_mut() } {
            // SAFETY: a valid, aligned reference to a byte.
            unsafe { core::ptr::write_volatile(b, 0) };
        }
    }
}

#[derive(Default)]
pub struct UsersPageRust {
    loaded: bool,
    available: bool,
    busy: bool,
    error: QString,
    me_uid: i32,
    me_name: QString,
    me_real_name: QString,
    me_picture: QString,
    me_admin: bool,
    auto_login: bool,
    others_json: QString,
    has_reader: bool,
    reader_name: QString,
    swipe: bool,
    fingers: QStringList,
    enroll_state: QString,
    enroll_done: i32,
    enroll_of: i32,
    enroll_message: QString,
    /// The reader, for enrolling and removing fingers.
    reader: Option<Reader>,
    /// Set to stop the scan that is under way.
    cancel: Option<Arc<AtomicBool>>,
}

/// What a read gives.
struct Read {
    accounts: Result<(Vec<User>, u64), Error>,
    reader: Result<Option<(Reader, Vec<String>)>, Error>,
}

fn read() -> Read {
    let me = accounts::current_uid();
    let accounts = Accounts::new(&Bus::System).and_then(|a| {
        let mut users = a.users()?;
        // The signed-in user may not be among the cached ones.
        if !users.iter().any(|u| u.uid == me) {
            users.insert(0, a.user(me)?);
        }
        Ok((users, me))
    });
    let reader = Fprint::new(&Bus::System).and_then(|f| {
        let Some(r) = f.reader()? else {
            return Ok(None);
        };
        let name = accounts
            .as_ref()
            .ok()
            .and_then(|(u, uid)| u.iter().find(|u| u.uid == *uid))
            .map(|u| u.name.clone())
            .unwrap_or_default();
        let fingers = if name.is_empty() {
            Vec::new()
        } else {
            f.enrolled(&r, &name).unwrap_or_default()
        };
        Ok(Some((r, fingers)))
    });
    Read { accounts, reader }
}

fn qs(s: &str) -> QString {
    QString::from(s)
}

fn list(items: &[String]) -> QStringList {
    let mut l = QStringList::default();
    for i in items {
        l.append(qs(i));
    }
    l
}

/// What the user can do about a failed change, as plain words.
fn describe(what: &str, e: &Error) -> String {
    worker::describe(what, e)
}

impl qobject::UsersPage {
    fn show(mut self: Pin<&mut Self>, read: Option<Read>) {
        self.as_mut().set_loaded(true);
        let Some(read) = read else {
            self.as_mut()
                .set_error(qs("Settings couldn't read the user accounts."));
            return;
        };
        let mut problem = None;
        match read.accounts {
            Ok((users, me)) => {
                self.as_mut().set_available(true);
                let mine = users.iter().find(|u| u.uid == me);
                if let Some(m) = mine {
                    self.as_mut().set_me_uid(i32::try_from(m.uid).unwrap_or(-1));
                    self.as_mut().set_me_name(qs(&m.name));
                    self.as_mut().set_me_real_name(qs(&m.real_name));
                    self.as_mut().set_me_picture(qs(&m.icon));
                    self.as_mut().set_me_admin(m.admin);
                    self.as_mut().set_auto_login(m.auto_login);
                }
                let others: Vec<_> = users
                    .iter()
                    .filter(|u| u.uid != me)
                    .map(|u| {
                        json!({
                            "uid": u.uid,
                            "name": u.name,
                            "realName": u.real_name,
                            "picture": u.icon,
                            "admin": u.admin,
                        })
                    })
                    .collect();
                let text = serde_json::to_string(&others).unwrap_or_else(|_| "[]".into());
                self.as_mut().set_others_json(qs(&text));
            }
            Err(e) => {
                self.as_mut().set_available(false);
                problem = Some(describe("reading the user accounts", &e));
            }
        }
        match read.reader {
            Ok(Some((r, fingers))) => {
                self.as_mut().set_has_reader(true);
                self.as_mut().set_reader_name(qs(&r.name));
                self.as_mut().set_swipe(r.swipe);
                self.as_mut().set_fingers(list(&fingers));
                self.as_mut().rust_mut().reader = Some(r);
            }
            Ok(None) => {
                self.as_mut().set_has_reader(false);
                self.as_mut().rust_mut().reader = None;
            }
            Err(e) => {
                self.as_mut().set_has_reader(false);
                self.as_mut().rust_mut().reader = None;
                // A fingerprint reader that can't be read is not worth a banner.
                log::warn!("reading the fingerprint reader: {}", e.detail);
            }
        }
        self.as_mut()
            .set_error(qs(problem.unwrap_or_default().as_str()));
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        worker::run(qt, "users-read", read, |o, r| o.show(r));
    }

    /// Runs change `job`, then reads everything again.
    fn change<J>(self: Pin<&mut Self>, what: &'static str, job: J)
    where
        J: FnOnce() -> Result<(), Error> + Send + 'static,
    {
        self.change_saying(what, job, |_| None);
    }

    /// Like [`change`](Self::change), with `say` first to tell what went wrong
    /// in the person's words, when the job knows (a picture that won't do).
    fn change_saying<J>(
        mut self: Pin<&mut Self>,
        what: &'static str,
        job: J,
        say: fn(&Error) -> Option<String>,
    ) where
        J: FnOnce() -> Result<(), Error> + Send + 'static,
    {
        if self.rust().busy {
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let qt = self.qt_thread();
        let started = worker::run(
            qt,
            "users-change",
            move || {
                let r = job();
                (r, read())
            },
            move |mut o, r| {
                o.as_mut().set_busy(false);
                match r {
                    Some((result, read)) => {
                        o.as_mut().show(Some(read));
                        if let Err(e) = result {
                            let text = say(&e).unwrap_or_else(|| describe(what, &e));
                            o.as_mut().set_error(qs(&text));
                        }
                    }
                    None => o.as_mut().show(None),
                }
            },
        );
        if !started {
            self.as_mut().set_busy(false);
        }
    }

    pub fn change_name(self: Pin<&mut Self>, uid: i32, name: &QString) {
        let name = name.to_string();
        self.change("changing the name", move || {
            Accounts::new(&Bus::System)?.set_real_name(uid_of(uid)?, &name)
        });
    }

    pub fn change_picture(self: Pin<&mut Self>, uid: i32, path: &QString) {
        let path = path.to_string();
        // The file is looked at on the worker thread (it may be on a slow
        // disk), and the reason it won't do is said in words.
        self.change_saying(
            "changing the picture",
            move || Accounts::new(&Bus::System)?.set_icon(uid_of(uid)?, &path),
            |e| {
                e.detail
                    .strip_prefix(accounts::PICTURE_PREFIX)
                    .map(str::to_string)
            },
        );
    }

    pub fn change_password(self: Pin<&mut Self>, uid: i32, password: &QString) {
        let password = Secret::of(password);
        self.change("changing the password", move || {
            Accounts::new(&Bus::System)?.set_password(uid_of(uid)?, &password.0)
        });
    }

    pub fn change_admin(self: Pin<&mut Self>, uid: i32, admin: bool) {
        self.change("changing the account type", move || {
            Accounts::new(&Bus::System)?.set_admin(uid_of(uid)?, admin)
        });
    }

    pub fn change_auto_login(self: Pin<&mut Self>, on: bool) {
        self.change("changing automatic login", move || {
            Accounts::new(&Bus::System)?.set_auto_login(accounts::current_uid(), on)
        });
    }

    pub fn add_user(
        self: Pin<&mut Self>,
        name: &QString,
        real_name: &QString,
        admin: bool,
        password: &QString,
    ) {
        let name = name.to_string();
        let real_name = real_name.to_string();
        let password = Secret::of(password);
        self.change("adding the user", move || {
            Accounts::new(&Bus::System)?
                .create_user(&name, &real_name, admin, &password.0)
                .map(|_| ())
        });
    }

    pub fn remove_user(self: Pin<&mut Self>, uid: i32, remove_files: bool) {
        self.change("removing the user", move || {
            let uid = uid_of(uid)?;
            // Never the account in use.
            if uid == accounts::current_uid() {
                return Err(Error::new(ErrorKind::Refused, "that is you"));
            }
            Accounts::new(&Bus::System)?.delete_user(uid, remove_files)
        });
    }

    pub fn start_enroll(mut self: Pin<&mut Self>, finger: &QString) {
        let finger = finger.to_string();
        let Some(reader) = self.rust().reader.clone() else {
            return;
        };
        if self.rust().cancel.is_some() || !fprint::valid_finger(&finger) {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().cancel = Some(Arc::clone(&cancel));
        self.as_mut().set_enroll_state(qs("scanning"));
        self.as_mut().set_enroll_done(0);
        self.as_mut()
            .set_enroll_of(i32::try_from(reader.stages).unwrap_or(5));
        self.as_mut().set_enroll_message(qs(if reader.swipe {
            "Swipe your finger across the reader."
        } else {
            "Touch the reader with your finger."
        }));
        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("fingerprint".into())
            .spawn(move || {
                let me = accounts::current_uid();
                let user = Accounts::new(&Bus::System)
                    .and_then(|a| a.user(me))
                    .map(|u| u.name);
                let outcome = user.and_then(|user| {
                    let f = Fprint::new(&Bus::System)?;
                    f.enroll(&reader, &user, &finger, &cancel, |p| {
                        let _ = qt.queue(move |o| o.progress(p));
                    })
                });
                let cancelled = cancel.load(Ordering::Relaxed);
                let _ = qt.queue(move |o| o.enrolled(outcome, cancelled));
            });
        if spawned.is_err() {
            self.as_mut().rust_mut().cancel = None;
            self.as_mut().set_enroll_state(qs("idle"));
        }
    }

    fn progress(mut self: Pin<&mut Self>, p: Progress) {
        if self.rust().cancel.is_none() {
            return;
        }
        match p {
            Progress::Stage { done, of } => {
                self.as_mut()
                    .set_enroll_done(i32::try_from(done).unwrap_or(0));
                self.as_mut().set_enroll_of(i32::try_from(of).unwrap_or(5));
                self.as_mut()
                    .set_enroll_message(qs("Good. Lift your finger and touch the reader again."));
            }
            Progress::Retry(text) => self.as_mut().set_enroll_message(qs(text)),
            Progress::Completed | Progress::Failed(_) => {}
        }
    }

    fn enrolled(mut self: Pin<&mut Self>, outcome: Result<bool, Error>, cancelled: bool) {
        self.as_mut().rust_mut().cancel = None;
        if cancelled {
            self.as_mut().set_enroll_state(qs("idle"));
        } else {
            match outcome {
                Ok(true) => {
                    self.as_mut().set_enroll_state(qs("saved"));
                    self.as_mut().set_enroll_message(qs("Fingerprint saved."));
                }
                Ok(false) => {
                    self.as_mut().set_enroll_state(qs("failed"));
                    self.as_mut()
                        .set_enroll_message(qs("The fingerprint wasn't saved. Try again."));
                }
                Err(e) => {
                    let text = describe("adding a fingerprint", &e);
                    self.as_mut().set_enroll_state(qs("failed"));
                    self.as_mut().set_enroll_message(qs(&text));
                }
            }
        }
        self.refresh();
    }

    pub fn cancel_enroll(mut self: Pin<&mut Self>) {
        if let Some(c) = &self.rust().cancel {
            c.store(true, Ordering::Relaxed);
        }
        self.as_mut().set_enroll_state(qs("idle"));
    }

    pub fn remove_fingerprints(self: Pin<&mut Self>) {
        let Some(reader) = self.rust().reader.clone() else {
            return;
        };
        self.change("removing fingerprints", move || {
            let user = Accounts::new(&Bus::System)?
                .user(accounts::current_uid())?
                .name;
            Fprint::new(&Bus::System)?.delete_all(&reader, &user)
        });
    }

    pub fn valid_user_name(&self, name: &QString) -> bool {
        accounts::valid_user_name(&name.to_string())
    }

    pub fn valid_real_name(&self, name: &QString) -> bool {
        accounts::valid_real_name(&name.to_string())
    }

    pub fn suggest_user_name(&self, real_name: &QString) -> QString {
        qs(&accounts::suggest_user_name(&real_name.to_string()))
    }

    pub fn valid_password(&self, password: &QString) -> bool {
        let p = Secret::of(password);
        let n = p.0.chars().count();
        n >= accounts::MIN_PASSWORD && p.0.len() <= accounts::MAX_PASSWORD
    }

    pub fn password_score(&self, password: &QString) -> i32 {
        password_score(&Secret::of(password).0)
    }
}

fn uid_of(uid: i32) -> Result<u64, Error> {
    u64::try_from(uid).map_err(|_| Error::new(ErrorKind::Refused, "no such user"))
}

/// 0 (very weak) to 4 (strong), -1 for nothing typed: length and the kinds
/// of characters, not a promise. AccountsService stores the hash it is given
/// and checks nothing, so the only rule enforced is the shortest allowed
/// (`accounts::MIN_PASSWORD`, in `hash_password`).
fn password_score(p: &str) -> i32 {
    if p.is_empty() {
        return -1;
    }
    let chars = p.chars().count();
    let kinds = [
        p.chars().any(|c| c.is_lowercase()),
        p.chars().any(|c| c.is_uppercase()),
        p.chars().any(|c| c.is_ascii_digit()),
        p.chars().any(|c| !c.is_alphanumeric()),
    ]
    .iter()
    .filter(|k| **k)
    .count();
    let distinct = {
        let mut v: Vec<char> = p.chars().collect();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    if chars < 6 || distinct < 4 {
        0
    } else if chars < 8 {
        1
    } else if chars >= 16 || (chars >= 12 && kinds >= 3) {
        4
    } else if chars >= 12 || kinds >= 3 {
        3
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::password_score;

    #[test]
    fn password_strength() {
        assert_eq!(password_score(""), -1);
        assert_eq!(password_score("abc"), 0);
        assert_eq!(password_score("aaaaaaaaaaaa"), 0);
        assert_eq!(password_score("abcdefg"), 1);
        assert_eq!(password_score("abcdefgh"), 2);
        assert_eq!(password_score("Abcdefg1"), 3);
        assert_eq!(password_score("correct horse battery"), 4);
        assert!(password_score("Tr0ub4dor&3xyz") >= 3);
    }
}
