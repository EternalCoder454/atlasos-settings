#include "displaylogic.h"
#include "screenconfig.h"

#include <kscreen/backendmanager_p.h>

#include <QGuiApplication>
#include <QSignalSpy>
#include <QTest>

using namespace Qt::StringLiterals;

// Runs ScreenConfig against libkscreen's Fake backend with a fixed layout
// (fixtures/displays-two.json: a 3840x2160 screen at 1.7x with a 1920x1080
// one turned upright beside it), never against real screens.
class ScreenConfigTest : public QObject
{
    Q_OBJECT

    static QVariantMap output(const ScreenConfig &c, int id)
    {
        for (const QVariant &v : c.outputs()) {
            if (v.toMap().value(QStringLiteral("id")).toInt() == id) {
                return v.toMap();
            }
        }
        return {};
    }
    static QRect rect(const QVariantMap &o)
    {
        return QRect(o[QStringLiteral("x")].toInt(), o[QStringLiteral("y")].toInt(), o[QStringLiteral("width")].toInt(), o[QStringLiteral("height")].toInt());
    }

    // A fresh page backend, loaded, with a short countdown.
    std::unique_ptr<ScreenConfig> open(int seconds = 600, int intervalMs = 100)
    {
        auto c = std::make_unique<ScreenConfig>();
        c->setCountdown(seconds, intervalMs);
        c->refresh();
        const bool loaded = QTest::qWaitFor([&] { return c->loaded(); }, 5000);
        Q_ASSERT(loaded);
        Q_UNUSED(loaded)
        return c;
    }

private Q_SLOTS:
    void initTestCase()
    {
        const QByteArray file = qgetenv("ATLAS_FIXTURES") + "/displays-two.json";
        QVERIFY2(QFile::exists(QString::fromLocal8Bit(file)), file.constData());
        qputenv("ATLAS_SETTINGS_FAKE_DISPLAYS", file);
    }

    void cleanup()
    {
        // The Fake backend keeps what was set: the next test starts from the
        // file again.
        KScreen::BackendManager::instance()->shutdownBackend();
    }

    void showsTheScreensAsTheyAre()
    {
        auto c = open();
        QVERIFY(c->error().isEmpty());
        QCOMPARE(c->outputs().size(), 2);
        QVERIFY(c->perOutputScaling());
        QVERIFY(c->primarySupported());

        const QVariantMap a = output(*c, 1);
        QCOMPARE(a[u"label"_s].toString(), u"Dell U2723QE"_s);
        QCOMPARE(a[u"name"_s].toString(), u"DP-1"_s);
        QVERIFY(a[u"primary"_s].toBool());
        QCOMPARE(a[u"scalePercent"_s].toInt(), 170);
        QCOMPARE(a[u"pixelWidth"_s].toInt(), 3840);
        QCOMPARE(a[u"resolutionText"_s].toString(), u"3840 × 2160"_s);
        QCOMPARE(a[u"rateText"_s].toString(), u"60 Hz"_s);
        QCOMPARE(a[u"rotation"_s].toInt(), 0);
        // 3840x2160 at 1.7x
        QVERIFY(qAbs(a[u"width"_s].toInt() - 2259) <= 1);
        QVERIFY(qAbs(a[u"height"_s].toInt() - 1271) <= 1);

        const QVariantMap b = output(*c, 2);
        QCOMPARE(b[u"label"_s].toString(), u"Acer KA242Y"_s);
        QVERIFY(!b[u"primary"_s].toBool());
        // 1920x1080 turned upright is 1080x1920 on the desktop
        QCOMPARE(b[u"rotation"_s].toInt(), 90);
        QCOMPARE(b[u"width"_s].toInt(), 1080);
        QCOMPARE(b[u"height"_s].toInt(), 1920);
        QCOMPARE(b[u"pixelWidth"_s].toInt(), 1920);
        // the owner's layout is acceptable as it is
        QVERIFY(DisplayLogic::connected({rect(a), rect(b)}));

        // resolutions: largest first, the preferred one marked, the current one marked
        const QVariantList resolutions = a[u"resolutions"_s].toList();
        QCOMPARE(resolutions.first().toMap()[u"text"_s].toString(), u"3840 × 2160"_s);
        QCOMPARE(resolutions.first().toMap()[u"ratio"_s].toString(), u"16:9"_s);
        QVERIFY(resolutions.first().toMap()[u"recommended"_s].toBool());
        QVERIFY(resolutions.first().toMap()[u"current"_s].toBool());
        QVERIFY(!resolutions.last().toMap()[u"current"_s].toBool());
        QCOMPARE(a[u"rates"_s].toList().size(), 3); // 60, 59.94 and 30 at 4K
        QVERIFY(a[u"hdrSupported"_s].toBool()); // the fake lists HDR for the first screen
        QVERIFY(!b[u"hdrSupported"_s].toBool());
    }

