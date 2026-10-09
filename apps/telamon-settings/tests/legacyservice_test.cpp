// The old D-Bus identity (net.eterneon.atlas.settings): the calls the
// Launcher makes reach the same handler as the new name's. Run under
// dbus-run-session (a private session bus, never the user's).

#include "legacyservice.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusVariant>
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
        QSignalSpy actions(&service, &LegacyService::activateActionRequested);

        // Activate raises, with the token.
        QVariantMap platform{{u"activation-token"_s, u"tok1"_s}};
        call(u"Activate"_s, {platform});
        QTRY_COMPARE(spy.size(), 1);
        QCOMPARE(spy.at(0).at(0).toStringList(), QStringList{});
        QCOMPARE(spy.at(0).at(1).toString(), u"tok1"_s);

        // ActivateAction("open", [link]) reaches the one reader of actions
        // (launch.rs) as the action and the text, as on the new name.
        QVariantList params{QVariant::fromValue(QDBusVariant(u"network/wifi"_s))};
        call(u"ActivateAction"_s, {u"open"_s, params, QVariantMap{{u"activation-token"_s, u"tok2"_s}}});
        QTRY_COMPARE(actions.size(), 1);
        QCOMPARE(actions.at(0).at(0).toString(), u"open"_s);
        QCOMPARE(actions.at(0).at(1).toString(), u"network/wifi"_s);
        QCOMPARE(actions.at(0).at(2).toString(), u"tok2"_s);

        // The text is handed on whole, never split into launch arguments: a
        // caller can't slip --kcm or --search in through the old name.
        const QString hostile = u"--kcm kcm_fonts --args x --search y"_s;
        call(u"ActivateAction"_s, {u"open"_s, QVariantList{QVariant::fromValue(QDBusVariant(hostile))}, QVariantMap{}});
        QTRY_COMPARE(actions.size(), 2);
        QCOMPARE(actions.at(1).at(1).toString(), hostile);
        QCOMPARE(spy.size(), 1);

        // open-app is an action too (the Launcher uses it on either name).
        call(u"ActivateAction"_s, {u"open-app"_s, QVariantList{QVariant::fromValue(QDBusVariant(u"com.brave.Browser.desktop"_s))}, QVariantMap{}});
        QTRY_COMPARE(actions.size(), 3);
        QCOMPARE(actions.at(2).at(0).toString(), u"open-app"_s);

        // No parameter, or one that is not text: "", never a crash.
        call(u"ActivateAction"_s, {u"open"_s, QVariantList{}, QVariantMap{}});
        QTRY_COMPARE(actions.size(), 4);
        QCOMPARE(actions.at(3).at(1).toString(), QString());
        call(u"ActivateAction"_s, {u"open"_s, QVariantList{QVariant::fromValue(QDBusVariant(42))}, QVariantMap{}});
        QTRY_COMPARE(actions.size(), 5);
        QCOMPARE(actions.at(4).at(1).toString(), QString());

        // Open has no documents to open: a plain activation.
        call(u"Open"_s, {QStringList{u"file:///etc/passwd"_s}, QVariantMap{}});
        QTRY_COMPARE(spy.size(), 2);
        QCOMPARE(spy.at(1).at(0).toStringList(), QStringList{});
        QCOMPARE(actions.size(), 5);
        QTRY_COMPARE(m_replies.size(), 7);
        QVERIFY(!m_replies.contains(false));
    }

    // The object exports Activate, Open and ActivateAction and nothing else:
    // not the signals, and not QObject's own slots (deleteLater).
    void exportsOnlyTheApplicationInterface()
    {
        LegacyService service;
        if (!service.registerOnSessionBus()) {
            QSKIP("the old name is taken on this bus");
        }
        auto message = QDBusMessage::createMethodCall(QString::fromLatin1(LegacyService::ServiceName), QString::fromLatin1(LegacyService::ObjectPath), u"org.freedesktop.DBus.Introspectable"_s, u"Introspect"_s);
        auto *watcher = new QDBusPendingCallWatcher(m_client.asyncCall(message), this);
        QString xml;
        bool done = false;
        connect(watcher, &QDBusPendingCallWatcher::finished, this, [&](QDBusPendingCallWatcher *w) {
            QDBusPendingReply<QString> reply = *w;
            xml = reply.value();
            done = true;
            w->deleteLater();
        });
        QTRY_VERIFY(done);
        // Beside the standard Introspectable, Properties and Peer, whose
        // methods are not ours, there is only org.freedesktop.Application.
        const qsizetype begin = xml.indexOf(u"<interface name=\"org.freedesktop.Application\">"_s);
        QVERIFY2(begin >= 0, qPrintable(xml));
        const qsizetype end = xml.indexOf(u"</interface>"_s, begin);
        QVERIFY2(end > begin, qPrintable(xml));
        const QString ours = xml.mid(begin, end - begin);
        QCOMPARE(ours.count(u"<method"_s), 3);
        QVERIFY2(ours.contains(u"<method name=\"Activate\""_s), qPrintable(ours));
        QVERIFY2(ours.contains(u"<method name=\"Open\""_s), qPrintable(ours));
        QVERIFY2(ours.contains(u"<method name=\"ActivateAction\""_s), qPrintable(ours));
        QVERIFY2(!ours.contains(u"<signal"_s), qPrintable(ours));
        QVERIFY2(!xml.contains(u"deleteLater"_s), qPrintable(xml));
        QCOMPARE(xml.count(u"<interface name="_s), 4);
    }
};

QTEST_MAIN(LegacyServiceTest)
#include "legacyservice_test.moc"
