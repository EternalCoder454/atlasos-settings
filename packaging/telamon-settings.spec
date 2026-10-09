# Settings, the settings app of Telamon OS (replaces KDE System Settings).

# --define "_telamon_build_cache <dir>" (packaging/build-rpm.sh passes it when
# TELAMON_BUILD_CACHE is set) keeps the CMake build, Corrosion's cargo target
# with it, in <dir>, so a rebuild only compiles what changed.
%if 0%{?_telamon_build_cache:1}
%global _vpath_builddir %{_telamon_build_cache}/cmake
%endif

# No debuginfo subpackage: the Rust flags below keep symbols (debuginfo=2,
# strip=none) and the binary is shipped as built.
%global debug_package %{nil}

Name:           telamon-settings
Version:        0.5.0
Release:        1%{?dist}
Summary:        Settings, the settings app of Telamon OS
License:        MIT
URL:            https://github.com/EternalCoder454/atlasos-settings
Source0:        telamon-settings-%{version}.tar.gz

# It was atlas-settings (Atlas Settings). The image upgrades in place.
Obsoletes:      atlas-settings < 0.3.0
Provides:       atlas-settings = %{version}-%{release}

BuildRequires:  cargo
BuildRequires:  rust
# %%build_rustflags
BuildRequires:  rust-srpm-macros
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  cmake
BuildRequires:  ninja-build
BuildRequires:  corrosion
# Cargo fetches the atlas-framework and atlasos-updater crates from GitHub.
BuildRequires:  git-core
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib
BuildRequires:  cmake(Qt6Core)
BuildRequires:  cmake(Qt6Gui)
BuildRequires:  cmake(Qt6Qml)
BuildRequires:  cmake(Qt6Quick)
BuildRequires:  cmake(Qt6QuickControls2)
BuildRequires:  cmake(Qt6Widgets)
BuildRequires:  cmake(Qt6QmlTools)
BuildRequires:  qt6-qtbase-devel
BuildRequires:  cmake(KF6CoreAddons)
BuildRequires:  cmake(KF6DBusAddons)
BuildRequires:  cmake(KF6WindowSystem)
BuildRequires:  cmake(KF6KIO)
BuildRequires:  cmake(KF6GuiAddons)
BuildRequires:  cmake(KF6Config)
# Displays: the screens, through libkscreen (Plasma's); Sound: PulseAudio's
# API on PipeWire, through PulseAudioQt (Plasma's).
BuildRequires:  cmake(KF6Screen)
BuildRequires:  cmake(KF6PulseAudioQt)
BuildRequires:  cmake(KF6Service)
# crypt(3), for the password hash AccountsService takes
BuildRequires:  libxcrypt-devel
# The Updates page's Flatpak updates (telamon-framework-flatpak, through
# telamon-updater-core, link libflatpak).
BuildRequires:  pkgconfig(flatpak)
BuildRequires:  pkgconfig(glib-2.0)
BuildRequires:  pkgconfig(gio-2.0)
# QML modules qmlcachegen resolves at build time (not linked). telamon-ui comes
# from atlas-framework, which is in no repository: install its RPMs first
# (build-rpm.sh does, given TELAMON_LOCAL_RPMS).
BuildRequires:  kf6-kirigami-devel
BuildRequires:  telamon-ui >= 2.0.0

Requires:       kf6-kirigami
# Displays reads and sets the screens through libkscreen's backend for the
# running session; Sound talks to PipeWire's PulseAudio server.
Requires:       libkscreen
Requires:       pulseaudio-qt-qt6
Requires:       pipewire-pulseaudio
# Telamon.Ui, the shared look (atlas-framework)
Requires:       telamon-ui >= 2.0.0
Requires:       kf6-qqc2-desktop-style
Requires:       qt6-qtdeclarative
# the app icon and Breeze's icons are SVG
Requires:       qt6-qtsvg
# kcmshell6, for the settings that stay on Plasma's KCMs ("More Settings")
Requires:       kf6-kcmutils
# The Updates page talks to the system helper (telamon-system-helper, which
# the Telamon OS image always has) and its background part, telamon-updater,
# checks for updates and shows the glow while one installs.
Recommends:     telamon-system-helper
Recommends:     telamon-updater

