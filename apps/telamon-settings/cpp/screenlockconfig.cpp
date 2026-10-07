#include "screenlockconfig.h"

#include <KConfig>
#include <KConfigGroup>

using namespace Qt::StringLiterals;

namespace
{
// The file and group of kcm_screenlocker (kscreenlockersettings.kcfg).
constexpr auto File = "kscreenlockerrc";
constexpr auto Daemon = "Daemon";
// A day: more is a typo.
constexpr int MaxMinutes = 24 * 60;

KConfig open()
{
    return KConfig(QString::fromLatin1(File), KConfig::NoGlobals);
}
}

QVariantMap ScreenLockConfig::read() const
{
    KConfig config = open();
    const KConfigGroup daemon(&config, QString::fromLatin1(Daemon));
    const bool autoLock = daemon.readEntry("Autolock", true);
    const int minutes = qBound(0, daemon.readEntry("Timeout", 5), MaxMinutes);
    return {
        // Autolock on with no time is as good as off.
        {u"autoLock"_s, autoLock && minutes > 0},
        {u"minutes"_s, autoLock ? minutes : 0},
        {u"onWake"_s, daemon.readEntry("LockOnResume", true)},
        {u"grace"_s, qBound(0, daemon.readEntry("LockGrace", 5), 300)},
    };
}

bool ScreenLockConfig::setLockAfter(int minutes)
{
    if (minutes < 0 || minutes > MaxMinutes) {
        return false;
    }
    KConfig config = open();
    KConfigGroup daemon(&config, QString::fromLatin1(Daemon));
    daemon.writeEntry("Autolock", minutes > 0, KConfigBase::Persistent | KConfigBase::Notify);
    if (minutes > 0) {
        daemon.writeEntry("Timeout", minutes, KConfigBase::Persistent | KConfigBase::Notify);
    }
    return config.sync();
}

bool ScreenLockConfig::setLockOnWake(bool on)
{
    KConfig config = open();
    KConfigGroup daemon(&config, QString::fromLatin1(Daemon));
    daemon.writeEntry("LockOnResume", on, KConfigBase::Persistent | KConfigBase::Notify);
    return config.sync();
}
