#pragma once

#include <QObject>
#include <QVariantList>

// The apps that start when the user signs in, as kcm_autostart and the XDG
// autostart specification keep them: .desktop files in
// ~/.config/autostart (the user's) over /etc/xdg/autostart and the other
// XDG_CONFIG_DIRS' autostart folders (the system's). A system entry is
// turned off by a file of the same name in the user's folder with
// `Hidden=true`, never by touching the system's file. Local files only.
class AutostartConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // The entries a person would recognise (not NoDisplay, shown in
    // Plasma): [{id, name, icon, comment, enabled, system}]. `system` is an
    // entry the system provides: it can be turned off, not removed.
    Q_INVOKABLE QVariantList entries() const;

    // Whether `id` ("org.kde.plasmashell.desktop") starts at sign-in.
    Q_INVOKABLE bool isEnabled(const QString &id) const;

    // Whether an entry of this name exists, the user's or the system's.
    Q_INVOKABLE bool known(const QString &id) const;

    // Turns an entry on or off for this user. False for an id that isn't a
    // desktop file name, or when the file can't be written.
    Q_INVOKABLE bool setEnabled(const QString &id, bool on);

    // Removes the user's own entry (a system entry it overrides comes back
    // as the system has it). False when there is no such entry of the
    // user's.
    Q_INVOKABLE bool remove(const QString &id);

    // Starts an installed app at sign-in: its desktop file is copied into
    // the user's autostart folder. `id` is a desktop file name.
    Q_INVOKABLE bool add(const QString &id);

    // The installed apps, for choosing one: [{id, name, icon, comment}].
    Q_INVOKABLE QVariantList installedApps() const;

    // A desktop file name: letters, digits, dots, dashes and underscores,
    // ending in .desktop, no path.
    Q_INVOKABLE static bool validId(const QString &id);
};