%description
Settings is where Telamon OS is set up: Wi-Fi and network, Bluetooth, displays,
sound, appearance, the desktop and dock, notifications, power, users, date and
time, language, keyboard, mouse and touchpad, printers, default apps, app
permissions, privacy, updates and system information. Settings it has no page
for open in Plasma's own settings modules.

%package systemsettings
Summary:        Telamon Settings in place of KDE System Settings
Requires:       %{name}%{?_isa} = %{version}-%{release}
# kcmshell6, which the systemsettings command starts for the settings modules
# Settings has no page for
Requires:       kf6-kcmutils
# It takes the place of Fedora's plasma-systemsettings (the same /usr/bin/systemsettings
# and systemsettings.desktop, which Plasma's KCMLauncher, tray applets and
# other packages' scripts start). Other packages require the name
# (plasma-desktop, colord-kde and kcm-plasmalogin, by name and with the
# architecture), so this provides it: at a version no Requires: from KDE
# can ask more of, and the Obsoletes covers every version Fedora ships, so a
# later plasma-systemsettings never comes back beside this.
Obsoletes:      plasma-systemsettings < 100
Provides:       plasma-systemsettings = 99
Provides:       plasma-systemsettings%{?_isa} = 99

%description systemsettings
Replaces KDE's System Settings with Telamon Settings: /usr/bin/systemsettings
opens Settings for a settings module it has a page for (systemsettings
kcm_kscreen) and Plasma's kcmshell6 for any other (systemsettings kcm_trash),
and the hidden systemsettings.desktop and kdesystemsettings.desktop start
Settings, so Plasma's KCMLauncher, the tray applets and old pins keep working.

%prep
%autosetup -n telamon-settings-%{version}

%build
# NETWORK: cargo (Corrosion runs it with --locked) fetches crates.io and the
# pinned atlas-framework crates during %%build. That works in podman and with
# `rpmbuild` on a networked machine, not in an offline mock/Koji build.
# CARGO_HOME from the environment keeps a crate cache between builds
# (CLAUDE.md mounts one); otherwise a fresh one in the build dir.
export CARGO_HOME=${CARGO_HOME:-%{_builddir}/cargo-home}
# Fedora's Rust flags (hardening, build-id, ...), also used by Corrosion's
# cargo. The remaps keep build paths (panic locations, assert file names) out
# of the package, as atlas-framework's DESIGN.md asks of apps using its crates.
# HOST_CXXFLAGS reaches only the C++ that cargo's build scripts compile (cc-rs
# reads HOST_ when not cross-compiling; CMake ignores it). CFLAGS and CXXFLAGS
# are Fedora's plus the same remap for the C++ CMake builds, so that two
# builds of one commit give the same build ID. These flags split on spaces,
# so _topdir must have none (build-rpm.sh's hasn't).
# A build cache (above) holds $PWD and the CMake build: its path is remapped
# too. The last matching remap wins, and $PWD is inside the cache: the
# cache's comes first.
cache_rs="%{?_telamon_build_cache:--remap-path-prefix=%{_telamon_build_cache}=cache}"
cache_cc="%{?_telamon_build_cache:-ffile-prefix-map=%{_telamon_build_cache}=cache}"
export RUSTFLAGS="%{build_rustflags} $cache_rs --remap-path-prefix=$PWD=. --remap-path-prefix=$CARGO_HOME=cargo"
export HOST_CXXFLAGS="$cache_cc -ffile-prefix-map=$PWD=. -ffile-prefix-map=$CARGO_HOME=cargo"
export CFLAGS="%{build_cflags} $cache_cc -ffile-prefix-map=$PWD=."
export CXXFLAGS="%{build_cxxflags} $cache_cc -ffile-prefix-map=$PWD=."
export CARGO_PROFILE_RELEASE_STRIP=none
# (%%cmake honours _vpath_srcdir, not __cmake_source_dir)
%global _vpath_srcdir apps/telamon-settings
%cmake -G Ninja -DCMAKE_BUILD_TYPE=Release
%cmake_build

