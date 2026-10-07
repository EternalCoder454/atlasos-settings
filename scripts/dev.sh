#!/bin/bash
# Run a command in the fedora:44 build container, with the repo at /src, the
# build output at /work and the cargo and dnf caches in named podman volumes
# (shared with the other Atlas apps).
#   scripts/dev.sh <command...>     e.g. scripts/dev.sh cargo test --workspace
#   scripts/dev.sh                  an interactive shell
# /work is $TELAMON_SETTINGS_WORK, by default ~/.cache/claude-builds/telamon-settings:
# on disk, outside the repo (cargo's target dirs, CMake build dirs, smoke-run
# XDG dirs). Set CARGO_TARGET_DIR to /work/target/<name> to keep one target
# dir per task.
# The first run installs the build dependencies from the spec (cached after),
# and python-dbusmock, which the settings-sys tests run the system services
# with on a private bus. Since the image has it, those tests fail rather than
# skip here (TELAMON_REQUIRE_DBUSMOCK=1 unless set otherwise).
# Telamon.Ui comes installed (telamon-ui, from atlas-framework), which no
# repository has: the first run needs TELAMON_LOCAL_RPMS=<dir with its RPMs>
# (built with atlas-framework's packaging/build-rpm.sh). Given later, the
# image takes those RPMs when they are another build than the one it has.
set -euo pipefail

# The old name of the variable still works.
TELAMON_LOCAL_RPMS=${TELAMON_LOCAL_RPMS:-${ATLAS_LOCAL_RPMS:-}}

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
image=localhost/telamon-settings-dev:44
work=${TELAMON_SETTINGS_WORK:-$HOME/.cache/claude-builds/telamon-settings}
mkdir -p "$work"
# The tools the image has besides the spec's build dependencies: clippy and
# rustfmt, Xvfb, xdotool, ImageMagick and procps (smoke and UI stress runs),
# dbus-daemon and python-dbusmock (the settings-sys tests), ccache.
tools="dnf5-plugins rpm-build clippy rustfmt xorg-x11-server-Xvfb dbus-daemon
    qt6-qtbase-gui kf6-qqc2-desktop-style breeze-icon-theme ImageMagick xdotool
    procps-ng ccache python3-dbusmock"
# The image is refreshed when either the spec or that list changes.
spec_sum=$({ cat "$repo/packaging/telamon-settings.spec"; echo "$tools"; } | sha256sum | cut -d' ' -f1)

# The telamon-ui and telamon-symbols-fonts RPMs in $TELAMON_LOCAL_RPMS: exactly one
# of each, or nothing is changed.
local_rpms() {
    local dir=$1 name f
    local -a found
    for name in telamon-ui telamon-symbols-fonts; do
        found=()
        for f in "$dir/$name"-[0-9]*.rpm; do
            [ -e "$f" ] && [[ $f != *.src.rpm ]] && found+=("${f##*/}")
        done
        if [ "${#found[@]}" != 1 ]; then
            echo "dev.sh: $dir needs exactly one $name RPM, found ${#found[@]}: ${found[*]}" >&2
            return 1
        fi
        printf '%s\n' "${found[0]}"
    done
}

new=1
podman image exists "$image" && new=0
if [ "$new" = 1 ] && [ -z "${TELAMON_LOCAL_RPMS:-}" ]; then
    echo "dev.sh: the first run needs TELAMON_LOCAL_RPMS=<dir with the telamon-ui and telamon-symbols-fonts RPMs>" >&2
    exit 1
