#include "defaultapps.h"

#include <KApplicationTrader>
#include <KConfig>
#include <KConfigGroup>
#include <KService>
#include <KShell>

#include <QProcess>
#include <QRegularExpression>
#include <QSet>
#include <QStandardPaths>

#include <algorithm>

using namespace Qt::StringLiterals;

namespace
{
constexpr int MaxChoices = 500;

struct Kind {
    QString id;
    QString title;
    // The first is the one the current default is read from.
    QStringList mimes;
    // The category an app must have, when it is chosen by one (browsers and
    // terminals have no file type of their own).
    QString category;
};

const QList<Kind> &kindList()
{
    static const QList<Kind> kinds{
        {u"browser"_s,
         QObject::tr("Web Browser"),
         {u"x-scheme-handler/https"_s, u"x-scheme-handler/http"_s, u"text/html"_s, u"application/xhtml+xml"_s},
         u"WebBrowser"_s},
        {u"email"_s, QObject::tr("Email"), {u"x-scheme-handler/mailto"_s}, {}},
        {u"files"_s, QObject::tr("File Manager"), {u"inode/directory"_s}, {}},
        {u"terminal"_s, QObject::tr("Terminal"), {}, u"TerminalEmulator"_s},
        {u"music"_s,
         QObject::tr("Music"),
         {u"audio/mpeg"_s, u"audio/flac"_s, u"audio/ogg"_s, u"audio/x-vorbis+ogg"_s, u"audio/x-wav"_s, u"audio/mp4"_s, u"audio/aac"_s, u"audio/opus"_s},
         {}},
        {u"video"_s,
         QObject::tr("Video"),
         {u"video/mp4"_s, u"video/x-matroska"_s, u"video/webm"_s, u"video/x-msvideo"_s, u"video/quicktime"_s, u"video/mpeg"_s, u"video/ogg"_s},
         {}},
        {u"images"_s,
         QObject::tr("Images"),
         {u"image/png"_s, u"image/jpeg"_s, u"image/gif"_s, u"image/webp"_s, u"image/bmp"_s, u"image/tiff"_s, u"image/svg+xml"_s},
         {}},
    };
    return kinds;
}

const Kind *find(const QString &id)
{
    for (const Kind &k : kindList()) {
        if (k.id == id) {
            return &k;
        }
    }
    return nullptr;
}

QString iconName(const QString &icon)
{
    static const QRegularExpression ok(u"^[A-Za-z0-9._+-]{1,100}$"_s);
    return ok.match(icon).hasMatch() ? icon : u"application-x-executable"_s;
}

QVariantMap describe(const KService::Ptr &s)
{
    const auto cap = [](const QString &t, int max) {
        return t.size() > max ? t.left(max) + QChar(0x2026) : t;
    };
    return {
        {u"id"_s, s->storageId()},
        {u"name"_s, cap(s->name(), 120)},
        {u"icon"_s, iconName(s->icon())},
        {u"comment"_s, cap(s->comment(), 200)},
    };
}

KService::List candidates(const Kind &k)
{
    KService::List list;
    if (!k.mimes.isEmpty() && k.category.isEmpty()) {
        list = KApplicationTrader::queryByMimeType(k.mimes.first());
    } else {
        list = KApplicationTrader::query([&k](const KService::Ptr &s) {
            return s->isApplication() && !s->noDisplay() && s->categories().contains(k.category);
        });
    }
    // Only apps that run something, once each.
    KService::List out;
    QSet<QString> seen;
    for (const KService::Ptr &s : list) {
        if (s->exec().isEmpty() || seen.contains(s->storageId())) {
            continue;
        }
        seen.insert(s->storageId());
        out.append(s);
        if (out.size() >= MaxChoices) {
            break;
        }
    }
    std::sort(out.begin(), out.end(), [](const KService::Ptr &a, const KService::Ptr &b) {
        return QString::localeAwareCompare(a->name(), b->name()) < 0;
    });
    return out;
}

KConfig mimeApps()
{
    return KConfig(u"mimeapps.list"_s, KConfig::SimpleConfig, QStandardPaths::GenericConfigLocation);
}

KConfig globals()
{
    return KConfig(u"kdeglobals"_s, KConfig::SimpleConfig);
}

// The command a terminal service runs, without arguments ("konsole").
QString terminalCommand(const KService::Ptr &s)
{
    const QStringList words = KShell::splitArgs(s->exec());
    for (const QString &w : words) {
        if (!w.startsWith(u'%')) {
            return w;
        }
    }
    return {};
}
}