    void aResolutionChangeAsksToKeepAndMovesTheNeighbour()
    {
        auto c = open();
        const QRect neighbour = rect(output(*c, 2));
        c->setResolution(1, 2560, 1440);
        QTRY_VERIFY(c->confirming());
        QVERIFY(c->revertSeconds() > 590);
        QTRY_COMPARE(output(*c, 1)[u"pixelWidth"_s].toInt(), 2560);
        // 144 Hz: 60 Hz exists at 3840x2160 but only 59.95 at 2560x1440
        QCOMPARE(output(*c, 1)[u"rateText"_s].toString(), u"144 Hz"_s);
        const QRect a = rect(output(*c, 1));
        const QRect b = rect(output(*c, 2));
        QVERIFY(a.width() < 2259);
        QVERIFY2(DisplayLogic::connected({a, b}), "the second screen follows the first one's edge");
        QCOMPARE(b.x(), a.x() + a.width());
        QVERIFY(b.x() < neighbour.x());
        c->keepChanges();
        QVERIFY(!c->confirming());
        QTest::qWait(150);
        QCOMPARE(output(*c, 1)[u"pixelWidth"_s].toInt(), 2560);
    }

    void unconfirmedChangesAreRevertedWhenTheTimeRunsOut()
    {
        auto c = open(3, 15);
        c->setScale(1, 2.0);
        QTRY_VERIFY(c->confirming());
        QTRY_COMPARE(output(*c, 1)[u"scalePercent"_s].toInt(), 200);
        QVERIFY(c->notice().isEmpty());
        // nobody answers: 3 steps of 15 ms
        QTRY_VERIFY_WITH_TIMEOUT(!c->confirming(), 3000);
        QTRY_COMPARE(output(*c, 1)[u"scalePercent"_s].toInt(), 170);
        QTRY_VERIFY(!c->notice().isEmpty());
        QVERIFY(DisplayLogic::connected({rect(output(*c, 1)), rect(output(*c, 2))}));
    }

    void revertingByHandPutsEverythingBack()
    {
        auto c = open();
        const QVariantMap before1 = output(*c, 1);
        const QVariantMap before2 = output(*c, 2);
        c->setRotation(2, 0);
        QTRY_VERIFY(c->confirming());
        QTRY_COMPARE(output(*c, 2)[u"rotation"_s].toInt(), 0);
        c->setResolution(1, 1920, 1080); // a second change while asking
        QTRY_COMPARE(output(*c, 1)[u"pixelWidth"_s].toInt(), 1920);
        QVERIFY(c->confirming());
        c->revertChanges();
        QTRY_VERIFY(!c->confirming());
        // back to how it was before the first change of the series
        QTRY_COMPARE(output(*c, 1)[u"pixelWidth"_s].toInt(), before1[u"pixelWidth"_s].toInt());
        QTRY_COMPARE(output(*c, 2)[u"rotation"_s].toInt(), before2[u"rotation"_s].toInt());
        QCOMPARE(rect(output(*c, 1)), rect(before1));
        QCOMPARE(rect(output(*c, 2)), rect(before2));
    }

