//! What System shows about the device, read from files without privileges:
//! the OS release (`/usr/lib/os-release`), the processor (`/proc/cpuinfo`),
//! the memory (`/proc/meminfo`) and the graphics (`/sys/class/drm` and the
//! PCI ID database). Every file is untrusted: reads are capped and values
//! made safe to show. Reads block on the disk, so call them off the GUI
//! thread.

use crate::error::clean;
use std::io::Read;
use std::path::Path;

/// Most bytes read from one file under /proc, /sys or /usr/lib.
const MAX_FILE: u64 = 256 * 1024;
/// Most bytes read from the PCI ID database (about 1.5 MB today).
const MAX_PCI_IDS: u64 = 8 * 1024 * 1024;
/// The longest value shown.
const MAX_VALUE: usize = 120;
/// Most graphics devices listed.
const MAX_GPUS: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Info {
    /// "AtlasOS 44" or the like; empty when unknown.
    pub os: String,
    /// The image's version or build, when the release file says.
    pub os_build: String,
    /// "AMD Ryzen 7 7840U (16 threads)"; empty when unknown.
    pub cpu: String,
    /// Total memory in bytes; 0 when unknown.
    pub memory: u64,
    /// The graphics devices' names, joined with ", ".
    pub graphics: String,
}

fn read_capped(path: &Path, max: u64) -> String {
    let mut buf = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(max).read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Everything System shows, read now.
pub fn read() -> Info {
    let release = {
        let usr = read_capped(Path::new("/usr/lib/os-release"), MAX_FILE);
        if usr.is_empty() {
            read_capped(Path::new("/etc/os-release"), MAX_FILE)
        } else {
            usr
        }
    };
    let (os, os_build) = os_of(&release);
    let pci_ids = || read_capped(Path::new("/usr/share/hwdata/pci.ids"), MAX_PCI_IDS);
    Info {
        os,
        os_build,
        cpu: cpu_of(&read_capped(Path::new("/proc/cpuinfo"), MAX_FILE * 4)),
        memory: memory_of(&read_capped(Path::new("/proc/meminfo"), MAX_FILE)),
        graphics: graphics(Path::new("/sys/class/drm"), pci_ids),
    }
}

/// One `KEY=value` from an os-release file, unquoted.
fn release_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|l| {
        let v = l.strip_prefix(key)?.strip_prefix('=')?.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(v);
        (!v.is_empty()).then_some(v)
    })
}

/// The OS's name and version, and its image version or build ID.
pub fn os_of(release: &str) -> (String, String) {
    let name = release_value(release, "PRETTY_NAME")
        .map(str::to_string)
        .or_else(|| {
            let n = release_value(release, "NAME")?;
            Some(match release_value(release, "VERSION_ID") {
                Some(v) => format!("{n} {v}"),
                None => n.to_string(),
            })
        })
        .unwrap_or_default();
    let build = release_value(release, "IMAGE_VERSION")
        .or_else(|| release_value(release, "OSTREE_VERSION"))
        .or_else(|| release_value(release, "BUILD_ID"))
        .unwrap_or_default();
    (clean(&name, MAX_VALUE), clean(build, MAX_VALUE))
}

/// The processor's model and how many threads it runs.
pub fn cpu_of(cpuinfo: &str) -> String {
    let field = |line: &str, key: &str| -> Option<String> {
        let (k, v) = line.split_once(':')?;
        (k.trim() == key).then(|| v.split_whitespace().collect::<Vec<_>>().join(" "))
    };
    let model = cpuinfo
        .lines()
        .find_map(|l| field(l, "model name"))
        .or_else(|| cpuinfo.lines().find_map(|l| field(l, "Model")))
        .or_else(|| cpuinfo.lines().find_map(|l| field(l, "Hardware")))
        .unwrap_or_default();
    let threads = cpuinfo
        .lines()
        .filter(|l| field(l, "processor").is_some())
        .count();
    let model = clean(&model, MAX_VALUE);
    match (model.is_empty(), threads) {
        (true, _) => String::new(),
        (false, 0 | 1) => model,
        (false, n) => format!("{model} ({n} threads)"),
    }
}

/// `MemTotal` in bytes.
pub fn memory_of(meminfo: &str) -> u64 {
    meminfo
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.trim().strip_suffix("kB"))
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(0, |kb| kb.saturating_mul(1024))
}

/// A PCI ID as written in sysfs (`0x10de`), as a number.
fn pci_id(text: &str) -> Option<u16> {
    u16::from_str_radix(text.trim().strip_prefix("0x")?, 16).ok()
}

/// The vendor's and the device's names from the PCI ID database (its
/// `vvvv  Vendor` lines, each followed by `\tdddd  Device` lines).
pub fn pci_name(ids: &str, vendor: u16, device: u16) -> Option<(String, Option<String>)> {
    let v = format!("{vendor:04x}  ");
    let d = format!("\t{device:04x}  ");
    let mut lines = ids.lines().skip_while(|l| !l.starts_with(&v));
    let vendor_name = lines.next()?[v.len()..].to_string();
    let device_name = lines
        .take_while(|l| l.starts_with('\t') || l.starts_with('#'))
        .find_map(|l| l.strip_prefix(&d).map(str::to_string));
    Some((vendor_name, device_name))
}

