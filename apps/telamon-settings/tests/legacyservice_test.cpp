// The old D-Bus identity (net.eterneon.atlas.settings): the calls the
// Launcher makes reach the same handler as the new name's. Run under
// dbus-run-session (a private session bus, never the user's).

#include "legacyservice.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QSignalSpy>
#include <QTest>

using namespace Qt::StringLiterals;

class LegacyServiceTest : public QObject
{
    Q_OBJECT

    // A caller of its own (another connection), calling without blocking:
    // the service answers on this thread's event loop.
    QDBusConnection m_client{QDBusConnection::connectToBus(QDBusConnection::SessionBus, u"client"_s)};

    void call(const QString &method, const QList<QVariant> &args)
    {
        auto message = QDBusMessage::createMethodCall(QString::fromLatin1(LegacyService::ServiceName), QString::fromLatin1(LegacyService::ObjectPath), u"org.freedesktop.Application"_s, method);
        message.setArguments(args);
        auto *watcher = new QDBusPendingCallWatcher(m_client.asyncCall(message), this);
        connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
            m_replies << !w->isError();
            w->deleteLater();
        });
    }
    QList<bool> m_replies;

private Q_SLOTS:
    void initTestCase()
    {
        if (!QDBusConnection::sessionBus().isConnected()) {
            QSKIP("no private session bus (run under dbus-run-session)");
        }
    }

    void takesTheOldNameAndForwardsTheCalls()
    {
        LegacyService service;
        QVERIFY(service.registerOnSessionBus());
        QVERIFY(QDBusConnection::sessionBus().interface()->isServiceRegistered(QString::fromLatin1(LegacyService::ServiceName)));
        QSignalSpy spy(&service, &LegacyService::activateRequested);

        // Activate raises, with the token.
        QVariantMap platform{{u"activation-token"_s, u"tok1"_s}};
        call(u"Activate"_s, {platform});
        QTRY_COMPARE(spy.size(), 1);
        QCOMPARE(spy.at(0).at(0).toStringList(), QStringList{});
        QCOMPARE(spy.at(0).at(1).toString(), u"tok1"_s);

        // ActivateAction("open", [link]) opens the page, as launch arguments.
        QVariantList params{QVariant::fromValue(QDBusVariant(u"network/wifi"_s))};
        call(u"ActivateAction"_s, {u"open"_s, params, QVariantMap{}});
        QTRY_COMPARE(spy.size(), 2);
        QCOMPARE(spy.at(1).at(0).toStringList(), (QStringList{u"network"_s, u"wifi"_s}));

        // Other actions only raise.
        call(u"ActivateAction"_s, {u"open-app"_s, QVariantList{QVariant::fromValue(QDBusVariant(u"x"_s))}, QVariantMap{}});
        QTRY_COMPARE(spy.size(), 3);
        QCOMPARE(spy.at(2).at(0).toStringList(), QStringList{});

        // Open has no documents to open: a plain activation.
        call(u"Open"_s, {QStringList{u"file:///etc/passwd"_s}, QVariantMap{}});
        QTRY_COMPARE(spy.size(), 4);
        QCOMPARE(spy.at(3).at(0).toStringList(), QStringList{});
        QTRY_COMPARE(m_replies.size(), 4);
        QVERIFY(!m_replies.contains(false));
    }
};

QTEST_MAIN(LegacyServiceTest)
#include "legacyservice_test.moc"
