#pragma once

#include <QObject>
#include <QVariantMap>

// PowerDevil's settings as kcm_powerdevilprofilesconfig keeps them:
// powerdevilrc, group [AC], [Battery] or [LowBattery], subgroups [Display]
// and [SuspendAndShutdown]. Written through KConfig (atomically, with the
// change notification PowerDevil's KConfigWatcher listens for). Local files
// only: PowerDevil is told to read them again by the page's Rust backend
// (settings_sys::power::reconfigure_powerdevil).
//
// Times are seconds; 0 is "never" (the KCM writes the "when idle" switch
// off and -1 for it).
class PowerConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // {screenOff, sleep, lid, powerButton} of `profile` ("AC", "Battery"):
    // the file's values, or PowerDevil's defaults for what it doesn't say.
    // Actions are PowerDevil's numbers (0 nothing, 1 sleep, 2 hibernate,
    // 8 shut down, 16 logout screen, 32 lock, 64 turn off the screen).
    Q_INVOKABLE QVariantMap read(const QString &profile) const;

    // Turn off the screen after `seconds` of no use (0 never).
    Q_INVOKABLE bool setScreenOff(const QString &profile, int seconds);
    // Sleep after `seconds` (0 never). Keeps hibernate when that is what
    // the profile already does.
    Q_INVOKABLE bool setSleep(const QString &profile, int seconds);
    Q_INVOKABLE bool setLid(const QString &profile, int action);
    Q_INVOKABLE bool setPowerButton(const QString &profile, int action);

    // The profiles this writes.
    Q_INVOKABLE static bool validProfile(const QString &profile);
};
