#include "autostartconfig.h"

#include <KApplicationTrader>
#include <KConfig>
#include <KConfigGroup>
#include <KDesktopFile>
#include <KService>

#include <QDir>
#include <QFileInfo>
#include <QMap>
#include <QSet>
#include <QFile>
#include <QRegularExpression>
#include <QStandardPaths>

#include <algorithm>
#include <memory>

using namespace Qt::StringLiterals;

namespace
{
constexpr auto Entry = "Desktop Entry";
// More than this is not a list a person reads.
constexpr int MaxEntries = 500;
constexpr int MaxApps = 2000;

QString userDir()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation) + u"/autostart"_s;
}

// The autostart folders of the system: every config dir but the user's.
QStringList systemDirs()
{
    QStringList dirs;
    const QString user = userDir();
    for (const QString &d : QStandardPaths::standardLocations(QStandardPaths::GenericConfigLocation)) {
        const QString dir = d + u"/autostart"_s;
        if (dir != user) {
            dirs << dir;
        }
    }
    return dirs;
}

// The system's file for `id`, the first folder that has it; empty for none.
QString systemFile(const QString &id)
{
    for (const QString &d : systemDirs()) {
        const QString path = d + u'/' + id;
        if (QFileInfo::exists(path)) {
            return path;
        }
    }
    return {};
}

// Whether the user's file at `path` may be written: not there yet, a regular
// file, or a link to a regular .desktop file (a dotfiles manager's). Never a
// link to anything else, or to nothing: KConfig and QFile::copy write through
// links, so a link someone else put in the folder (an app that may write
// there, a name picked to look like an entry) would have Settings replace the
// file it points at, a shell's startup file say.
bool safeToWrite(const QString &path)
{
    const QFileInfo info(path);
    if (info.isSymLink()) {
        const QString target = info.canonicalFilePath();
        return !target.isEmpty() && QFileInfo(target).isFile() && target.endsWith(u".desktop"_s);
    }
    return !info.exists() || info.isFile();
}

bool shownInPlasma(const KConfigGroup &g)
{
    const QStringList only = g.readXdgListEntry("OnlyShowIn");
    const QStringList hidden = g.readXdgListEntry("NotShowIn");
    if (!only.isEmpty() && !only.contains(u"KDE"_s)) {
        return false;
    }
    return !hidden.contains(u"KDE"_s);
}

bool enabledIn(const KConfigGroup &g)
{
    return !g.readEntry("Hidden", false) && g.readEntry("X-GNOME-Autostart-enabled", true);
}

// A theme icon name; a path or anything odd is not shown.
QString iconName(const QString &icon)
{
    static const QRegularExpression ok(u"\\A[A-Za-z0-9._+-]{1,100}\\z"_s);
    return ok.match(icon).hasMatch() ? icon : u"application-x-executable"_s;
}

QString cap(const QString &text, int max)
{
    return text.size() > max ? text.left(max) + QChar(0x2026) : text;
}
}

bool AutostartConfig::validId(const QString &id)
{
    static const QRegularExpression re(u"\\A[A-Za-z0-9._-]{1,120}\\.desktop\\z"_s);
    return re.match(id).hasMatch() && !id.contains(u".."_s);
}

QVariantList AutostartConfig::entries() const
{
    QMap<QString, QString> files; // id -> path, the user's over the system's
    QSet<QString> systemIds;
    for (const QString &d : systemDirs()) {
        for (const QString &name : QDir(d).entryList({u"*.desktop"_s}, QDir::Files)) {
            if (validId(name) && !files.contains(name)) {
                files.insert(name, d + u'/' + name);
                systemIds.insert(name);
            }
        }
    }
    const QString user = userDir();
    for (const QString &name : QDir(user).entryList({u"*.desktop"_s}, QDir::Files)) {
        if (validId(name)) {
            files.insert(name, user + u'/' + name);
        }
    }

    QVariantList out;
    for (auto it = files.cbegin(); it != files.cend() && out.size() < MaxEntries; ++it) {
        const QString &id = it.key();
        // What the user's file doesn't say, the system's does.
        const QString sysPath = systemFile(id);
        KDesktopFile main(it.value());
        const KConfigGroup g = main.desktopGroup();
        KConfigGroup shown = g;
        std::unique_ptr<KDesktopFile> base;
        if (it.value() != sysPath && !sysPath.isEmpty() && g.readEntry("Name", QString()).isEmpty()) {
            base = std::make_unique<KDesktopFile>(sysPath);
            shown = base->desktopGroup();
        }
        if (shown.readEntry("NoDisplay", false) || !shownInPlasma(shown)) {
            continue;
        }
        const QString name = (base ? base->readName() : main.readName());
        if (name.isEmpty()) {
            continue;
        }
        out.append(QVariantMap{
            {u"id"_s, id},
            {u"name"_s, cap(name, 120)},
            {u"icon"_s, iconName(base ? base->readIcon() : main.readIcon())},
            {u"comment"_s, cap(base ? base->readComment() : main.readComment(), 200)},
            {u"enabled"_s, enabledIn(g)},
            {u"system"_s, systemIds.contains(id)},
        });
    }
    std::sort(out.begin(), out.end(), [](const QVariant &a, const QVariant &b) {
        return QString::localeAwareCompare(a.toMap().value(u"name"_s).toString(), b.toMap().value(u"name"_s).toString()) < 0;
    });
    return out;
}