%install
%cmake_install
# Telamon OS needs its settings app: dnf refuses to remove it.
install -Dpm0644 apps/telamon-settings/data/dnf/protected.d/telamon-settings.conf \
    %{buildroot}%{_sysconfdir}/dnf/protected.d/telamon-settings.conf
# Programs and pins that still use the old names (the image's pinned apps, the
# Launcher, scripts) keep working in this release: atlas-settings starts the
# same program, and the hidden net.eterneon.atlas.settings.desktop starts it
# for a pin or a launcher that looks the old desktop file up.
ln -s telamon-settings %{buildroot}%{_bindir}/atlas-settings
install -Dpm0644 apps/telamon-settings/data/net.eterneon.atlas.settings.desktop \
    %{buildroot}%{_datadir}/applications/net.eterneon.atlas.settings.desktop

%check
# The page registry's and the systemsettings command's own tests (no Qt): a
# separate debug build of those two crates, beside the release build above.
export CARGO_HOME=${CARGO_HOME:-%{_builddir}/cargo-home}
CARGO_TARGET_DIR=%{_builddir}/cargo-test-target cargo test --locked \
    -p settings-registry -p systemsettings-shim
# No path into the build tree (checked as well as set: see %%build).
# grep: 0 = found, 1 = not found, anything else (no binary) fails too.
for path in "%{_builddir}" %{?_telamon_build_cache:"%{_telamon_build_cache}"}; do
    rc=0
    grep -qF "$path" %{buildroot}%{_bindir}/telamon-settings || rc=$?
    if [ "$rc" != 1 ]; then
        echo "telamon-settings holds the build path $path (grep status $rc)" >&2
        exit 1
    fi
done
# The C++ logic's tests (screens through libkscreen's Fake backend, Night
# Light in a throwaway kwinrc; the sound test needs a sound server and skips).
QT_QPA_PLATFORM=offscreen %ctest
desktop-file-validate %{buildroot}%{_datadir}/applications/net.eterneon.telamon.settings.desktop \
    %{buildroot}%{_datadir}/applications/net.eterneon.atlas.settings.desktop \
    %{buildroot}%{_datadir}/applications/systemsettings.desktop \
    %{buildroot}%{_datadir}/applications/kdesystemsettings.desktop \
    %{buildroot}%{_datadir}/kglobalaccel/net.eterneon.telamon.settings.desktop
# Settings is in the menu and the Launcher finds it; the stand-ins for System
# Settings are hidden.
if grep -q '^NoDisplay=true' %{buildroot}%{_datadir}/applications/net.eterneon.telamon.settings.desktop; then
    echo "net.eterneon.telamon.settings.desktop is still hidden" >&2
    exit 1
fi
# System Settings' Meta+I goes on with Settings (the copy Plasma reads is the same file).
grep -qx 'X-KDE-Shortcuts=Tools,Meta+I' %{buildroot}%{_datadir}/kglobalaccel/net.eterneon.telamon.settings.desktop
for f in systemsettings kdesystemsettings; do
    grep -qx 'NoDisplay=true' %{buildroot}%{_datadir}/applications/$f.desktop
    grep -qx 'Exec=telamon-settings' %{buildroot}%{_datadir}/applications/$f.desktop
done
# The Launcher's search index and the command that stands in for System Settings.
grep -q '^{"version":1,"app":"net.eterneon.telamon.settings"' %{buildroot}%{_datadir}/telamon-settings/search-index.json
grep -q '"link":"displays/night-light"' %{buildroot}%{_datadir}/telamon-settings/search-index.json
%{buildroot}%{_bindir}/systemsettings --version | grep -q '^systemsettings %{version} '
# (`!` would not stop the script under set -e.)
rc=0
%{buildroot}%{_bindir}/systemsettings '../../bin/sh' 2>/dev/null || rc=$?
if [ "$rc" != 2 ]; then
    echo "systemsettings accepted a path as a module (status $rc)" >&2
    exit 1
