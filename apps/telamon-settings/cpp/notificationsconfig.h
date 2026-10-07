#pragma once

#include <QObject>
#include <QVariantList>
#include <QVariantMap>

// Notifications as Plasma keeps them: plasmanotifyrc, the file
// kcm_notifications writes and Plasma's notification server watches
// (docs/DESIGN.md, "Notifications"). Written through KConfig with a change
// notification, with the keys of libnotificationmanager's kcfg files:
//
// - Do Not Disturb is `[DoNotDisturb] Until`: a time in the future turns it
//   on until then, none turns it off.
// - Each app is `[Applications][<desktop entry>]`: `ShowPopups` (banners),
//   `ShowInHistory`, `ShowBadges`.
// - Popups are `[Notifications] PopupPosition` (the module's enum),
//   `PopupTimeout` (milliseconds) and `ShowPopupTimeout` (false: popups stay
//   until dismissed).
// Local files only: nothing here waits on D-Bus.
class NotificationsConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // {active, until (seconds since 1970, 0 for none), forever}
    Q_INVOKABLE QVariantMap doNotDisturb() const;
    // "off", "30m", "1h", "4h", "tomorrow" (8:00 the next morning) or
    // "forever".
    Q_INVOKABLE bool setDoNotDisturb(const QString &until);

    // [{id, name, icon, allowed, banners}]: the apps that have sent
    // notifications or have a notification file, by name. `allowed` is
    // false when both the banners and the history are off.
    Q_INVOKABLE QVariantList apps() const;
    Q_INVOKABLE bool setAppAllowed(const QString &id, bool allowed);
    Q_INVOKABLE bool setAppBanners(const QString &id, bool banners);

    // {position: 0..6 (the module's order: near the widget, top left, top
    // centre, top right, bottom left, bottom centre, bottom right),
    // timeout: seconds, 0 for "until dismissed"}
    Q_INVOKABLE QVariantMap popups() const;
    Q_INVOKABLE bool setPopupPosition(int position);
    Q_INVOKABLE bool setPopupTimeout(int seconds);

    // Whether `id` is an app ID that can be written.
    Q_INVOKABLE static bool validAppId(const QString &id);
};
