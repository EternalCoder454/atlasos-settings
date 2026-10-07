#!/bin/bash
# The cutover, end to end, in a fedora:44 container: the RPMs from
# packaging/build-rpm.sh go onto a Plasma that has Fedora's plasma-systemsettings,
# and the transaction and the `systemsettings` command are checked.
#
#   scripts/test-systemsettings-rpm.sh <dir with the built RPMs> <dir with the telamon-ui RPMs>
#
# Checks: telamon-settings-systemsettings replaces plasma-systemsettings and
# keeps plasma-desktop, colord-kde and kcm-plasmalogin (which require it);
# dnf does not bring Fedora's package back; the files are where the spec
# says. Then /usr/bin/telamon-settings and /usr/bin/kcmshell6 are replaced
# by stubs that log their argv, and `systemsettings kcm_kscreen` must reach
# telamon-settings --kcm kcm_kscreen, `systemsettings kcm_trash` kcmshell6
# kcm_trash, and so on; the time from starting the command to the stub
# running is measured against the 5 ms budget. Nothing touches the host: it
# is all inside the container, which is thrown away.
set -euo pipefail

if [ "${1:-}" != --inner ]; then
    rpms=$(cd "${1:?usage: test-systemsettings-rpm.sh <rpm dir> <telamon-ui rpm dir>}" && pwd)
    fw=$(cd "${2:?usage: test-systemsettings-rpm.sh <rpm dir> <telamon-ui rpm dir>}" && pwd)
    here=$(cd "$(dirname "$0")" && pwd)
    exec podman run --rm --security-opt label=disable \
        -v "$rpms":/rpms:ro -v "$fw":/fw:ro -v "$here/test-systemsettings-rpm.sh":/test.sh:ro \
        -v telamon-dnf:/var/cache/libdnf5 \
        registry.fedoraproject.org/fedora:44 bash /test.sh --inner
fi

fail=0
ok() { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }
check() { # description, command...
    local d=$1
    shift
    if "$@" >/dev/null 2>&1; then ok "$d"; else bad "$d"; fi
}

echo "== a Plasma with Fedora's System Settings"
echo keepcache=True >>/etc/dnf/dnf.conf
dnf -y -q install plasma-desktop plasma-systemsettings kcm-plasmalogin colord-kde kf6-kcmutils \
    gcc desktop-file-utils >/dev/null 2>&1
rpm -q plasma-desktop plasma-systemsettings kcm-plasmalogin colord-kde kf6-kcmutils
check "systemsettings is KDE's" bash -c 'rpm -qf /usr/bin/systemsettings | grep -q "^plasma-systemsettings"'

echo "== install the RPMs"
ls /rpms
mapfile -t ours < <(ls /rpms/telamon-settings-[0-9]*.rpm /rpms/telamon-settings-systemsettings-[0-9]*.rpm)
mapfile -t fw < <(ls /fw/telamon-ui-[0-9]*.rpm /fw/telamon-symbols-fonts-[0-9]*.rpm)
dnf -y install "${fw[@]}" "${ours[@]}" 2>&1 | grep -E "^(Installing|Removing|Upgrading|Replacing|Obsoleting|Transaction|Failed|Error|Problem)|plasma-systemsettings|telamon-settings" | head -30
echo "== the transaction"
check "telamon-settings-systemsettings is installed" rpm -q telamon-settings-systemsettings
check "telamon-settings is installed" rpm -q telamon-settings
check "plasma-systemsettings is gone" bash -c '! rpm -q plasma-systemsettings'
check "plasma-desktop, colord-kde and kcm-plasmalogin stayed" rpm -q plasma-desktop colord-kde kcm-plasmalogin
check "the name is provided by telamon-settings-systemsettings" bash -c '[ "$(rpm -q --whatprovides plasma-systemsettings)" = "$(rpm -q telamon-settings-systemsettings)" ]'
check "with the architecture too" bash -c '[ "$(rpm -q --whatprovides "plasma-systemsettings(x86-64)")" = "$(rpm -q telamon-settings-systemsettings)" ]'
check "/usr/bin/systemsettings is ours" bash -c '[ "$(rpm -qf /usr/bin/systemsettings)" = "$(rpm -q telamon-settings-systemsettings)" ]'
check "the KDE desktop files are ours and hidden" bash -c 'for f in systemsettings kdesystemsettings; do [ "$(rpm -qf /usr/share/applications/$f.desktop)" = "$(rpm -q telamon-settings-systemsettings)" ] && grep -qx NoDisplay=true /usr/share/applications/$f.desktop; done'
check "Settings is visible" bash -c '! grep -q ^NoDisplay=true /usr/share/applications/net.eterneon.telamon.settings.desktop'
check "its D-Bus service file is there" test -f /usr/share/dbus-1/services/net.eterneon.telamon.settings.service
check "the Launcher's search index is there" grep -q '"link":"displays/night-light"' /usr/share/telamon-settings/search-index.json
check "desktop files validate" desktop-file-validate /usr/share/applications/net.eterneon.telamon.settings.desktop /usr/share/applications/systemsettings.desktop /usr/share/applications/kdesystemsettings.desktop
check "rpm database is consistent" rpm -Va --nofiles --nodigest plasma-desktop telamon-settings-systemsettings
echo "-- dnf does not bring Fedora's package back"
dnf -y install plasma-systemsettings 2>&1 | tail -3
check "still not installed" bash -c '! rpm -q plasma-systemsettings'
check "dnf upgrade leaves it out" bash -c 'dnf -y upgrade >/dev/null 2>&1; ! rpm -q plasma-systemsettings && rpm -q telamon-settings-systemsettings'
echo "== systemsettings -> the two programs (argv logged by stubs)"
log=/tmp/argv.log
for p in /usr/bin/telamon-settings /usr/bin/kcmshell6; do
    cat >"$p" <<'STUB'
