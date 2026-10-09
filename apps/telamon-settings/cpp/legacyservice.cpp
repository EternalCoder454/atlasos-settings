#include "legacyservice.h"

#include "actionparam.h"

#include <QDBusConnection>
#include <QDBusError>
#include <QDBusVariant>
#include <QDebug>

LegacyService::LegacyService(QObject *parent)
    : QObject(parent)
{
}

bool LegacyService::registerOnSessionBus()
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return false;
    }
    if (!bus.registerObject(QString::fromLatin1(ObjectPath), this, QDBusConnection::ExportAllSlots)) {
        qWarning() << "the old D-Bus object of Settings could not be exported:" << bus.lastError().message();
        return false;
    }
    if (!bus.registerService(QString::fromLatin1(ServiceName))) {
        qInfo() << "the old D-Bus name of Settings is taken (an older Settings runs?)";
        return false;
    }
    return true;
}

static QString tokenOf(const QVariantMap &platformData)
{
    return platformData.value(QStringLiteral("activation-token")).toString();
}

void LegacyService::Activate(const QVariantMap &platformData)
{
    Q_EMIT activateRequested({}, tokenOf(platformData));
}

void LegacyService::Open(const QStringList &, const QVariantMap &platformData)
{
    // Settings opens no documents: a plain activation.
    Q_EMIT activateRequested({}, tokenOf(platformData));
}

void LegacyService::ActivateAction(const QString &actionName, const QVariantList &parameter, const QVariantMap &platformData)
{
    // "open" with a link such as "displays" or "network/wifi" (the Launcher's
    // deep link), "open-app" with a desktop file ID; anything else only
    // raises the window. Which of them is well formed is read in Rust, as for
    // the new name; nothing is split or interpreted here.
    Q_EMIT activateActionRequested(actionName, actionParameter(QVariant(parameter)), tokenOf(platformData));
}
