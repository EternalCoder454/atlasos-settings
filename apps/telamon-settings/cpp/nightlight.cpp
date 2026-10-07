#include "nightlight.h"

#include <KConfig>
#include <KConfigGroup>

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusVariant>

#include <algorithm>

using namespace Qt::StringLiterals;

namespace
{
constexpr auto File = "kwinrc";
constexpr auto Group = "NightColor";
// The values of ColorCorrect::NightLightMode in kcm_nightlight (enum.h).
constexpr int ModeConstant = 0;
constexpr int ModeDarkLight = 1;

const QString Service = u"org.kde.KWin.NightLight"_s;
const QString Path = u"/org/kde/KWin/NightLight"_s;

KConfig open()
{
    return KConfig(QString::fromLatin1(File), KConfig::NoGlobals);
}
}

NightLight::NightLight(QObject *parent)
    : QObject(parent)
{
    readFile();
    askKWin();
}

int NightLight::clampTemperature(int kelvin)
{
    const int stepped = qRound(kelvin / 100.0) * 100;
    return std::clamp(stepped, MinTemperature, MaxTemperature);
}

void NightLight::readFile()
{
    KConfig config = open();
    const KConfigGroup group(&config, QString::fromLatin1(Group));
    m_active = group.readEntry("Active", false);
    m_always = group.readEntry("Mode", ModeDarkLight) == ModeConstant;
    m_temperature = clampTemperature(group.readEntry("NightTemperature", 4500));
}

void NightLight::refresh()
{
    readFile();
    askKWin();
    Q_EMIT changed();
}

void NightLight::askKWin()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    auto message = QDBusMessage::createMethodCall(Service, Path, u"org.freedesktop.DBus.Properties"_s, u"GetAll"_s);
    message.setArguments({u"org.kde.KWin.NightLight"_s});
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(message, 2000), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
        w->deleteLater();
        const QDBusPendingReply<QVariantMap> reply = *w;
        if (reply.isError()) {
            // KWin isn't there (or has no Night Light plugin): the file is all
            // there is to go on.
            return;
        }
        const QVariantMap all = reply.value();
        m_available = all.value(u"available"_s, true).toBool();
        m_running = all.value(u"running"_s, false).toBool();
        Q_EMIT changed();
    });
}

bool NightLight::write(const QString &key, const QVariant &value)
{
    KConfig config = open();
    KConfigGroup group(&config, QString::fromLatin1(Group));
    group.writeEntry(key, value);
    // KConfig writes a new file and renames it over the old one, and tells
    // KWin (which watches kwinrc) over the session bus.
    if (!config.sync()) {
        m_error = tr("Settings couldn't save the Night Light settings.");
        Q_EMIT changed();
        return false;
    }
    m_error.clear();
    return true;
}

void NightLight::setActive(bool on)
{
    // The mode stays as it was set (sunrise and sunset when it never was).
    if (write(u"Active"_s, on)) {
        readFile();
        Q_EMIT changed();
    }
}

void NightLight::setSchedule(const QString &schedule)
{
    if (schedule != u"sun" && schedule != u"always") {
        return;
    }
    if (write(u"Mode"_s, schedule == u"always" ? ModeConstant : ModeDarkLight)) {
        readFile();
        Q_EMIT changed();
    }
}

void NightLight::setTemperature(int kelvin)
{
    if (write(u"NightTemperature"_s, clampTemperature(kelvin))) {
        readFile();
        Q_EMIT changed();
    }
}

void NightLight::preview(int kelvin)
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    auto message = QDBusMessage::createMethodCall(Service, Path, Service, u"preview"_s);
    message.setArguments({uint(clampTemperature(kelvin))});
    bus.send(message);
}

void NightLight::stopPreview()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    bus.send(QDBusMessage::createMethodCall(Service, Path, Service, u"stopPreview"_s));
}