    void scaleIsKeptToFivePercentSteps()
    {
        auto c = open();
        c->setScale(1, 9.0);
        QTRY_COMPARE(output(*c, 1)[u"scalePercent"_s].toInt(), 300);
        c->keepChanges();
        c->setScale(1, 0.01);
        QTRY_COMPARE(output(*c, 1)[u"scalePercent"_s].toInt(), 50);
        c->keepChanges();
        c->setScale(1, 1.73);
        QTRY_COMPARE(output(*c, 1)[u"scalePercent"_s].toInt(), 175);
        c->keepChanges();
    }

    void aBadRotationOrResolutionDoesNothing()
    {
        auto c = open();
        c->setRotation(1, 45);
        c->setResolution(1, 123, 456);
        c->setResolution(99, 1280, 720);
        c->setRefreshRate(1, 999);
        QTest::qWait(80);
        QVERIFY(!c->confirming());
        QCOMPARE(output(*c, 1)[u"pixelWidth"_s].toInt(), 3840);
        QCOMPARE(output(*c, 1)[u"rotation"_s].toInt(), 0);
    }

    void aRefreshRateChangeKeepsTheResolution()
    {
        auto c = open();
        c->setResolution(1, 2560, 1440);
        QTRY_VERIFY(c->confirming());
        c->keepChanges();
        QTRY_COMPARE(output(*c, 1)[u"rateText"_s].toString(), u"144 Hz"_s);
        c->setRefreshRate(1, 120.0);
        QTRY_VERIFY(c->confirming());
        QTRY_COMPARE(output(*c, 1)[u"rateText"_s].toString(), u"120 Hz"_s);
        QCOMPARE(output(*c, 1)[u"pixelWidth"_s].toInt(), 2560);
        c->keepChanges();
    }

    void thePrimaryScreenChangesWithoutAsking()
    {
        auto c = open();
        c->setPrimary(2);
        QTRY_VERIFY(output(*c, 2)[u"primary"_s].toBool());
        QVERIFY(!output(*c, 1)[u"primary"_s].toBool());
        QVERIFY(!c->confirming());
        c->setPrimary(1);
        QTRY_VERIFY(output(*c, 1)[u"primary"_s].toBool());
    }

    void dragsSnapAndNeverLeaveAGap()
    {
        auto c = open();
        const QRect a = rect(output(*c, 1));
        const QRect b = rect(output(*c, 2));
        // drag the portrait screen far away: the preview already says where it lands
        const QPoint to = c->snapped(2, 9000, 7000);
        QVERIFY(DisplayLogic::connected({a, QRect(to, b.size())}));
        QVERIFY(!DisplayLogic::overlapping({a, QRect(to, b.size())}));
        c->moveOutput(2, 9000, 7000);
        QTRY_VERIFY(c->confirming());
        QTRY_VERIFY(rect(output(*c, 2)) != b);
        const QRect moved = rect(output(*c, 2));
        QVERIFY(DisplayLogic::connected({rect(output(*c, 1)), moved}));
        // positions start at 0, 0
        QCOMPARE(qMin(rect(output(*c, 1)).x(), moved.x()), 0);
        QCOMPARE(qMin(rect(output(*c, 1)).y(), moved.y()), 0);
        // dropped on the other screen: pushed off it
        c->revertChanges();
        QTRY_VERIFY(!c->confirming());
        QTRY_COMPARE(rect(output(*c, 2)), b);
        c->moveOutput(2, 100, 100);
        QTRY_VERIFY(c->confirming());
        QVERIFY(!DisplayLogic::overlapping({rect(output(*c, 1)), rect(output(*c, 2))}));
        c->keepChanges();
    }

    void hdrIsOnlyOfferedToScreensThatHaveIt()
    {
        auto c = open();
        c->setHdr(2, true); // the second has none
        QTest::qWait(80);
        QVERIFY(!c->confirming());
        QVERIFY(!output(*c, 2)[u"hdr"_s].toBool());
        c->setHdr(1, true);
        QTRY_VERIFY(c->confirming());
        QTRY_VERIFY(output(*c, 1)[u"hdr"_s].toBool());
        c->keepChanges();
    }
};

QTEST_MAIN(ScreenConfigTest)
#include "screenconfig_test.moc"