fi
appstream-util validate-relax --nonet \
    %{buildroot}%{_datadir}/metainfo/net.eterneon.telamon.settings.metainfo.xml
# The programs carry the hardening the build flags give them (position
# independent, full RELRO and BIND_NOW, no executable stack, no RPATH, stack
# protectors in the C++), and the program has none of the tests' hooks: readelf
# says, not the flags we meant.
scripts/check-hardening.sh --cxx %{buildroot}%{_bindir}/telamon-settings
scripts/check-hardening.sh %{buildroot}%{_bindir}/systemsettings

%files
%license LICENSE
%{_bindir}/telamon-settings
%{_bindir}/atlas-settings
%{_datadir}/applications/net.eterneon.telamon.settings.desktop
%{_datadir}/applications/net.eterneon.atlas.settings.desktop
%{_datadir}/kglobalaccel/net.eterneon.telamon.settings.desktop
%{_datadir}/dbus-1/services/net.eterneon.telamon.settings.service
%dir %{_datadir}/telamon-settings
%{_datadir}/telamon-settings/search-index.json
%{_datadir}/metainfo/net.eterneon.telamon.settings.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/net.eterneon.telamon.settings.svg
%config(noreplace) %{_sysconfdir}/dnf/protected.d/telamon-settings.conf

%files systemsettings
%{_bindir}/systemsettings
%{_datadir}/applications/systemsettings.desktop
%{_datadir}/applications/kdesystemsettings.desktop

%changelog
* Thu Oct 08 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.5.0-1
- Security release; docs/SECURITY.md is the threat model.
- The old D-Bus name (net.eterneon.atlas.settings) split the text of an
  ActivateAction call into launch arguments, so a caller could slip --kcm or
  --search in through it; it now hands the text to the same link check as the
  new name, which accepts a page and a setting and nothing else.
- Accounts: a picture is looked at before AccountsService is asked to copy it
  (a regular file of at most 1 MB that is a PNG, JPEG, WebP or GIF by its
  contents, not an SVG, never a link to something else), and says why when
  it won't do. Passwords are hashed with yescrypt, as Fedora stores them
  (SHA-512 with 100,000 rounds where libcrypt has none), and the key is
  overwritten after; passwords above libcrypt's 511 byte limit are refused
  up front instead of after the account was made. A new password has at
  least 8 characters, and a failed Add User leaves no account behind.
- Programs Settings starts are an allowlist (kcmshell6, the plasma-apply
  tools, gsettings, Telamon Store), and names handed to them must start with a
  letter or digit. Names checked by a regular expression no longer let a
  trailing newline through.
- The test hook that swaps the real screens for fake ones is no longer part of
  the program (only the tests have it).
- Support and crash report links open only on an allowlist of https sites;
  the Flatpak override files are read without waiting on a pipe or trusting a
  size checked earlier; wallpaper and theme metadata and autostart entries are
  read only when they are small regular files (a link to /dev/zero froze the
  page); KCM plugin paths are taken only in Plasma's form.
- The package build checks its programs' hardening (PIE, full RELRO, no
  executable stack, stack protectors) and fails without it; CI runs
  cargo-deny and cargo-audit, and property tests of the parsers.

* Thu Oct 08 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.7-1
- Fix: the app used about 8% of a core with its window idle. The icon layers added in the last release
  were redrawn on every frame with Qt Quick's software renderer; a layer is live now only for a moment
  after its icon changes (source, colour, size, state or theme).

* Thu Oct 08 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.6-1
- Fix: icons drawn over dialogs, popups and menus. With Qt Quick's software renderer a Kirigami.Icon was
  painted again over what sat in front of it whenever a repaint touched a part of it; the app's icons are
  layers on that renderer now.

* Thu Oct 08 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.5-1
- Updates: a newer version published after one was downloaded is offered, not
  hidden. With an update waiting for a restart, the page kept saying "Restart to
  finish updating" for the old version and had no Check for Updates or Download
  Update, so the restart landed on the old version and the newer one needed a
  second restart. Now Check for Updates stays while an update waits, and when
  a newer version is available the page says so, "Download Update" comes first
  (it replaces the downloaded version, one restart starts the newest), and the
  restart button names the version it starts. The download says it replaces
  the downloaded one, and its release notes are the newest version's.
* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.4-1
- Settings pinned in the dock opens on its pinned icon again, not as a second
  icon beside it. The hidden systemsettings.desktop and kdesystemsettings.desktop
  named Settings' window class (StartupWMClass), so Plasma matched the window
  to them instead of to net.eterneon.telamon.settings.desktop, the pin. They
  no longer name it: the window already carries its desktop file's name.

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.3-1
- Crash Reports: every report is a closed row that opens to its details, and
  the sheet scrolls evenly (60 px a wheel notch, no stack trace taking the
  wheel); the sheets with long lists (App Permissions, Startup Apps, What's
  New, Update History) scroll the same way.
- Accounts: the camera on the picture starts the camera app (Plasma Camera,
  else Kamoso or Snapshot) and offers to choose the picture it takes; with no
  camera app it opens Telamon Store at Snapshot.
- Updates: the firmware part shows this computer's own firmware (BIOS/UEFI)
  version and date, and never says "up to date" about firmware fwupd has no
  release for: a maker that publishes nothing to fwupd gets a note and a button
  to its support site (ASRock, ASUS, MSI, Gigabyte, Dell, Lenovo, HP,
  Framework; else a web search).

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.2-1
- Light or Dark now changes the colour scheme. With an accent set (always, as
  Violet is one) the tool Settings ran ignored the scheme and only re-tinted
  the current one, so the scheme was never written to kdeglobals. The scheme is
  applied first and the accent after it, and a scheme that only the Global
  Theme's defaults name is written to kdeglobals, where Flatpak apps and
  kvantum-sync's first read of a session find it.

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.1-1
- Appearance shows a live preview of the desktop: your wallpaper, the top
  bar, a window in the chosen Light/Dark and accent, and the dock. Hovering a
  card or a colour previews it before you pick it.

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.4.0-1
- Settings replaces KDE's System Settings. It is in the menu and the Launcher
  now (the desktop file is no longer hidden), and the Launcher finds every
  page and setting (/usr/share/telamon-settings/search-index.json, written
  from the page registry) and opens it, also when Settings is not running
  yet (ActivateAction "open", "open-app"; D-Bus activation).
- New package telamon-settings-systemsettings, which replaces
  plasma-systemsettings: /usr/bin/systemsettings opens Settings for a
  settings module it has a page for and kcmshell6 for any other, and the
  hidden systemsettings.desktop and kdesystemsettings.desktop start
  Settings, so Plasma's KCMLauncher, the tray applets and old pins keep
  working. Settings takes over System Settings' global shortcut, Meta+I.

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.3.1-1
- The screen edge glow is asked for only while the OS image changes (an
  update, a channel switch, a rollback), not for app or firmware updates.

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.3.0-1
- Renamed to Telamon Settings (telamon-settings, net.eterneon.telamon.settings),
  on Telamon.Ui 2.0.0. The old package name is obsoleted and provided;
  atlas-settings, the old desktop file and the old D-Bus name keep working
  for this release. The settings file moves from atlas-settingsrc to
  telamon-settingsrc by itself.

* Wed Oct 07 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.2.0-1
- Fifteen pages instead of System Settings' dozens: Home, Network,
  Bluetooth & Devices, Displays, Sound, Keyboard & Mouse, Appearance,
  Notifications, Apps, Privacy & Security, Users, Power & Battery,
  Accessibility, Time & Language and System. Each shows the settings most
  people change; the rest fold under Advanced, and search finds them all.
- Every page is Settings' own, through the system's services
  (NetworkManager, BlueZ, libkscreen, PipeWire, AccountsService, firewalld,
  power-profiles-daemon, timedated and others); rarely used Plasma pages
  open from rows that say so, or from Other Plasma Settings.
- Still hidden from the menu until it replaces System Settings.

* Tue Oct 06 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.1.1-1
- A new icon of its own (a steel gear), without the purple tile.
- atlas-framework 1.6.0.

* Mon Oct 05 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.1.0-1
- First package