#!/bin/sh
# argv in brackets, one line per start; the activation token goes through.
{ printf '%s' "${0##*/}"; for a in "$@"; do printf ' [%s]' "$a"; done; printf ' token=%s\n' "${XDG_ACTIVATION_TOKEN:-}"; } >>/tmp/argv.log
STUB
    chmod 755 "$p"
done
run() { : >"$log"; XDG_ACTIVATION_TOKEN=tok systemsettings "$@" >/tmp/out 2>/tmp/err || echo "(status $?)" >>"$log"; cat "$log"; }
expect() { # command line..., then the expected log line after "->"
    local args=() want
    while [ "$1" != -- ]; do args+=("$1"); shift; done
    shift
    want=$1
    got=$(run "${args[@]}")
    if [ "$got" = "$want" ]; then ok "systemsettings ${args[*]} -> $want"; else bad "systemsettings ${args[*]} -> $got (wanted $want)"; fi
}
expect kcm_kscreen -- 'telamon-settings [--kcm] [kcm_kscreen] token=tok'
expect kcm_trash -- 'kcmshell6 [kcm_trash] token=tok'
expect kcm_users -- 'telamon-settings [--kcm] [kcm_users] token=tok'
expect kcm_keys -- 'kcmshell6 [kcm_keys] token=tok'
expect -- 'telamon-settings token=tok'
expect kcm_landingpage -- 'telamon-settings token=tok'
expect kcm_fonts --args 'a b' -- 'kcmshell6 [kcm_fonts] [--args=a b] token=tok'
expect kcm_fonts --args=--evil -- 'kcmshell6 [kcm_fonts] [--args=--evil] token=tok'
expect plasma/kcms/systemsettings/kcm_bluetooth -- 'telamon-settings [--kcm] [kcm_bluetooth] token=tok'
for bad in '../../bin/sh' 'kcm_a;reboot' '--evil' 'kcm_a kcm_b' '/usr/bin/id'; do
    got=$(run "$bad")
    if [[ $got == '(status 2)' ]] && [ -s /tmp/err ]; then ok "refused: $bad"; else bad "not refused: $bad -> $got"; fi
done
got=$(run kcm_a kcm_b)
[[ $got == '(status 2)' ]] && ok "two modules refused" || bad "two modules: $got"
check "no shell in the way: PATH is not consulted" bash -c 'mkdir /tmp/fakebin && printf "#!/bin/sh\necho HIJACKED >>/tmp/argv.log\n" >/tmp/fakebin/kcmshell6 && chmod 755 /tmp/fakebin/kcmshell6 && : >/tmp/argv.log && PATH=/tmp/fakebin:$PATH systemsettings kcm_trash && ! grep -q HIJACKED /tmp/argv.log && grep -q "^kcmshell6 \[kcm_trash\]" /tmp/argv.log'
echo "-- also the way KCMLauncher calls it, from a program, without a shell"
cat >/tmp/launch.c <<'C'
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>
#include <sys/wait.h>
#include <spawn.h>
extern char **environ;
static double now(void){struct timespec t;clock_gettime(CLOCK_REALTIME,&t);return t.tv_sec*1e3+t.tv_nsec/1e6;}
int main(int c,char**v){
    /* usage: launch <runs> <module>; the stub is a C program that writes its start time */
    int runs=atoi(v[1]); double *d=malloc(sizeof(double)*runs);
    for(int i=0;i<runs;i++){
        remove("/tmp/stamp");
        double t0=now(); pid_t p; char*argv[]={"systemsettings",v[2],0};
        posix_spawn(&p,"/usr/bin/systemsettings",0,0,argv,environ); int st; waitpid(p,&st,0);
        FILE*f=fopen("/tmp/stamp","r"); double t1=0; if(f){fscanf(f,"%lf",&t1);fclose(f);}
        d[i]=t1-t0;
    }
    for(int i=0;i<runs;i++)for(int j=i+1;j<runs;j++)if(d[j]<d[i]){double x=d[i];d[i]=d[j];d[j]=x;}
    printf("%.3f\n",d[runs/2]); return 0; }
C
cat >/tmp/stub.c <<'C'
#include <stdio.h>
#include <time.h>
int main(void){struct timespec t;clock_gettime(CLOCK_REALTIME,&t);FILE*f=fopen("/tmp/stamp","w");fprintf(f,"%.6f",t.tv_sec*1e3+t.tv_nsec/1e6);fclose(f);return 0;}
C
gcc -O2 -o /tmp/launch /tmp/launch.c && gcc -O2 -o /usr/bin/telamon-settings /tmp/stub.c && cp /usr/bin/telamon-settings /usr/bin/kcmshell6
for m in kcm_kscreen kcm_trash; do
    ms=$(/tmp/launch 200 "$m")
    echo "  $m: median ${ms} ms from spawning systemsettings to the program's first line"
    if awk -v m="$ms" 'BEGIN { exit !(m > 0 && m < 5) }'; then ok "$m within 5 ms"; else bad "$m over 5 ms: $ms"; fi
done

[ "$fail" = 0 ] && echo "ALL OK" || { echo "FAILED"; exit 1; }
