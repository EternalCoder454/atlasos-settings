//! Flatpak apps and what they may reach, the way flatpak-kcm does it: read
//! from each app's `metadata` and the user's override files
//! (`~/.local/share/flatpak/overrides/<app>`, a key file with a `[Context]`
//! group), and written to the user's override file only. Nothing here is
//! privileged and no Flatpak program runs. Other groups and keys of an
//! override file are kept as they are.

use crate::error::clean;
use crate::portal::valid_app_id;
use crate::{Error, ErrorKind};
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Most apps listed.
pub const MAX_APPS: usize = 500;
/// The largest metadata or override file read.
const MAX_FILE: u64 = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    pub id: String,
    pub name: String,
    /// A theme icon name.
    pub icon: String,
}

/// How much of the person's files an app can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Files {
    /// Only its own files.
    None,
    /// The Downloads folder.
    Downloads,
    /// The home folder.
    Home,
    /// Everything.
    Everything,
}

impl Files {
    pub fn name(self) -> &'static str {
        match self {
            Files::None => "none",
            Files::Downloads => "downloads",
            Files::Home => "home",
            Files::Everything => "everything",
        }
    }

    pub fn from_name(n: &str) -> Option<Files> {
        [
            Files::None,
            Files::Downloads,
            Files::Home,
            Files::Everything,
        ]
        .into_iter()
        .find(|f| f.name() == n)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub files: Files,
    /// `devices=all`: cameras, USB devices, game controllers.
    pub devices: bool,
    /// `shared=network`.
    pub network: bool,
}

/// Where Flatpak keeps things.
#[derive(Clone, Debug)]
pub struct Roots {
    /// The installations' folders (`/var/lib/flatpak`, the user's
    /// `~/.local/share/flatpak`), each with an `app` folder.
    pub installations: Vec<PathBuf>,
    /// The user's override folder (`~/.local/share/flatpak/overrides`).
    pub overrides: PathBuf,
}

impl Roots {
    /// The system and the user's installation, for `data_home`
    /// (`$XDG_DATA_HOME`, `~/.local/share`).
    pub fn standard(data_home: &Path) -> Roots {
        let user = data_home.join("flatpak");
        Roots {
            installations: vec![PathBuf::from("/var/lib/flatpak"), user.clone()],
            overrides: user.join("overrides"),
        }
    }
}

/// A key file as lines of groups and keys, in order; comments are not kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct KeyFile {
    groups: Vec<(String, Vec<(String, String)>)>,
}

impl KeyFile {
    fn parse(text: &str) -> KeyFile {
        let mut kf = KeyFile::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(g) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                kf.groups.push((g.to_string(), Vec::new()));
            } else if let Some((k, v)) = line.split_once('=')
                && let Some(last) = kf.groups.last_mut()
            {
                last.1.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        kf
    }

