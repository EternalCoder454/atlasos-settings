// Tests for the KConfig-backed settings: each writes into a temporary
// XDG_CONFIG_HOME (never the real one) and checks the exact keys and values
// in the files, as the KCMs write them.

#include "../cpp/powerconfig.h"
#include "../cpp/screenlockconfig.h"

#include <QFile>
#include <QTemporaryDir>
#include <QtTest>

using namespace Qt::StringLiterals;

class ConfigTests : public QObject
{
    Q_OBJECT

    QTemporaryDir m_dir;

    QString path(const QString &name) const
    {
        return m_dir.filePath(name);
    }

    // The file's lines, without blanks, in order.
    QStringList lines(const QString &name) const
    {
        QFile f(path(name));
        if (!f.open(QIODevice::ReadOnly)) {
            return {};
        }
        QStringList out;
        for (const QString &l : QString::fromUtf8(f.readAll()).split(u'\n')) {
            if (!l.trimmed().isEmpty()) {
                out << l;
            }
        }
        return out;
    }

    void put(const QString &name, const QByteArray &text) const
    {
        QFile f(path(name));
        QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Truncate));
        f.write(text);
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY(m_dir.isValid());
        qputenv("XDG_CONFIG_HOME", m_dir.path().toUtf8());
        qunsetenv("XDG_CONFIG_DIRS");
        QStandardPaths::setTestModeEnabled(false);
    }

    void init()
    {
        QFile::remove(path(u"powerdevilrc"_s));
        QFile::remove(path(u"kscreenlockerrc"_s));
    }

    void powerDefaultsWithNoFile()
    {
        PowerConfig c;
        QVariantMap ac = c.read(u"AC"_s);
        QCOMPARE(ac[u"screenOff"_s].toInt(), 600);
        QCOMPARE(ac[u"sleep"_s].toInt(), 900);
        QCOMPARE(ac[u"lid"_s].toInt(), 1);
        QCOMPARE(c.read(u"Battery"_s)[u"sleep"_s].toInt(), 600);
        QVERIFY(c.read(u"Nonsense"_s).isEmpty());
    }

    void powerReadsWhatThePowerKcmWrote()
    {
        // The file as kcm_powerdevilprofilesconfig left it for a desktop.
        put(u"powerdevilrc"_s,
            "[AC][Display]\nDimDisplayIdleTimeoutSec=-1\nDimDisplayWhenIdle=false\n"
            "TurnOffDisplayIdleTimeoutSec=-1\nTurnOffDisplayWhenIdle=false\n\n"
            "[AC][SuspendAndShutdown]\nAutoSuspendAction=0\nPowerButtonAction=8\n");
        PowerConfig c;
        const QVariantMap ac = c.read(u"AC"_s);
        QCOMPARE(ac[u"screenOff"_s].toInt(), 0);
        QCOMPARE(ac[u"sleep"_s].toInt(), 0);
        QCOMPARE(ac[u"powerButton"_s].toInt(), 8);
    }

    void powerWritesTheKeysTheKcmWrites()
    {
        put(u"powerdevilrc"_s, "[Other]\nKeep=me\n");
        PowerConfig c;
        QVERIFY(c.setScreenOff(u"AC"_s, 300));
        QVERIFY(c.setSleep(u"AC"_s, 1800));
        QVERIFY(c.setLid(u"AC"_s, 2));
        QVERIFY(c.setPowerButton(u"AC"_s, 64));
        const QStringList l = lines(u"powerdevilrc"_s);
        QVERIFY2(l.contains(u"[AC][Display]"_s), qPrintable(l.join(u'|')));
        QVERIFY(l.contains(u"TurnOffDisplayWhenIdle=true"_s));
        QVERIFY(l.contains(u"TurnOffDisplayIdleTimeoutSec=300"_s));
        QVERIFY(l.contains(u"[AC][SuspendAndShutdown]"_s));
        QVERIFY(l.contains(u"AutoSuspendAction=1"_s));
        QVERIFY(l.contains(u"AutoSuspendIdleTimeoutSec=1800"_s));
        QVERIFY(l.contains(u"LidAction=2"_s));
        QVERIFY(l.contains(u"PowerButtonAction=64"_s));
        // What was there stays.
        QVERIFY(l.contains(u"[Other]"_s));
        QVERIFY(l.contains(u"Keep=me"_s));

        const QVariantMap ac = c.read(u"AC"_s);
        QCOMPARE(ac[u"screenOff"_s].toInt(), 300);
        QCOMPARE(ac[u"sleep"_s].toInt(), 1800);
        QCOMPARE(ac[u"lid"_s].toInt(), 2);
        QCOMPARE(ac[u"powerButton"_s].toInt(), 64);
    }

    void powerNeverIsTheKcmsNever()
    {
        PowerConfig c;
        QVERIFY(c.setScreenOff(u"AC"_s, 0));
        QVERIFY(c.setSleep(u"AC"_s, 0));
        const QStringList l = lines(u"powerdevilrc"_s);
        QVERIFY(l.contains(u"TurnOffDisplayWhenIdle=false"_s));
        QVERIFY(l.contains(u"TurnOffDisplayIdleTimeoutSec=-1"_s));
        QVERIFY(l.contains(u"AutoSuspendAction=0"_s));
        QCOMPARE(c.read(u"AC"_s)[u"screenOff"_s].toInt(), 0);
        QCOMPARE(c.read(u"AC"_s)[u"sleep"_s].toInt(), 0);
    }

    void powerKeepsHibernate()
    {
        put(u"powerdevilrc"_s, "[Battery][SuspendAndShutdown]\nAutoSuspendAction=2\n");
        PowerConfig c;
        QVERIFY(c.setSleep(u"Battery"_s, 600));
        QVERIFY(lines(u"powerdevilrc"_s).contains(u"AutoSuspendAction=2"_s));
    }

    void powerProfilesAreIndependent()
    {
        PowerConfig c;
        QVERIFY(c.setSleep(u"Battery"_s, 300));
        QCOMPARE(c.read(u"Battery"_s)[u"sleep"_s].toInt(), 300);
        QCOMPARE(c.read(u"AC"_s)[u"sleep"_s].toInt(), 900);
        QVERIFY(lines(u"powerdevilrc"_s).contains(u"[Battery][SuspendAndShutdown]"_s));
    }

    void powerRefusesNonsense()
    {
        PowerConfig c;
        QVERIFY(!c.setScreenOff(u"AC"_s, -5));
        QVERIFY(!c.setScreenOff(u"AC"_s, 10 * 24 * 3600));
        QVERIFY(!c.setScreenOff(u"../AC"_s, 60));
        QVERIFY(!c.setLid(u"AC"_s, 12345));
        QVERIFY(!c.setPowerButton(u"AC"_s, 2));
        QVERIFY(lines(u"powerdevilrc"_s).isEmpty());
    }

    void powerClampsWhatTheFileSays()
    {
        put(u"powerdevilrc"_s, "[AC][Display]\nTurnOffDisplayIdleTimeoutSec=99999999\n");
        PowerConfig c;
        QCOMPARE(c.read(u"AC"_s)[u"screenOff"_s].toInt(), 24 * 3600);
    }

    void screenLockDefaults()
    {
        ScreenLockConfig c;
        const QVariantMap m = c.read();
        QCOMPARE(m[u"autoLock"_s].toBool(), true);
        QCOMPARE(m[u"minutes"_s].toInt(), 5);
        QCOMPARE(m[u"onWake"_s].toBool(), true);
    }

    void screenLockReadsWhatTheKcmWrote()
    {
        put(u"kscreenlockerrc"_s, "[Daemon]\nAutolock=false\nTimeout=0\n");
        ScreenLockConfig c;
        const QVariantMap m = c.read();
        QCOMPARE(m[u"autoLock"_s].toBool(), false);
        QCOMPARE(m[u"minutes"_s].toInt(), 0);
    }

    void screenLockWritesTheKcmsKeys()
    {
        put(u"kscreenlockerrc"_s, "[Greeter]\nWallpaperPlugin=x\n");
        ScreenLockConfig c;
        QVERIFY(c.setLockAfter(10));
        QVERIFY(c.setLockOnWake(false));
        const QStringList l = lines(u"kscreenlockerrc"_s);
        QVERIFY(l.contains(u"[Daemon]"_s));
        QVERIFY(l.contains(u"Autolock=true"_s));
        QVERIFY(l.contains(u"Timeout=10"_s));
        QVERIFY(l.contains(u"LockOnResume=false"_s));
        QVERIFY(l.contains(u"WallpaperPlugin=x"_s));
        QCOMPARE(c.read()[u"minutes"_s].toInt(), 10);
        QCOMPARE(c.read()[u"onWake"_s].toBool(), false);

        QVERIFY(c.setLockAfter(0));
        QVERIFY(lines(u"kscreenlockerrc"_s).contains(u"Autolock=false"_s));
        QCOMPARE(c.read()[u"autoLock"_s].toBool(), false);
        QVERIFY(!c.setLockAfter(-1));
        QVERIFY(!c.setLockAfter(1000000));
    }
};

QTEST_GUILESS_MAIN(ConfigTests)
#include "configtests.moc"
