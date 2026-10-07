// The old D-Bus identity of Settings, `net.eterneon.atlas.settings`, kept for
// this release: the Launcher and other programs that have not moved yet call
// `org.freedesktop.Application` there (Activate, ActivateAction) to raise the
// window or open a page. The calls end up where KDBusService's do, as the
// same `activateRequested(arguments, token)`; what they carry is read in Rust
// like any launch argument (settings-registry's launch.rs).
#pragma once

#include <QObject>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>

class LegacyService : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.freedesktop.Application")

public:
    explicit LegacyService(QObject *parent = nullptr);

    /// Takes the old name and exports the interface; false (and a log line)
    /// when there is no session bus or an older Settings owns the name.
    bool registerOnSessionBus();

    static constexpr auto ServiceName = "net.eterneon.atlas.settings";
    static constexpr auto ObjectPath = "/net/eterneon/atlas/settings";

public Q_SLOTS:
    void Activate(const QVariantMap &platformData);
    void Open(const QStringList &uris, const QVariantMap &platformData);
    void ActivateAction(const QString &actionName, const QVariantList &parameter, const QVariantMap &platformData);

Q_SIGNALS:
    /// `arguments` as a launch would have them after the program name.
    void activateRequested(const QStringList &arguments, const QString &activationToken);
};