    fn get(&self, group: &str, key: &str) -> Option<&str> {
        self.groups
            .iter()
            .find(|(g, _)| g == group)?
            .1
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The `;`-separated list at `key`.
    fn list(&self, group: &str, key: &str) -> Vec<String> {
        self.get(group, key)
            .map(|v| {
                v.split(';')
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Sets the list at `key`, or removes the key (and its group when that
    /// leaves it empty) for an empty list.
    fn set_list(&mut self, group: &str, key: &str, items: &[String]) {
        let value: String = items.iter().map(|i| format!("{i};")).collect();
        let gi = self.groups.iter().position(|(g, _)| g == group);
        match (gi, items.is_empty()) {
            (None, true) => {}
            (None, false) => self
                .groups
                .push((group.to_string(), vec![(key.to_string(), value)])),
            (Some(gi), empty) => {
                let keys = &mut self.groups[gi].1;
                match (keys.iter().position(|(k, _)| k == key), empty) {
                    (Some(ki), true) => {
                        keys.remove(ki);
                    }
                    (Some(ki), false) => keys[ki].1 = value,
                    (None, true) => {}
                    (None, false) => keys.push((key.to_string(), value)),
                }
                if self.groups[gi].1.is_empty() {
                    self.groups.remove(gi);
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for (i, (g, keys)) in self.groups.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&format!("[{g}]\n"));
            for (k, v) in keys {
                out.push_str(&format!("{k}={v}\n"));
            }
        }
        out
    }
}

/// An item of a context list after an override: `x` adds, `!x` removes.
fn apply(base: &[String], overrides: &[String]) -> Vec<String> {
    let mut out: Vec<String> = base.to_vec();
    for o in overrides {
        if let Some(n) = o.strip_prefix('!') {
            out.retain(|i| i != n);
        } else if !out.contains(o) {
            out.push(o.clone());
        }
    }
    out
}

/// A filesystem entry without its `:ro`, `:rw` or `:create`.
fn fs_name(entry: &str) -> &str {
    entry.split(':').next().unwrap_or(entry)
}

/// The text of the file at `path`, when it is a regular file of at most
/// [`MAX_FILE`] bytes. It is opened once and what was opened is what is read
/// (a file swapped in after the size check can't be bigger), and a pipe or a
/// device in its place is not waited for. A link to a regular file is
/// followed: people keep their override files in a dotfiles folder.
fn read_limited(path: &Path) -> Option<String> {
    use std::io::Read;
    let f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    let meta = f.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return None;
    }
    let mut text = String::new();
    f.take(MAX_FILE + 1).read_to_string(&mut text).ok()?;
    (text.len() as u64 <= MAX_FILE).then_some(text)
}

fn theme_icon(icon: &str) -> String {
    let ok = !icon.is_empty()
        && icon.len() <= 100
        && icon
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'));
    if ok {
        icon.to_string()
    } else {
        "application-x-executable".to_string()
    }
}

pub struct Flatpak {
    roots: Roots,
}

impl Flatpak {
    pub fn new(roots: Roots) -> Self {
        Flatpak { roots }
    }

    /// The folder of an installed app's active deployment.
    fn deployment(&self, id: &str) -> Option<PathBuf> {
        self.roots
            .installations
            .iter()
            .map(|r| r.join("app").join(id).join("current/active"))
            .find(|d| d.join("metadata").is_file())
    }

    /// The installed apps (not runtimes), by name.
    pub fn apps(&self) -> Vec<App> {
        let mut apps: Vec<App> = Vec::new();
        for root in &self.roots.installations {
            let Ok(dir) = fs::read_dir(root.join("app")) else {
                continue;
            };
            for entry in dir.flatten().take(MAX_APPS * 2) {
                let Some(id) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                if !valid_app_id(&id) || apps.iter().any(|a| a.id == id) {
                    continue;
                }
                let Some(dep) = self.deployment(&id) else {
                    continue;
                };
                let Some(meta) = read_limited(&dep.join("metadata")) else {
                    continue;
                };
                // Only apps have an [Application] group.
                if KeyFile::parse(&meta).get("Application", "name").is_none() {
                    continue;
                }
                let desktop =
                    read_limited(&dep.join(format!("export/share/applications/{id}.desktop")))
                        .map(|t| KeyFile::parse(&t));
                let field = |k: &str| {
                    desktop
                        .as_ref()
                        .and_then(|d| d.get("Desktop Entry", k))
                        .unwrap_or("")
                        .to_string()
                };
                let name = clean(field("Name").trim(), 100);
                apps.push(App {
                    name: if name.is_empty() { id.clone() } else { name },
                    icon: theme_icon(&field("Icon")),
                    id,
                });
                if apps.len() >= MAX_APPS {
                    break;
                }
            }
        }
        apps.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        apps
    }

    fn override_path(&self, name: &str) -> PathBuf {
        self.roots.overrides.join(name)
    }

    fn read_override(&self, name: &str) -> KeyFile {
        read_limited(&self.override_path(name))
            .map(|t| KeyFile::parse(&t))
            .unwrap_or_default()
    }

    /// One context list as the app has it: its metadata, then the global
    /// override, then the app's own.
    fn context(&self, id: &str, key: &str) -> Result<(Vec<String>, KeyFile), Error> {
        let dep = self
            .deployment(id)
            .ok_or_else(|| Error::new(ErrorKind::Refused, "no such app"))?;
        let meta = read_limited(&dep.join("metadata"))
            .map(|t| KeyFile::parse(&t))
            .unwrap_or_default();
        let base = apply(
            &meta.list("Context", key),
            &self.read_override("global").list("Context", key),
        );
        Ok((base, self.read_override(id)))
    }

    /// What the app may reach now.
    pub fn permissions(&self, id: &str) -> Result<Permissions, Error> {
        if !valid_app_id(id) {
            return Err(Error::new(ErrorKind::Refused, "not an app ID"));
        }
        let get = |key: &str| -> Result<Vec<String>, Error> {
            let (base, own) = self.context(id, key)?;
            Ok(apply(&base, &own.list("Context", key)))
        };
        let shared = get("shared")?;
        let devices = get("devices")?;
        let filesystems = get("filesystems")?;
        let has = |n: &str| filesystems.iter().any(|f| fs_name(f) == n);
        Ok(Permissions {
            files: if has("host") {
                Files::Everything
            } else if has("home") {
                Files::Home
            } else if has("xdg-download") {
                Files::Downloads
            } else {
                Files::None
            },
            devices: devices.iter().any(|d| d == "all"),
            network: shared.iter().any(|s| s == "network"),
        })
    }

    /// Makes `want` the state of `names` in list `key` for the app, by
    /// writing only what differs from its metadata and the global
    /// override.
    fn set_names(&self, id: &str, key: &str, want: &[(&str, bool)]) -> Result<(), Error> {
        if !valid_app_id(id) {
            return Err(Error::new(ErrorKind::Refused, "not an app ID"));
        }
        let (base, mut own) = self.context(id, key)?;
        let mut items = own.list("Context", key);
        for (name, allowed) in want {
            // Forget what the override said about this name (any suffix).
            items.retain(|i| fs_name(i.trim_start_matches('!')) != *name);
            let base_has = base.iter().any(|b| fs_name(b) == *name);
            if *allowed && !base_has {
                items.push((*name).to_string());
            } else if !*allowed && base_has {
                items.push(format!("!{name}"));
            }
        }
        own.set_list("Context", key, &items);
        self.write_override(id, &own)
    }

    fn write_override(&self, id: &str, kf: &KeyFile) -> Result<(), Error> {
        let path = self.override_path(id);
        if kf.is_empty() {
            return match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(Error::new(
                    ErrorKind::Refused,
                    format!("removing {id}: {e}"),
                )),
            };
        }
        let io = |e: std::io::Error| Error::new(ErrorKind::Refused, format!("writing {id}: {e}"));
        fs::create_dir_all(&self.roots.overrides).map_err(io)?;
        // A new file beside it, then renamed over it: never half a file.
        let tmp = self
            .roots
            .overrides
            .join(format!(".{id}.{}.tmp", std::process::id()));
        let _ = fs::remove_file(&tmp);
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&tmp)
            .map_err(io)?;
        f.write_all(kf.render().as_bytes())
            .and_then(|()| f.sync_all())
            .map_err(io)?;
        fs::rename(&tmp, &path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            io(e)
        })
    }

    pub fn set_network(&self, id: &str, on: bool) -> Result<(), Error> {
        self.set_names(id, "shared", &[("network", on)])
    }

    pub fn set_devices(&self, id: &str, on: bool) -> Result<(), Error> {
        self.set_names(id, "devices", &[("all", on)])
    }

    pub fn set_files(&self, id: &str, files: Files) -> Result<(), Error> {
        self.set_names(
            id,
            "filesystems",
            &[
                ("host", files == Files::Everything),
                ("home", files == Files::Home),
                ("xdg-download", files == Files::Downloads),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_files_keep_their_order_and_other_keys() {
        let text = "[Context]\nshared=network;ipc;\nsockets=x11;\n\n[Environment]\nFOO=bar\n";
        let mut kf = KeyFile::parse(text);
        assert_eq!(kf.list("Context", "shared"), ["network", "ipc"]);
        assert_eq!(kf.get("Environment", "FOO"), Some("bar"));
        assert_eq!(kf.render(), text);
        kf.set_list("Context", "shared", &["ipc".to_string()]);
        assert_eq!(kf.list("Context", "shared"), ["ipc"]);
        kf.set_list("Context", "shared", &[]);
        kf.set_list("Context", "sockets", &[]);
        // The empty group goes; the other stays.
        assert_eq!(kf.render(), "[Environment]\nFOO=bar\n");
    }

    #[test]
    fn overrides_add_and_remove() {
        let base = ["a".to_string(), "b".to_string()];
        assert_eq!(apply(&base, &["!a".into(), "c".into()]), ["b", "c"]);
        assert_eq!(apply(&base, &["b".into()]), ["a", "b"]);
    }

    /// A scratch folder of this test's own.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "settings-sys-flatpak-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_small_regular_files_are_read() {
        let dir = scratch("read");
        let small = dir.join("small");
        fs::write(&small, "[Context]\nshared=network;\n").unwrap();
        assert_eq!(
            read_limited(&small).as_deref(),
            Some("[Context]\nshared=network;\n")
        );
        // At the limit, and one over.
        let exact = dir.join("exact");
        fs::write(&exact, vec![b'a'; MAX_FILE as usize]).unwrap();
        assert_eq!(
            read_limited(&exact).map(|t| t.len()),
            Some(MAX_FILE as usize)
        );
        let over = dir.join("over");
        fs::write(&over, vec![b'a'; MAX_FILE as usize + 1]).unwrap();
        assert_eq!(read_limited(&over), None);
        // Not text.
        let binary = dir.join("binary");
        fs::write(&binary, [0xFF, 0xFE, 0x00, 0x80]).unwrap();
        assert_eq!(read_limited(&binary), None);
        // Missing, a folder, a device.
        assert_eq!(read_limited(&dir.join("missing")), None);
        assert_eq!(read_limited(&dir), None);
        assert_eq!(read_limited(Path::new("/dev/zero")), None);
        // A link to a regular file is the file (dotfiles folders).
        let link = dir.join("link");
        std::os::unix::fs::symlink(&small, &link).unwrap();
        assert!(read_limited(&link).is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pipe_in_place_of_a_file_is_not_waited_for() {
        let dir = scratch("pipe");
        let fifo = dir.join("global");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let started = std::time::Instant::now();
        assert_eq!(read_limited(&fifo), None);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        // The whole listing survives it: no overrides, nothing blocked.
        let roots = Roots {
            installations: vec![],
            overrides: dir.clone(),
        };
        let kf = Flatpak::new(roots).read_override("global");
        assert!(kf.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn overrides_are_written_whole_and_not_through_a_link() {
        let dir = scratch("write");
        let overrides = dir.join("overrides");
        fs::create_dir_all(&overrides).unwrap();
        // Something else's file that a link in the folder points at.
        let precious = dir.join("precious");
        fs::write(&precious, "keep me\n").unwrap();
        std::os::unix::fs::symlink(&precious, overrides.join("org.example.App")).unwrap();
        let fp = Flatpak::new(Roots {
            installations: vec![],
            overrides: overrides.clone(),
        });
        let mut kf = KeyFile::default();
        kf.set_list("Context", "shared", &["network".to_string()]);
        fp.write_override("org.example.App", &kf).unwrap();
        // The link is replaced by the new file; the file it pointed at is as it was.
        assert_eq!(fs::read_to_string(&precious).unwrap(), "keep me\n");
        let written = overrides.join("org.example.App");
        assert!(
            !fs::symlink_metadata(&written)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&written).unwrap(),
            "[Context]\nshared=network;\n"
        );
        // No half-written file is left beside it, and it is not group- or
        // world-writable.
        let names: Vec<_> = fs::read_dir(&overrides)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["org.example.App"]);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&written).unwrap().permissions().mode() & 0o022,
            0
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filesystem_names() {
        assert_eq!(fs_name("home:ro"), "home");
        assert_eq!(fs_name("xdg-download"), "xdg-download");
    }
}
