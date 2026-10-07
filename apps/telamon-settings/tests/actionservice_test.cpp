// org.freedesktop.Application.ActivateAction as the Launcher calls it, on the
// name KDBusService takes (main.cpp connects activateActionRequested to the
// Rust side). Run under dbus-run-session (a private session bus, never the
// user's).

#include "actionparam.h"

#include <KDBusService>

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusVariant>
#include <QSignalSpy>
#include <QTest>

using namespace Qt::StringLiterals;

class ActionServiceTest : public QObject
{
    Q_OBJECT

    QDBusConnection m_client{QDBusConnection::connectToBus(QDBusConnection::SessionBus, u"client"_s)};
    QList<bool> m_replies;

    // The service is the first of its name: what KDBusService makes of the
    // organization domain and application name.
    static QString serviceName()
    {
        return u"net.eterneon.actionservicetest"_s;
    }

    void call(const QString &method, const QList<QVariant> &args)
    {
        auto message = QDBusMessage::createMethodCall(serviceName(), u"/net/eterneon/actionservicetest"_s, u"org.freedesktop.Application"_s, method);
        message.setArguments(args);
        auto *watcher = new QDBusPendingCallWatcher(m_client.asyncCall(message), this);
        connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *w) {
            m_replies << !w->isError();
            w->deleteLater();
        });
    }

    static QVariantList param(const QVariant &value)
    {
        return {QVariant::fromValue(QDBusVariant(value))};
    }

private Q_SLOTS:
    void initTestCase()
    {
        if (!QDBusConnection::sessionBus().isConnected()) {
            QSKIP("no private session bus (run under dbus-run-session)");
        }
        QCoreApplication::setOrganizationDomain(u"eterneon.net"_s);
        QCoreApplication::setApplicationName(u"actionservicetest"_s);
    }

    void actionsReachTheHandler()
    {
        KDBusService service(KDBusService::Unique | KDBusService::NoExitOnFailure);
        QVERIFY(QDBusConnection::sessionBus().interface()->isServiceRegistered(serviceName()));
        QSignalSpy spy(&service, &KDBusService::activateActionRequested);
        QSignalSpy plain(&service, &KDBusService::activateRequested);

        call(u"ActivateAction"_s, {u"open"_s, param(u"displays/night-light"_s), QVariantMap{{u"activation-token"_s, u"tok"_s}}});
        QTRY_COMPARE(spy.size(), 1);
        QCOMPARE(spy.at(0).at(0).toString(), u"open"_s);
        QCOMPARE(actionParameter(spy.at(0).at(1).value<QVariant>()), u"displays/night-light"_s);

        call(u"ActivateAction"_s, {u"open-app"_s, param(u"com.brave.Browser.desktop"_s), QVariantMap{}});
        QTRY_COMPARE(spy.size(), 2);
        QCOMPARE(spy.at(1).at(0).toString(), u"open-app"_s);
        QCOMPARE(actionParameter(spy.at(1).at(1).value<QVariant>()), u"com.brave.Browser.desktop"_s);

        // No parameter, and one that is not text: an empty string, never a crash.
        call(u"ActivateAction"_s, {u"open"_s, QVariantList{}, QVariantMap{}});
        QTRY_COMPARE(spy.size(), 3);
        QCOMPARE(actionParameter(spy.at(2).at(1).value<QVariant>()), QString());
        call(u"ActivateAction"_s, {u"open"_s, param(42), QVariantMap{}});
        QTRY_COMPARE(spy.size(), 4);
        QCOMPARE(actionParameter(spy.at(3).at(1).value<QVariant>()), QString());

        // The plain Activate is a different signal.
        call(u"Activate"_s, {QVariantMap{}});
        QTRY_COMPARE(plain.size(), 1);
        QCOMPARE(spy.size(), 4);

        QTRY_COMPARE(m_replies.size(), 5);
        QVERIFY(!m_replies.contains(false));
    }

    void parameterUnwrapping()
    {
        QCOMPARE(actionParameter(QVariant(u"x"_s)), u"x"_s);
        QCOMPARE(actionParameter(QVariant::fromValue(QDBusVariant(u"x"_s))), u"x"_s);
        QCOMPARE(actionParameter(QVariant(QVariantList{QVariant::fromValue(QDBusVariant(u"x"_s))})), u"x"_s);
        QCOMPARE(actionParameter(QVariant(QVariantList{QVariant(QVariantList{QVariant(u"deep"_s)})})), u"deep"_s);
        QCOMPARE(actionParameter(QVariant()), QString());
        QCOMPARE(actionParameter(QVariant(QVariantList{})), QString());
        QCOMPARE(actionParameter(QVariant(7)), QString());
        QCOMPARE(actionParameter(QVariant(QStringList{u"a"_s})), QString());
        // Nesting past the bound is not followed.
        QVariant nested = u"x"_s;
        for (int i = 0; i < 20; ++i) {
            nested = QVariant(QVariantList{nested});
        }
        QCOMPARE(actionParameter(nested), QString());
    }
};

QTEST_MAIN(ActionServiceTest)
#include "actionservice_test.moc"
