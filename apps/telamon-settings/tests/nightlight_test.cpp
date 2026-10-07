#include "nightlight.h"

#include <KConfig>
#include <KConfigGroup>

#include <QSignalSpy>
#include <QStandardPaths>
#include <QTest>

class NightLightTest : public QObject
{
    Q_OBJECT

private Q_SLOTS:
    void initTestCase()
    {
        // kwinrc under ~/.qttest/config, never the real one.
        QStandardPaths::setTestModeEnabled(true);
    }

    void init()
    {
        QFile::remove(QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation) + QStringLiteral("/kwinrc"));
    }

    void defaultsAreKWins()
    {
        NightLight n;
        QVERIFY(!n.active());
        QCOMPARE(n.schedule(), QStringLiteral("sun"));
        QCOMPARE(n.temperature(), 4500);
    }

    void changesGoToKwinrcTheWayPlasmaWritesThem()
    {
        NightLight n;
        QSignalSpy changed(&n, &NightLight::changed);
        n.setActive(true);
        n.setSchedule(QStringLiteral("always"));
        n.setTemperature(3650);
        QVERIFY(changed.count() >= 3);
        QVERIFY(n.active());
        QCOMPARE(n.schedule(), QStringLiteral("always"));
        QCOMPARE(n.temperature(), 3700);

        KConfig config(QStringLiteral("kwinrc"), KConfig::NoGlobals);
        const KConfigGroup group(&config, QStringLiteral("NightColor"));
        QVERIFY(group.readEntry("Active", false));
        // ColorCorrect::NightLightMode: 0 is Constant, 1 is DarkLight
        QCOMPARE(group.readEntry("Mode", -1), 0);
        QCOMPARE(group.readEntry("NightTemperature", -1), 3700);
        // the day temperature is left alone
        QVERIFY(!group.hasKey("DayTemperature"));

        n.setSchedule(QStringLiteral("sun"));
        config.reparseConfiguration();
        QCOMPARE(KConfigGroup(&config, QStringLiteral("NightColor")).readEntry("Mode", -1), 1);
        n.setActive(false);
        NightLight again;
        QVERIFY(!again.active());
        QCOMPARE(again.schedule(), QStringLiteral("sun"));
        QCOMPARE(again.temperature(), 3700);
    }

    void aBadScheduleIsIgnored()
    {
        NightLight n;
        n.setSchedule(QStringLiteral("whenever"));
        n.setSchedule(QString());
        QCOMPARE(n.schedule(), QStringLiteral("sun"));
        KConfig config(QStringLiteral("kwinrc"), KConfig::NoGlobals);
        QVERIFY(!KConfigGroup(&config, QStringLiteral("NightColor")).hasKey("Mode"));
    }

    void temperaturesStayInKwinsRange()
    {
        QCOMPARE(NightLight::clampTemperature(100), 1000);
        QCOMPARE(NightLight::clampTemperature(99999), 6500);
        QCOMPARE(NightLight::clampTemperature(-5), 1000);
        QCOMPARE(NightLight::clampTemperature(4549), 4500);
        QCOMPARE(NightLight::clampTemperature(4550), 4600);
        NightLight n;
        n.setTemperature(0);
        QCOMPARE(n.temperature(), 1000);
        n.setTemperature(1000000);
        QCOMPARE(n.temperature(), 6500);
    }

    void aValueFromTheFileThatIsOutOfRangeIsClamped()
    {
        {
            KConfig config(QStringLiteral("kwinrc"), KConfig::NoGlobals);
            KConfigGroup(&config, QStringLiteral("NightColor")).writeEntry("NightTemperature", 123456789);
            config.sync();
        }
        NightLight n;
        QCOMPARE(n.temperature(), 6500);
    }

    void withoutAnySessionBusNothingBreaks()
    {
        NightLight n;
        n.preview(3000);
        n.stopPreview();
        n.refresh();
        QVERIFY(n.available()); // the file is all there is to go on
        QVERIFY(!n.running());
    }
};

QTEST_GUILESS_MAIN(NightLightTest)
#include "nightlight_test.moc"
