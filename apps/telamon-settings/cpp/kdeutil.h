#pragma once

#include <KConfig>
#include <KConfigGroup>

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QObject>
#include <QStandardPaths>
#include <QStringList>
#include <QVariantList>

#include <functional>

// What the page backends share: how Plasma's own settings modules write a
// value (through KConfig, announced with KConfig::Notify so Plasma, KWin and
// the apps that watch the file pick it up at once) and how they ask a
// running service something (an asynchronous D-Bus call, so the GUI thread
// never waits for it).
namespace kdeutil
{
// KConfig writes a new file and renames it over the old one, so a reader
// never sees half a file.
constexpr auto Notify = KConfigBase::Notify | KConfigBase::Persistent;

// The user's config folder ($XDG_CONFIG_HOME, else ~/.config).
inline QString configHome()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation);
}

// The user's file `name` ("kdeglobals", "plasmanotifyrc") on its own, with
// no system file under it: what Settings writes.
inline KConfig user(const QString &name)
{
    return KConfig(name, KConfig::NoGlobals);
}

// A value as the session sees it: the user's file first, then each system
// file (/etc/xdg) in turn; `fallback` when none has it.
inline QString layered(const QString &name, const QString &group, const QString &key, const QString &fallback = QString())
{
    const QStringList files = QStandardPaths::locateAll(QStandardPaths::GenericConfigLocation, name);
    for (const QString &file : files) {
        KConfig config(file, KConfig::SimpleConfig);
        const KConfigGroup g(&config, group);
        if (g.hasKey(key)) {
            return g.readEntry(key, fallback);
        }
    }
    return fallback;
}

// Calls a method on the session bus without waiting; `done` gets the reply
// on the GUI thread (an error reply too: check isError()).
inline void call(QObject *context,
                 const QString &service,
                 const QString &path,
                 const QString &interface,
                 const QString &method,
                 const QVariantList &args = {},
                 std::function<void(const QDBusMessage &)> done = {})
{
    QDBusMessage message = QDBusMessage::createMethodCall(service, path, interface, method);
    message.setArguments(args);
    auto *watcher = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(message, 5000), context);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, context, [watcher, done = std::move(done)] {
        if (done) {
            done(watcher->reply());
        }
        watcher->deleteLater();
    });
}

// Asks KWin to read its settings again (what the KWin modules do after they
// save kwinrc).
inline void reconfigureKWin(QObject *context)
{
    call(context, QStringLiteral("org.kde.KWin"), QStringLiteral("/KWin"), QStringLiteral("org.kde.KWin"), QStringLiteral("reconfigure"));
}
}
