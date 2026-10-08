// Launcher::runApplication: starts an installed application from its desktop
// file, the first of the names that exists, and says "" when none does. The
// desktop file is made here, in a private XDG_DATA_HOME, and its program only
// writes a marker file. Run under dbus-run-session like the other tests.

#include "launcher.h"

#include <QDir>
#include <QFile>
#include <QSignalSpy>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QTest>

using namespace Qt::StringLiterals;

class LauncherTest : public QObject
{
    Q_OBJECT

    QTemporaryDir m_home;
    QString m_marker;

    int markerLines() const
    {
        QFile f(m_marker);
        return f.open(QIODevice::ReadOnly) ? f.readAll().count('\n') : 0;
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY(m_home.isValid());
        qputenv("XDG_DATA_HOME", m_home.path().toUtf8());
        qputenv("XDG_DATA_DIRS", m_home.path().toUtf8());
        qputenv("XDG_CACHE_HOME", m_home.path().toUtf8() + "/cache");
        qputenv("XDG_CONFIG_HOME", m_home.path().toUtf8() + "/config");
        m_marker = m_home.path() + u"/started"_s;
        QVERIFY(QDir().mkpath(m_home.path() + u"/applications"_s));
        QFile desktop(m_home.path() + u"/applications/net.eterneon.test.camera.desktop"_s);
        QVERIFY(desktop.open(QIODevice::WriteOnly));
        desktop.write("[Desktop Entry]\nType=Application\nName=Test Camera\nExec=/bin/sh -c 'echo x >> " + m_marker.toUtf8() + "'\n");
        desktop.close();
        for (int i = 0; i < 6; ++i) {
            QFile app(m_home.path() + u"/applications/net.eterneon.test.many%1.desktop"_s.arg(i));
            QVERIFY(app.open(QIODevice::WriteOnly));
            app.write("[Desktop Entry]\nType=Application\nName=Many\nExec=/bin/true\n");
        }
        // A link, not an application.
        QFile link(m_home.path() + u"/applications/net.eterneon.test.link.desktop"_s);
        QVERIFY(link.open(QIODevice::WriteOnly));
        link.write("[Desktop Entry]\nType=Link\nName=Link\nURL=https://example.org\n");
        link.close();
    }

    void nothingInstalledStartsNothingAndSaysSo()
    {
        Launcher launcher;
        QSignalSpy failed(&launcher, &Launcher::failed);
        QCOMPARE(launcher.runApplication({}), QString());
        QCOMPARE(launcher.runApplication({u"net.eterneon.test.missing"_s}), QString());
        // not desktop file IDs: skipped, never looked up as paths
        QCOMPARE(launcher.runApplication({u"../applications/net.eterneon.test.camera"_s, u"/bin/sh"_s, u"a b"_s, QString()}), QString());
        // not an application
        QCOMPARE(launcher.runApplication({u"net.eterneon.test.link"_s}), QString());
        QCOMPARE(failed.count(), 0);
        QCOMPARE(markerLines(), 0);
    }

    void startsTheFirstInstalledOne()
    {
        Launcher launcher;
        QSignalSpy failed(&launcher, &Launcher::failed);
        QCOMPARE(launcher.runApplication({u"net.eterneon.test.missing"_s, u"net.eterneon.test.camera"_s, u"net.eterneon.test.link"_s}), u"net.eterneon.test.camera"_s);
        QTRY_COMPARE_WITH_TIMEOUT(markerLines(), 1, 10000);
        QCOMPARE(failed.count(), 0);
        // the same again at once (a double click) starts nothing more
        QCOMPARE(launcher.runApplication({u"net.eterneon.test.camera"_s}), u"net.eterneon.test.camera"_s);
        QTest::qWait(500);
        QCOMPARE(markerLines(), 1);
    }

    void tooManyStartsAreRefusedAndNotReportedAsStarted()
    {
        Launcher launcher;
        QSignalSpy failed(&launcher, &Launcher::failed);
        for (int i = 0; i < 5; ++i) {
            const QString name = u"net.eterneon.test.many%1"_s.arg(i);
            QCOMPARE(launcher.runApplication({name}), name);
        }
        QCOMPARE(failed.count(), 0);
        QCOMPARE(launcher.runApplication({u"net.eterneon.test.many5"_s}), QString());
        QCOMPARE(failed.count(), 1);
    }
};

QTEST_MAIN(LauncherTest)
#include "launcher_test.moc"
