#pragma once

#include <QObject>
#include <QVariantMap>

// The screen locker's settings as kcm_screenlocker keeps them:
// kscreenlockerrc, group [Daemon]. Written through KConfig (atomically,
// with the change notification kscreenlocker listens for); the running
// locker picks the change up itself.
class ScreenLockConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // {autoLock, minutes, onWake, grace}: lock after `minutes` of no use
    // when `autoLock`; `onWake` asks for the password when the computer wakes
    // from sleep; `grace` is the seconds after locking in which no password
    // is asked. The KCM's defaults for what the file doesn't say.
    Q_INVOKABLE QVariantMap read() const;

    // Lock after `minutes` of no use (0 never).
    Q_INVOKABLE bool setLockAfter(int minutes);
    // Ask for the password when the computer wakes up.
    Q_INVOKABLE bool setLockOnWake(bool on);
};
