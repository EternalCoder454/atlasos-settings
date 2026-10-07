//! A private D-Bus with python-dbusmock's model of a system service on it,
//! so the clients are tested without the machine's own services.
//!
//! Without dbus-daemon or python-dbusmock the tests skip, unless
//! `TELAMON_REQUIRE_DBUSMOCK=1` (CI and `scripts/dev.sh` set it), when they fail.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

pub struct MockBus {
    pub address: String,
    daemon: Child,
    mocks: Vec<Child>,
    dir: PathBuf,
}

const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>system</type>
  <listen>unix:path=@DIR@/bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

fn have(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

impl MockBus {
    /// A new private bus, or `None` (the test skips) when the tools are
    /// missing and not required.
    pub fn start() -> Option<MockBus> {
        let ok = have("dbus-daemon", &["--version"]) && have("python3", &["-c", "import dbusmock"]);
        if !ok {
            if std::env::var("TELAMON_REQUIRE_DBUSMOCK").as_deref() == Ok("1") {
                panic!(
                    "dbus-daemon and python3-dbusmock are required (TELAMON_REQUIRE_DBUSMOCK=1)"
                );
            }
            eprintln!("skipped: no dbus-daemon or python3-dbusmock");
            return None;
        }
        static N: AtomicUsize = AtomicUsize::new(0);
        // A private 0700 directory that must not exist yet (no squatting).
        let mut made = None;
        for _ in 0..16 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos());
            let d = std::env::temp_dir().join(format!(
                "settings-sys-test-{}-{}-{nanos}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::DirBuilder::new().mode(0o700).create(&d) {
                Ok(()) => {
                    made = Some(d);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("test dir: {e}"),
            }
        }
        let dir = made.expect("test dir: all names taken");
        let conf = dir.join("bus.conf");
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&conf)
            .expect("bus.conf");
        f.write_all(
            CONFIG
                .replace("@DIR@", dir.to_str().expect("utf-8 temp dir"))
                .as_bytes(),
        )
        .expect("bus.conf");
        drop(f);
        let mut daemon = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", conf.display()))
            .args(["--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("dbus-daemon");
        let mut line = String::new();
        BufReader::new(daemon.stdout.take().expect("stdout"))
            .read_line(&mut line)
            .expect("bus address");
        let address = line.trim().to_string();
        assert!(!address.is_empty(), "dbus-daemon printed no address");
        Some(MockBus {
            address,
            daemon,
            mocks: Vec::new(),
            dir,
        })
    }

    /// Starts a dbusmock template (`timedated`, `networkmanager`, ...) with
    /// its parameters as JSON, and waits until `name` is on the bus.
    pub fn template(&mut self, template: &str, params: Option<&str>, name: &str) {
        let mut cmd = Command::new("python3");
        cmd.args(["-m", "dbusmock", "--system", "--template", template]);
        if let Some(p) = params {
            cmd.args(["--parameters", p]);
        }
        let child = cmd
            .env("DBUS_SYSTEM_BUS_ADDRESS", &self.address)
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("python3 -m dbusmock");
        self.mocks.push(child);
        self.wait_for(name);
    }

    /// Starts one of this crate's own templates (`tests/templates/<name>.py`),
    /// for services python-dbusmock has none for.
    pub fn local_template(&mut self, template: &str, params: Option<&str>, name: &str) {
        let path = format!(
            "{}/tests/templates/{template}.py",
            env!("CARGO_MANIFEST_DIR")
        );
        self.template(&path, params, name);
    }

    /// Adds a method to a mocked object through dbusmock's own interface:
    /// `code` is the Python that answers it (`ret = ...`).
    pub fn add_method(
        &self,
        name: &str,
        path: &str,
        iface: &str,
        method: &str,
        sigs: (&str, &str),
        code: &str,
    ) {
        let conn = zbus::blocking::connection::Builder::address(self.address.as_str())
            .and_then(|b| b.build())
            .expect("connect to the test bus");
        conn.call_method(
            Some(name),
            path,
            Some("org.freedesktop.DBus.Mock"),
            "AddMethod",
            &(iface, method, sigs.0, sigs.1, code),
        )
        .expect("AddMethod");
    }

    fn wait_for(&self, name: &str) {
        let conn = zbus::blocking::connection::Builder::address(self.address.as_str())
            .and_then(|b| b.build())
            .expect("connect to the test bus");
        let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).expect("DBus proxy");
        let deadline = Instant::now() + Duration::from_secs(15);
        let name = zbus::names::BusName::try_from(name).expect("bus name");
        while !dbus.name_has_owner(name.clone()).unwrap_or(false) {
            assert!(
                Instant::now() < deadline,
                "{name} never appeared on the test bus"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn bus(&self) -> settings_sys::Bus {
        settings_sys::Bus::Address(self.address.clone())
    }
}

impl Drop for MockBus {
    fn drop(&mut self) {
        for m in &mut self.mocks {
            let _ = m.kill();
            let _ = m.wait();
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