/// A short vendor name for the ones whose database names are long.
fn short_vendor(vendor: u16, name: &str) -> String {
    match vendor {
        0x8086 => "Intel".into(),
        0x1002 => "AMD".into(),
        0x10de => "NVIDIA".into(),
        0x1af4 => "Virtio".into(),
        0x15ad => "VMware".into(),
        0x1234 => "QEMU".into(),
        _ => name.to_string(),
    }
}

/// The graphics devices under `drm` (`card0`, `card1`, ...), named from the
/// PCI ID database `ids` (read only when there is a device to name).
pub fn graphics(drm: &Path, ids: impl FnOnce() -> String) -> String {
    let mut cards: Vec<(u16, u16)> = Vec::new();
    let Ok(entries) = std::fs::read_dir(drm) else {
        return String::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.strip_prefix("card")
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        })
        .take(64)
        .collect();
    names.sort();
    for n in names {
        let dev = drm.join(&n).join("device");
        let (Some(v), Some(d)) = (
            pci_id(&read_capped(&dev.join("vendor"), 64)),
            pci_id(&read_capped(&dev.join("device"), 64)),
        ) else {
            continue;
        };
        if !cards.contains(&(v, d)) {
            cards.push((v, d));
        }
        if cards.len() == MAX_GPUS {
            break;
        }
    }
    if cards.is_empty() {
        return String::new();
    }
    let ids = ids();
    let named: Vec<String> = cards
        .iter()
        .map(|&(v, d)| match pci_name(&ids, v, d) {
            Some((vn, Some(dn))) => {
                // "[Radeon 780M]" style marketing names, when the database has them.
                let dn = match (dn.find('['), dn.rfind(']')) {
                    (Some(a), Some(b)) if a < b => dn[a + 1..b].to_string(),
                    _ => dn,
                };
                format!("{} {dn}", short_vendor(v, &vn))
            }
            Some((vn, None)) => short_vendor(v, &vn),
            None => format!("{v:04x}:{d:04x}"),
        })
        .map(|s| clean(&s, MAX_VALUE))
        .collect();
    named.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release() {
        let r = "NAME=\"AtlasOS\"\nVERSION_ID=44\nPRETTY_NAME=\"AtlasOS 44 (Kinoite)\"\nIMAGE_VERSION='44.20261006'\n";
        assert_eq!(
            os_of(r),
            ("AtlasOS 44 (Kinoite)".into(), "44.20261006".into())
        );
        assert_eq!(
            os_of("NAME=Fedora\nVERSION_ID=44\nBUILD_ID=x\n"),
            ("Fedora 44".into(), "x".into())
        );
        assert_eq!(os_of(""), (String::new(), String::new()));
        let (n, _) = os_of(&format!("PRETTY_NAME=\"{}\u{1b}[2J\"", "A".repeat(500)));
        assert!(n.chars().count() <= MAX_VALUE + 1 && !n.contains('\u{1b}'));
    }

    #[test]
    fn cpu() {
        let c = "processor\t: 0\nmodel name\t: AMD Ryzen 7  7840U w/ Radeon  780M Graphics\nprocessor\t: 1\nmodel name\t: AMD Ryzen 7  7840U w/ Radeon  780M Graphics\n";
        assert_eq!(
            cpu_of(c),
            "AMD Ryzen 7 7840U w/ Radeon 780M Graphics (2 threads)"
        );
        assert_eq!(cpu_of("processor : 0\nHardware : BCM2835\n"), "BCM2835");
        assert_eq!(cpu_of(""), "");
    }

    #[test]
    fn memory() {
        assert_eq!(
            memory_of("MemTotal:       32617752 kB\nMemFree: 1 kB\n"),
            32617752 * 1024
        );
        assert_eq!(memory_of("MemTotal: lots\n"), 0);
        assert_eq!(memory_of(""), 0);
    }

    #[test]
    fn pci_names() {
        let ids = "# comment\n1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t15bf  Phoenix1 [Radeon 780M]\n\t\t1234 5678  Sub\n10de  NVIDIA Corporation\n\t2684  AD102 [GeForce RTX 4090]\n";
        assert_eq!(
            pci_name(ids, 0x1002, 0x15bf),
            Some((
                "Advanced Micro Devices, Inc. [AMD/ATI]".into(),
                Some("Phoenix1 [Radeon 780M]".into())
            ))
        );
        // A device of another vendor isn't found under this one.
        assert_eq!(
            pci_name(ids, 0x1002, 0x2684),
            Some(("Advanced Micro Devices, Inc. [AMD/ATI]".into(), None))
        );
        assert_eq!(pci_name(ids, 0x8086, 0x1), None);
        assert_eq!(pci_id("0x10de\n"), Some(0x10de));
        assert_eq!(pci_id("10de"), None);
    }

    #[test]
    fn graphics_from_sysfs() {
        let dir = std::env::temp_dir().join(format!("settings-sys-drm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (card, v, d) in [("card0", "0x1002", "0x15bf"), ("card1", "0x10de", "0x2684")] {
            let dev = dir.join(card).join("device");
            std::fs::create_dir_all(&dev).unwrap();
            std::fs::write(dev.join("vendor"), v).unwrap();
            std::fs::write(dev.join("device"), d).unwrap();
        }
        std::fs::create_dir_all(dir.join("card0-eDP-1")).unwrap();
        let ids = "1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t15bf  Phoenix1 [Radeon 780M]\n10de  NVIDIA Corporation\n";
        assert_eq!(
            graphics(&dir, || ids.to_string()),
            "AMD Radeon 780M, NVIDIA"
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(graphics(Path::new("/nonexistent"), String::new), "");
    }
}
