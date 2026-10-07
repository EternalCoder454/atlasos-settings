#!/bin/bash
# Build the Settings RPM inside a fedora:44 container, as root.
#   packaging/build-rpm.sh <out dir> [rpmbuild options]
# The binary RPM (no source, no debuginfo) is copied to <out dir>.
# Cargo needs network access.
# TELAMON_LOCAL_RPMS=<dir> installs the RPMs in <dir> first: atlas-framework's
# (telamon-ui), which the app builds against and no repository has.
set -euo pipefail

# The old names of these variables still work.
TELAMON_LOCAL_RPMS=${TELAMON_LOCAL_RPMS:-${ATLAS_LOCAL_RPMS:-}}
TELAMON_BUILD_CACHE=${TELAMON_BUILD_CACHE:-${ATLAS_BUILD_CACHE:-}}

main() {
    out=${1:?usage: build-rpm.sh <out dir> [rpmbuild options]}
    shift

    here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
    src=$(dirname "$here")
    spec=$here/telamon-settings.spec
    version=$(awk '/^Version:/ {print $2; exit}' "$spec")

    dnf -y install rpm-build dnf5-plugins tar gzip >&2
    if [ -n "${TELAMON_LOCAL_RPMS:-}" ]; then
        # Telamon.Ui and its fonts, not the gallery. dnf brings their
        # dependencies; rpm then puts these exact files in place even when a
        # build of the same version is installed already.
        local_rpms=("$TELAMON_LOCAL_RPMS"/telamon-ui-[0-9]*.rpm "$TELAMON_LOCAL_RPMS"/telamon-symbols-fonts-[0-9]*.rpm)
        for f in "${local_rpms[@]}"; do
            if [ ! -f "$f" ]; then
                echo "TELAMON_LOCAL_RPMS ($TELAMON_LOCAL_RPMS) has no $(basename "$f")" >&2; exit 1
            fi
        done
        dnf -y install "${local_rpms[@]}" >&2
        rpm -U --replacepkgs --replacefiles "${local_rpms[@]}" >&2
    fi
    dnf -y builddep "$spec" >&2

    # TELAMON_BUILD_CACHE=<dir> keeps the CMake build (Corrosion's cargo target
    # in it) in <dir>, at a fixed path, so a rebuild only compiles what
    # changed (CI caches it). It starts over when the version or the
    # toolchain changes. Without it, every build is a clean one in a temp dir.
    rpmopts=()
    cache=${TELAMON_BUILD_CACHE:-}
    if [ -n "$cache" ]; then
        mkdir -p "$cache"
        cache=$(cd "$cache" && pwd -P)
        case $cache/ in
            "$(cd "$src" && pwd -P)"/*) echo "TELAMON_BUILD_CACHE must be outside the source tree" >&2; exit 1 ;;
        esac
        # It goes into the build flags (split on spaces) and the spec's shell.
        if [[ ! $cache =~ ^[A-Za-z0-9_./-]+$ ]]; then
            echo "TELAMON_BUILD_CACHE may hold only letters, digits and _ . / -" >&2; exit 1
        fi
        # Only a directory this script made (or an empty one): it deletes
        # rpmbuild/ and cmake/ in it.
        if [ ! -e "$cache/.telamon-settings-build-cache" ] && [ -n "$(ls -A "$cache")" ]; then
            echo "TELAMON_BUILD_CACHE ($cache) is not empty and is not a Settings build cache" >&2; exit 1
        fi
        touch "$cache/.telamon-settings-build-cache"
        top=$cache/rpmbuild
        rm -rf "$top"
        # With telamon-ui's build time: a rebuild of the same version changes
        # what the QML is compiled against.
        if ! toolchain=$(rpm -q --qf '%{NEVRA}-%{BUILDTIME}\n' rust cargo corrosion gcc-c++ cmake \
            qt6-qtbase-devel qt6-qtdeclarative-devel kf6-kirigami-devel telamon-ui); then
            echo "a build dependency is missing: $toolchain" >&2; exit 1
        fi
        toolchain="telamon-settings-$version
$toolchain"
        if [ "$(cat "$cache/toolchain" 2>/dev/null)" != "$toolchain" ]; then
            rm -rf "$cache/cmake"
            printf '%s\n' "$toolchain" >"$cache/toolchain"
        fi
        rpmopts+=(--define "_telamon_build_cache $cache")
    else
        top=$(mktemp -d)
    fi
    trap 'rm -rf "$top"' EXIT
    mkdir -p "$top"/{SOURCES,BUILD,RPMS,SRPMS,SPECS}
    tar -C "$src" \
        --exclude=./.git --exclude=./target --exclude=./out --exclude=./build \
        --transform "s,^\./,telamon-settings-$version/," \
        -czf "$top/SOURCES/telamon-settings-$version.tar.gz" .

    rpmbuild -bb "${rpmopts[@]}" "$@" --define "_topdir $top" "$spec"

    mkdir -p "$out"
    find "$top/RPMS" -name '*.rpm' ! -name '*.src.rpm' ! -name '*debuginfo*' ! -name '*debugsource*' \
        -exec cp -v {} "$out"/ \;
}

main "$@"
exit $?