QVariantList DefaultApps::kinds()
{
    QVariantList out;
    for (const Kind &k : kindList()) {
        out.append(QVariantMap{{u"id"_s, k.id}, {u"title"_s, k.title}});
    }
    return out;
}

QStringList DefaultApps::mimeTypes(const QString &kind)
{
    const Kind *k = find(kind);
    return k ? k->mimes : QStringList{};
}

QVariantMap DefaultApps::current(const QString &kind) const
{
    const Kind *k = find(kind);
    if (!k) {
        return {};
    }
    KService::Ptr service;
    if (k->id == u"terminal") {
        KConfig config = globals();
        const QString id = KConfigGroup(&config, u"General"_s).readEntry("TerminalService", QString());
        service = KService::serviceByStorageId(id);
        if (!service) {
            const KService::List all = candidates(*k);
            if (!all.isEmpty()) {
                service = all.first();
            }
        }
    } else {
        // What the user's list says first (it is written at once; the
        // service cache catches up later), else what KService prefers.
        KConfig config = mimeApps();
        const QByteArray key = k->mimes.first().toUtf8();
        const QStringList ids = KConfigGroup(&config, u"Default Applications"_s).readXdgListEntry(key.constData());
        for (const QString &id : ids) {
            for (const KService::Ptr &s : candidates(*k)) {
                if (s->storageId() == id) {
                    service = s;
                    break;
                }
            }
            if (service) {
                break;
            }
        }
        if (!service) {
            service = KApplicationTrader::preferredService(k->mimes.first());
            if (service && !k->category.isEmpty() && !service->categories().contains(k->category)) {
                service.reset();
            }
        }
    }
    if (!service) {
        return {{u"id"_s, QString()}, {u"name"_s, QString()}, {u"icon"_s, QString()}};
    }
    return describe(service);
}

QVariantList DefaultApps::choices(const QString &kind) const
{
    const Kind *k = find(kind);
    QVariantList out;
    if (!k) {
        return out;
    }
    for (const KService::Ptr &s : candidates(*k)) {
        out.append(describe(s));
    }
    return out;
}

bool DefaultApps::setDefault(const QString &kind, const QString &id)
{
    const Kind *k = find(kind);
    if (!k) {
        return false;
    }
    // Only an app that is one of the choices.
    KService::Ptr service;
    for (const KService::Ptr &s : candidates(*k)) {
        if (s->storageId() == id) {
            service = s;
            break;
        }
    }
    if (!service) {
        return false;
    }
    if (k->id == u"terminal") {
        const QString command = terminalCommand(service);
        if (command.isEmpty()) {
            return false;
        }
        KConfig config = globals();
        KConfigGroup general(&config, u"General"_s);
        general.writeEntry("TerminalApplication", command, KConfigBase::Persistent | KConfigBase::Notify);
        general.writeEntry("TerminalService", service->storageId(), KConfigBase::Persistent | KConfigBase::Notify);
        return config.sync();
    }
    KConfig config = mimeApps();
    KConfigGroup defaults(&config, u"Default Applications"_s);
    KConfigGroup added(&config, u"Added Associations"_s);
    KConfigGroup removed(&config, u"Removed Associations"_s);
    for (const QString &mime : k->mimes) {
        const QByteArray key = mime.toUtf8();
        defaults.writeXdgListEntry(key.constData(), QStringList{id});
        QStringList list = added.readXdgListEntry(key.constData());
        list.removeAll(id);
        list.prepend(id);
        added.writeXdgListEntry(key.constData(), list);
        if (removed.hasKey(key.constData())) {
            QStringList gone = removed.readXdgListEntry(key.constData());
            if (gone.removeAll(id) > 0) {
                if (gone.isEmpty()) {
                    removed.deleteEntry(key.constData());
                } else {
                    removed.writeXdgListEntry(key.constData(), gone);
                }
            }
        }
    }
    if (!config.sync()) {
        return false;
    }
    // Other programs read the choice through the service cache: have it
    // rebuilt, without waiting for it.
    QProcess::startDetached(u"/usr/bin/kbuildsycoca6"_s, {});
    return true;
}