bool AutostartConfig::isEnabled(const QString &id) const
{
    if (!validId(id)) {
        return false;
    }
    const QString user = userDir() + u'/' + id;
    if (QFileInfo::exists(user)) {
        return enabledIn(KDesktopFile(user).desktopGroup());
    }
    const QString sys = systemFile(id);
    return !sys.isEmpty() && enabledIn(KDesktopFile(sys).desktopGroup());
}

bool AutostartConfig::known(const QString &id) const
{
    return validId(id) && (QFileInfo::exists(userDir() + u'/' + id) || !systemFile(id).isEmpty());
}

bool AutostartConfig::setEnabled(const QString &id, bool on)
{
    if (!validId(id)) {
        return false;
    }
    const QString path = userDir() + u'/' + id;
    const QString sys = systemFile(id);
    if (!QDir().mkpath(userDir()) || !safeToWrite(path)) {
        return false;
    }
    if (!on) {
        KConfig config(path, KConfig::SimpleConfig);
        KConfigGroup g(&config, QString::fromLatin1(Entry));
        if (!QFileInfo::exists(path)) {
            g.writeEntry("Type", "Application");
        }
        g.writeEntry("Hidden", true);
        return config.sync();
    }
    // On: the user's file stops saying "off". One that only said that (it
    // masked a system entry) goes, so the system's entry is as it ships.
    if (QFileInfo::exists(path)) {
        KConfig config(path, KConfig::SimpleConfig);
        KConfigGroup g(&config, QString::fromLatin1(Entry));
        g.deleteEntry("Hidden");
        g.deleteEntry("X-GNOME-Autostart-enabled");
        if (!config.sync()) {
            return false;
        }
        const QStringList keys = g.keyList();
        const bool onlyMask = keys.isEmpty() || (keys.size() == 1 && keys.first() == u"Type"_s);
        if (onlyMask && !sys.isEmpty()) {
            if (!enabledIn(KDesktopFile(sys).desktopGroup())) {
                // The system's entry is off by itself: say "on" explicitly.
                g.writeEntry("Hidden", false);
                return config.sync();
            }
            return QFile::remove(path);
        }
        return true;
    }
    if (sys.isEmpty()) {
        return false;
    }
    if (enabledIn(KDesktopFile(sys).desktopGroup())) {
        return true;
    }
    KConfig config(path, KConfig::SimpleConfig);
    KConfigGroup g(&config, QString::fromLatin1(Entry));
    g.writeEntry("Type", "Application");
    g.writeEntry("Hidden", false);
    return config.sync();
}

bool AutostartConfig::remove(const QString &id)
{
    if (!validId(id)) {
        return false;
    }
    const QString path = userDir() + u'/' + id;
    return QFileInfo(path).isFile() && QFile::remove(path);
}

bool AutostartConfig::add(const QString &id)
{
    if (!validId(id)) {
        return false;
    }
    const KService::Ptr service = KService::serviceByStorageId(id);
    if (!service || !service->isApplication() || service->entryPath().isEmpty()) {
        return false;
    }
    if (!QDir().mkpath(userDir())) {
        return false;
    }
    const QString path = userDir() + u'/' + id;
    if (!safeToWrite(path)) {
        return false;
    }
    if (QFileInfo::exists(path)) {
        return setEnabled(id, true);
    }
    return QFile::copy(service->entryPath(), path);
}

QVariantList AutostartConfig::installedApps() const
{
    const KService::List apps = KApplicationTrader::query([](const KService::Ptr &s) {
        return s->isApplication() && !s->noDisplay() && !s->exec().isEmpty();
    });
    QVariantList out;
    QSet<QString> seen;
    for (const KService::Ptr &s : apps) {
        const QString id = s->storageId();
        if (!validId(id) || seen.contains(id) || out.size() >= MaxApps) {
            continue;
        }
        seen.insert(id);
        out.append(QVariantMap{
            {u"id"_s, id},
            {u"name"_s, cap(s->name(), 120)},
            {u"icon"_s, iconName(s->icon())},
            {u"comment"_s, cap(s->comment(), 200)},
        });
    }
    std::sort(out.begin(), out.end(), [](const QVariant &a, const QVariant &b) {
        return QString::localeAwareCompare(a.toMap().value(u"name"_s).toString(), b.toMap().value(u"name"_s).toString()) < 0;
    });
    return out;
}
