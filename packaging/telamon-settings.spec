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
Version:        0.3.0
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
    %{buildroot}%{_datadir}/applications/net.eterneon.atlas.settings.desktop
appstream-util validate-relax --nonet \
    %{buildroot}%{_datadir}/metainfo/net.eterneon.telamon.settings.metainfo.xml

%files
%license LICENSE
%{_bindir}/telamon-settings
%{_bindir}/atlas-settings
%{_datadir}/applications/net.eterneon.telamon.settings.desktop
%{_datadir}/applications/net.eterneon.atlas.settings.desktop
%{_datadir}/metainfo/net.eterneon.telamon.settings.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/net.eterneon.telamon.settings.svg
%config(noreplace) %{_sysconfdir}/dnf/protected.d/telamon-settings.conf

%changelog
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
