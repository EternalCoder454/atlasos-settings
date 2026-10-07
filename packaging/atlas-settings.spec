# Settings, the settings app of AtlasOS (replaces KDE System Settings).

# --define "_atlas_build_cache <dir>" (packaging/build-rpm.sh passes it when
# ATLAS_BUILD_CACHE is set) keeps the CMake build, Corrosion's cargo target
# with it, in <dir>, so a rebuild only compiles what changed.
%if 0%{?_atlas_build_cache:1}
%global _vpath_builddir %{_atlas_build_cache}/cmake
%endif

# No debuginfo subpackage: the Rust flags below keep symbols (debuginfo=2,
# strip=none) and the binary is shipped as built.
%global debug_package %{nil}

Name:           atlas-settings
Version:        0.1.1
Release:        1%{?dist}
Summary:        Settings, the settings app of AtlasOS
License:        MIT
URL:            https://github.com/EternalCoder454/atlasos-settings
Source0:        atlas-settings-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
# %%build_rustflags
BuildRequires:  rust-srpm-macros
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  cmake
BuildRequires:  ninja-build
BuildRequires:  corrosion
# Cargo fetches the atlas-framework crates from GitHub.
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
# QML modules qmlcachegen resolves at build time (not linked). atlas-ui comes
# from atlas-framework, which is in no repository: install its RPMs first
# (build-rpm.sh does, given ATLAS_LOCAL_RPMS).
BuildRequires:  kf6-kirigami-devel
BuildRequires:  atlas-ui >= 1.4.0

Requires:       kf6-kirigami
# Displays reads and sets the screens through libkscreen's backend for the
# running session; Sound talks to PipeWire's PulseAudio server.
Requires:       libkscreen
Requires:       pulseaudio-qt-qt6
Requires:       pipewire-pulseaudio
# Atlas.Ui, the shared look (atlas-framework); 1.4.0 for AtlasSidebar and
# AtlasSearchResults
Requires:       atlas-ui >= 1.4.0
Requires:       kf6-qqc2-desktop-style
Requires:       qt6-qtdeclarative
# the app icon and Breeze's icons are SVG
Requires:       qt6-qtsvg
# kcmshell6, for the settings that stay on Plasma's KCMs ("More Settings")
Requires:       kf6-kcmutils
# Updates opens Atlas Updater
Recommends:     atlas-updater

%description
Settings is where AtlasOS is set up: Wi-Fi and network, Bluetooth, displays,
sound, appearance, the desktop and dock, notifications, power, users, date and
time, language, keyboard, mouse and touchpad, printers, default apps, app
permissions, privacy, updates and system information. Settings it has no page
for open in Plasma's own settings modules.

%prep
%autosetup -n atlas-settings-%{version}

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
cache_rs="%{?_atlas_build_cache:--remap-path-prefix=%{_atlas_build_cache}=cache}"
cache_cc="%{?_atlas_build_cache:-ffile-prefix-map=%{_atlas_build_cache}=cache}"
export RUSTFLAGS="%{build_rustflags} $cache_rs --remap-path-prefix=$PWD=. --remap-path-prefix=$CARGO_HOME=cargo"
export HOST_CXXFLAGS="$cache_cc -ffile-prefix-map=$PWD=. -ffile-prefix-map=$CARGO_HOME=cargo"
export CFLAGS="%{build_cflags} $cache_cc -ffile-prefix-map=$PWD=."
export CXXFLAGS="%{build_cxxflags} $cache_cc -ffile-prefix-map=$PWD=."
export CARGO_PROFILE_RELEASE_STRIP=none
# (%%cmake honours _vpath_srcdir, not __cmake_source_dir)
%global _vpath_srcdir apps/atlas-settings
%cmake -G Ninja -DCMAKE_BUILD_TYPE=Release
%cmake_build

%install
%cmake_install
# AtlasOS needs its settings app: dnf refuses to remove it.
install -Dpm0644 apps/atlas-settings/data/dnf/protected.d/atlas-settings.conf \
    %{buildroot}%{_sysconfdir}/dnf/protected.d/atlas-settings.conf

%check
# No path into the build tree (checked as well as set: see %%build).
# grep: 0 = found, 1 = not found, anything else (no binary) fails too.
for path in "%{_builddir}" %{?_atlas_build_cache:"%{_atlas_build_cache}"}; do
    rc=0
    grep -qF "$path" %{buildroot}%{_bindir}/atlas-settings || rc=$?
    if [ "$rc" != 1 ]; then
        echo "atlas-settings holds the build path $path (grep status $rc)" >&2
        exit 1
    fi
done
# The C++ logic's tests (screens through libkscreen's Fake backend, Night
# Light in a throwaway kwinrc; the sound test needs a sound server and skips).
QT_QPA_PLATFORM=offscreen %ctest
desktop-file-validate %{buildroot}%{_datadir}/applications/net.eterneon.atlas.settings.desktop
appstream-util validate-relax --nonet \
    %{buildroot}%{_datadir}/metainfo/net.eterneon.atlas.settings.metainfo.xml

%files
%license LICENSE
%{_bindir}/atlas-settings
%{_datadir}/applications/net.eterneon.atlas.settings.desktop
%{_datadir}/metainfo/net.eterneon.atlas.settings.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/net.eterneon.atlas.settings.svg
%config(noreplace) %{_sysconfdir}/dnf/protected.d/atlas-settings.conf

%changelog
* Tue Oct 06 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.1.1-1
- A new icon of its own (a steel gear), without the purple tile.
- atlas-framework 1.6.0.

* Mon Oct 05 2026 EternalHell <77252745+EternalCoder454@users.noreply.github.com> - 0.1.0-1
- First package
