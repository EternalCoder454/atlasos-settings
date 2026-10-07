#include "soundlogic.h"

#include "safetext.h"

#include <QCoreApplication>
#include <QRegularExpression>

#include <algorithm>

namespace SoundLogic
{
int volumePercent(qint64 volume, qint64 normal)
{
    if (normal <= 0 || volume < 0) {
        return 0;
    }
    return int(qRound64(double(volume) * 100.0 / double(normal)));
}

qint64 volumeFromPercent(int percent, qint64 normal, int maxPercent)
{
    const int clamped = std::clamp(percent, 0, std::max(0, maxPercent));
    return qRound64(double(normal) * clamped / 100.0);
}

bool isMonitor(const QString &name, const QVariantMap &properties)
{
    return name.endsWith(QLatin1String(".monitor")) || properties.value(QStringLiteral("device.class")).toString() == QLatin1String("monitor");
}

bool isEventSound(const QVariantMap &properties)
{
    return properties.value(QStringLiteral("media.role")).toString() == QLatin1String("event");
}

QString deviceKind(const QString &formFactor, const QString &bus, const QString &name, bool input)
{
    const QString form = formFactor.toLower();
    const QString lowerName = name.toLower();
    if (form == QLatin1String("headphone")) {
        return QStringLiteral("headphones");
    }
    if (form == QLatin1String("headset") || form == QLatin1String("hands-free") || form == QLatin1String("handset")) {
        return QStringLiteral("headset");
    }
    if (form == QLatin1String("webcam")) {
        return QStringLiteral("webcam");
    }
    if (form == QLatin1String("microphone")) {
        return QStringLiteral("microphone");
    }
    if (form == QLatin1String("tv")) {
        return QStringLiteral("monitor");
    }
    if (lowerName.contains(QLatin1String("hdmi")) || lowerName.contains(QLatin1String("displayport"))) {
        return QStringLiteral("monitor");
    }
    if (bus == QLatin1String("bluetooth") || lowerName.startsWith(QLatin1String("bluez"))) {
        return QStringLiteral("bluetooth");
    }
    if (bus == QLatin1String("usb")) {
        return input ? QStringLiteral("microphone") : QStringLiteral("usb");
    }
    if (form == QLatin1String("speaker") || form == QLatin1String("internal") || form == QLatin1String("computer") || form == QLatin1String("hifi") || form == QLatin1String("portable")) {
        return input ? QStringLiteral("microphone") : QStringLiteral("speaker");
    }
    return input ? QStringLiteral("microphone") : QStringLiteral("other");
}

QString appName(const QString &clientName, const QVariantMap &streamProperties)
{
    for (const QString &candidate : {streamProperties.value(QStringLiteral("application.name")).toString(),
                                     clientName,
                                     streamProperties.value(QStringLiteral("node.name")).toString(),
                                     streamProperties.value(QStringLiteral("media.name")).toString()}) {
        const QString text = TelamonText::safeText(candidate, 60);
        if (!text.isEmpty()) {
            return text;
        }
    }
    return QCoreApplication::translate("SoundLogic", "Unknown App");
}

QString appIcon(const QVariantMap &streamProperties)
{
    const QString name = streamProperties.value(QStringLiteral("application.icon_name")).toString();
    // A theme icon's name, never a path.
    static const QRegularExpression valid(QStringLiteral("^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"));
    return valid.match(name).hasMatch() ? name : QString();
}
}
