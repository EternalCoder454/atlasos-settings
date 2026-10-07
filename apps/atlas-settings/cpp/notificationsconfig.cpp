#include "notificationsconfig.h"

#include "kdeutil.h"

#include <QDateTime>
#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>

using namespace Qt::StringLiterals;

namespace
{
constexpr auto File = "plasmanotifyrc";
constexpr int MaxApps = 200;

QString plain(const QString &s, int max = 100)
{
    QString out = s.simplified().left(max);
    for (QChar &c : out) {
        if (c.unicode() < 0x20 || c.unicode() == 0x7f) {
            c = u' ';
        }
    }
    return out;
}

// The name and icon of an app: its desktop file, else the Global group of
// its notification file.
struct Known
{
    QString name;
    QString icon;
};

Known knownApp(const QString &id, const QMap<QString, Known> &notifyrc)
{
    const QString desktop = QStandardPaths::locate(QStandardPaths::ApplicationsLocation, id + u".desktop"_s);
    if (!desktop.isEmpty()) {
        KConfig config(desktop, KConfig::SimpleConfig);
        const KConfigGroup g(&config, u"Desktop Entry"_s);
        const QString name = plain(g.readEntry("Name", QString()));
        if (!name.isEmpty()) {
            return {name, plain(g.readEntry("Icon", QString()), 200)};
        }
    }
    return notifyrc.value(id);
}
}

bool NotificationsConfig::validAppId(const QString &id)
{
    static const QRegularExpression re(u"^[A-Za-z0-9._-]{1,200}$"_s);
    return re.match(id).hasMatch();
}

QVariantMap NotificationsConfig::doNotDisturb() const
{
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    const QDateTime until = KConfigGroup(&config, u"DoNotDisturb"_s).readEntry("Until", QDateTime());
    const bool active = until.isValid() && until > QDateTime::currentDateTime();
    return {
        {u"active"_s, active},
        {u"until"_s, active ? until.toSecsSinceEpoch() : qint64(0)},
        {u"forever"_s, active && until.date().year() >= 2099},
    };
}

bool NotificationsConfig::setDoNotDisturb(const QString &until)
{
    const QDateTime now = QDateTime::currentDateTime();
    QDateTime when;
    if (until == u"30m"_s) {
        when = now.addSecs(30 * 60);
    } else if (until == u"1h"_s) {
        when = now.addSecs(60 * 60);
    } else if (until == u"4h"_s) {
        when = now.addSecs(4 * 60 * 60);
    } else if (until == u"tomorrow"_s) {
        // The morning: today's at 8:00 when it is still night.
        QDateTime morning(now.date(), QTime(8, 0));
        when = now.time() < QTime(6, 0) ? morning : morning.addDays(1);
    } else if (until == u"forever"_s) {
        when = QDateTime(QDate(2099, 12, 31), QTime(23, 59));
    } else if (until != u"off"_s) {
        return false;
    }
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    KConfigGroup g(&config, u"DoNotDisturb"_s);
    if (when.isValid()) {
        g.writeEntry("Until", when, kdeutil::Notify);
    } else {
        g.deleteEntry("Until", kdeutil::Notify);
    }
    return config.sync();
}

QVariantList NotificationsConfig::apps() const
{
    // The notification files apps install (KNotification's .notifyrc).
    QMap<QString, Known> notifyrc;
    for (const QString &dir : QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, u"knotifications6"_s, QStandardPaths::LocateDirectory)) {
        const QFileInfoList files = QDir(dir).entryInfoList({u"*.notifyrc"_s}, QDir::Files | QDir::Readable);
        for (const QFileInfo &f : files) {
            if (f.size() > 1024 * 1024) {
                continue;
            }
            KConfig config(f.absoluteFilePath(), KConfig::SimpleConfig);
            const KConfigGroup global(&config, u"Global"_s);
            QString id = global.readEntry("DesktopEntry", QString());
            if (id.isEmpty()) {
                id = f.completeBaseName();
            }
            if (validAppId(id) && !notifyrc.contains(id) && notifyrc.size() < MaxApps) {
                notifyrc.insert(id, {plain(global.readEntry("Name", QString())), plain(global.readEntry("IconName", QString()), 200)});
            }
        }
    }

    KConfig config = kdeutil::user(QString::fromLatin1(File));
    const KConfigGroup applications(&config, u"Applications"_s);
    QStringList ids = applications.groupList();
    for (auto it = notifyrc.cbegin(); it != notifyrc.cend(); ++it) {
        if (!ids.contains(it.key())) {
            ids << it.key();
        }
    }

    QVariantList result;
    for (const QString &id : ids) {
        if (!validAppId(id) || result.size() >= MaxApps) {
            continue;
        }
        const Known known = knownApp(id, notifyrc);
        // An app that is gone leaves its group behind: nothing to name it by.
        if (known.name.isEmpty()) {
            continue;
        }
        const KConfigGroup g(&applications, id);
        const bool banners = g.readEntry("ShowPopups", true);
        const bool history = g.readEntry("ShowInHistory", true);
        result.append(QVariantMap{{u"id"_s, id},
                                  {u"name"_s, known.name},
                                  {u"icon"_s, known.icon},
                                  {u"allowed"_s, banners || history},
                                  {u"banners"_s, banners}});
    }
    std::sort(result.begin(), result.end(), [](const QVariant &a, const QVariant &b) {
        return a.toMap().value(u"name"_s).toString().localeAwareCompare(b.toMap().value(u"name"_s).toString()) < 0;
    });
    return result;
}

bool NotificationsConfig::setAppAllowed(const QString &id, bool allowed)
{
    if (!validAppId(id)) {
        return false;
    }
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    KConfigGroup g = KConfigGroup(&config, u"Applications"_s).group(id);
    g.writeEntry("ShowPopups", allowed, kdeutil::Notify);
    g.writeEntry("ShowInHistory", allowed, kdeutil::Notify);
    g.writeEntry("ShowBadges", allowed, kdeutil::Notify);
    return config.sync();
}

bool NotificationsConfig::setAppBanners(const QString &id, bool banners)
{
    if (!validAppId(id)) {
        return false;
    }
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    KConfigGroup(&config, u"Applications"_s).group(id).writeEntry("ShowPopups", banners, kdeutil::Notify);
    return config.sync();
}

QVariantMap NotificationsConfig::popups() const
{
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    const KConfigGroup g(&config, u"Notifications"_s);
    const int timeout = std::clamp(g.readEntry("PopupTimeout", 5000), 1000, 120000);
    return {
        {u"position"_s, std::clamp(g.readEntry("PopupPosition", 0), 0, 6)},
        {u"timeout"_s, g.readEntry("ShowPopupTimeout", true) ? timeout / 1000 : 0},
    };
}

bool NotificationsConfig::setPopupPosition(int position)
{
    if (position < 0 || position > 6) {
        return false;
    }
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    KConfigGroup(&config, u"Notifications"_s).writeEntry("PopupPosition", position, kdeutil::Notify);
    return config.sync();
}

bool NotificationsConfig::setPopupTimeout(int seconds)
{
    if (seconds < 0 || seconds > 120) {
        return false;
    }
    KConfig config = kdeutil::user(QString::fromLatin1(File));
    KConfigGroup g(&config, u"Notifications"_s);
    if (seconds == 0) {
        g.writeEntry("ShowPopupTimeout", false, kdeutil::Notify);
    } else {
        g.writeEntry("ShowPopupTimeout", true, kdeutil::Notify);
        g.writeEntry("PopupTimeout", seconds * 1000, kdeutil::Notify);
    }
    return config.sync();
}
