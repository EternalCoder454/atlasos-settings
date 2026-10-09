// The old D-Bus identity of Settings, `net.eterneon.atlas.settings`, kept for
// this release: the Launcher and other programs that have not moved yet call
// `org.freedesktop.Application` there (Activate, ActivateAction) to raise the
// window or open a page. The calls end up where KDBusService's do: Activate
// and Open as `activateRequested(arguments, token)` (no arguments), and
// ActivateAction as `activateActionRequested(action, parameter, token)`, the
// same action and text KDBusService hands on, so the one reader of them,
// settings-registry's launch.rs (`action_args`), decides what they mean. The
// text of a call is never split into launch arguments here: a caller can't
// slip options (`--kcm`, `--search`) in through it.
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
    /// A plain activation (Activate, Open): raise the window. `arguments` is
    /// always empty.
    void activateRequested(const QStringList &arguments, const QString &activationToken);
    /// ActivateAction: `parameter` is the call's first parameter when it is
    /// text, else "".
    void activateActionRequested(const QString &action, const QString &parameter, const QString &activationToken);
};