fi
if [ -n "${TELAMON_LOCAL_RPMS:-}" ]; then
    rpms=$(cd "$TELAMON_LOCAL_RPMS" && pwd)
    mapfile -t files < <(local_rpms "$rpms")
    [ "${#files[@]}" = 2 ] || exit 1
    # Which build: the label the image was committed with.
    build=$(cd "$rpms" && rpm -qp --qf '%{NEVRA}-%{BUILDTIME},' "${files[@]}")
    have=$([ "$new" = 1 ] || podman image inspect --format '{{index .Labels "telamon-ui"}}' "$image")
    if [ "$build" != "$have" ]; then
        # From the clean base each time (the dnf cache makes it quick), so
        # refreshes don't stack layers on the old image.
        # No relabelling, as for /src below. The RPMs are copied in, not
        # mounted: only these two files of $TELAMON_LOCAL_RPMS are read.
        ctr=$(podman run -d --security-opt label=disable -v "$repo/packaging":/packaging:ro \
            -v telamon-dnf:/var/cache/libdnf5 \
            registry.fedoraproject.org/fedora:44 sleep infinity)
        trap 'podman rm -f -t 0 "$ctr" >/dev/null' EXIT
        podman exec "$ctr" mkdir /rpms
        for f in "${files[@]}"; do
            podman cp "$rpms/$f" "$ctr:/rpms/$f"
        done
        # shellcheck disable=SC2016 # expanded by the container's shell
        podman exec -e TOOLS="$tools" "$ctr" bash -c '
            set -e
            echo keepcache=True >>/etc/dnf/dnf.conf
            # Split on whitespace on purpose: a list of package names.
            # shellcheck disable=SC2086
            dnf -y install $TOOLS
            cd /rpms
            dnf -y install "$@"
            # The exact files, also when this version or a newer one is installed.
            rpm -U --replacepkgs --oldpackage "$@"
            cd /
            rm -r /rpms
            dnf -y builddep /packaging/telamon-settings.spec' bash "${files[@]}" >&2
        podman commit --change "LABEL telamon-ui=$build" --change "LABEL spec=$spec_sum" \
            "$ctr" "$image" >/dev/null
        podman rm -f -t 0 "$ctr" >/dev/null
        trap - EXIT
    fi
fi

# A changed spec (new BuildRequires) or tools list installs them into the
# image, keeping its Telamon.Ui.
if [ "$(podman image inspect --format '{{index .Labels "spec"}}' "$image")" != "$spec_sum" ]; then
    ctr=$(podman run -d --security-opt label=disable -v "$repo/packaging":/packaging:ro \
        -v telamon-dnf:/var/cache/libdnf5 "$image" sleep infinity)
    trap 'podman rm -f -t 0 "$ctr" >/dev/null' EXIT
    # shellcheck disable=SC2016,SC2086 # $TOOLS is split on purpose, in the container
    podman exec -e TOOLS="$tools" "$ctr" bash -c 'dnf -y install $TOOLS && dnf -y builddep /packaging/telamon-settings.spec' >&2
    podman commit --change "LABEL spec=$spec_sum" "$ctr" "$image" >/dev/null
    podman rm -f -t 0 "$ctr" >/dev/null
    trap - EXIT
fi

tty=()
[ -t 0 ] && tty=(-it)
# SELinux labelling is off for the container (label=disable) rather than
# relabelling the mounts with :z or :Z, which would change the labels of the
# repo and ~/.cache on the host.
exec podman run --rm "${tty[@]}" --security-opt label=disable \
    -v "$repo":/src -w /src \
    -v "$work":/work \
    -v telamon-cargo:/root/.cargo/registry \
    -v telamon-cargo-git:/root/.cargo/git \
    -v telamon-settings-ccache:/root/.cache/ccache \
    -e CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/work/target/dev}" \
    -e TELAMON_REQUIRE_DBUSMOCK="${TELAMON_REQUIRE_DBUSMOCK:-1}" \
    "$image" bash -c '
        # C++ through ccache (its own volume), so a new build dir or a
        # rebuild after a header change reuses what was compiled before.
        if command -v ccache >/dev/null; then
            export CMAKE_CXX_COMPILER_LAUNCHER=ccache CMAKE_C_COMPILER_LAUNCHER=ccache
        fi
        exec "$@"' bash "${@:-bash}"
