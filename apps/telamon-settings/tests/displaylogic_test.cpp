#include "displaylogic.h"
#include "reverttimer.h"
#include "safetext.h"

#include <QSignalSpy>
#include <QTest>

using namespace DisplayLogic;
using TelamonText::safeText;
using namespace Qt::StringLiterals;

class DisplayLogicTest : public QObject
{
    Q_OBJECT

private:
    static QList<Mode> modes()
    {
        return {
            {u"a"_s, {3840, 2160}, 60.0},
            {u"b"_s, {3840, 2160}, 59.94},
            {u"c"_s, {2560, 1440}, 144.0},
            {u"d"_s, {2560, 1440}, 60.0},
            {u"e"_s, {2560, 1440}, 143.9995}, // the same rate as c
            {u"f"_s, {1920, 1080}, 50.0},
            {u"g"_s, {2560, 1080}, 100.0},
        };
    }

private Q_SLOTS:
    void scaleStepsAreWholeFivePercent()
    {
        QCOMPARE(clampScalePercent(1.7), 170);
        QCOMPARE(clampScalePercent(1.72), 170);
        QCOMPARE(clampScalePercent(1.73), 175);
        QCOMPARE(clampScalePercent(0.1), MinScalePercent);
        QCOMPARE(clampScalePercent(9), MaxScalePercent);
        QCOMPARE(clampScalePercent(-3), MinScalePercent);
        QCOMPARE(clampScalePercent(std::nan("")), 100);
        // every step is exact in 1/120, as the Wayland protocol counts
        for (int p = MinScalePercent; p <= MaxScalePercent; p += ScaleStepPercent) {
            const double s = scaleFromPercent(p);
            QVERIFY2(qFuzzyCompare(s * 120.0, std::round(s * 120.0)), qPrintable(QString::number(p)));
            QCOMPARE(qRound(s * 100.0), p);
            QCOMPARE(clampScalePercent(s), p);
        }
        QCOMPARE(scaleFromPercent(170), 1.7);
        QCOMPARE(scaleText(1.7), u"170%"_s);
        QCOMPARE(scaleText(0.5), u"50%"_s);
    }

    void resolutionsAreUniqueAndLargestFirst()
    {
        const QList<QSize> sizes = resolutions(modes());
        QCOMPARE(sizes, (QList<QSize>{{3840, 2160}, {2560, 1440}, {2560, 1080}, {1920, 1080}}));
        QVERIFY(resolutions({}).isEmpty());
    }

    void refreshRatesAreUniqueAndFastestFirst()
    {
        QCOMPARE(refreshRates(modes(), {2560, 1440}), (QList<double>{144.0, 60.0}));
        QCOMPARE(refreshRates(modes(), {3840, 2160}), (QList<double>{60.0, 59.94}));
        QVERIFY(refreshRates(modes(), {800, 600}).isEmpty());
    }

    void aResolutionChangeKeepsTheRateWhenItCan()
    {
        // 60 Hz exists at 2560x1440: kept
        QCOMPARE(modeFor(modes(), {2560, 1440}, 60.0), u"d"_s);
        // 59.94 doesn't at 2560x1440: the fastest
        QCOMPARE(modeFor(modes(), {2560, 1440}, 59.94), u"c"_s);
        // one mode of that size
        QCOMPARE(modeFor(modes(), {1920, 1080}, 144.0), u"f"_s);
        // a size no mode has
        QCOMPARE(modeFor(modes(), {800, 600}, 60.0), QString());
        // a rate of 0 (no current mode) picks the fastest
        QCOMPARE(modeFor(modes(), {3840, 2160}, 0), u"a"_s);
    }

    void aRateChangePicksTheNearestMode()
    {
        QCOMPARE(modeAtRate(modes(), {2560, 1440}, 60.0), u"d"_s);
        QCOMPARE(modeAtRate(modes(), {2560, 1440}, 144.0), u"c"_s);
        QCOMPARE(modeAtRate(modes(), {2560, 1440}, 100.0), u"d"_s);
        QCOMPARE(modeAtRate(modes(), {800, 600}, 60.0), QString());
    }

    void textsAreReadable()
    {
        QCOMPARE(resolutionText({3840, 2160}), u"3840 × 2160 (16:9)"_s);
        QCOMPARE(resolutionText({3840, 2160}, false), u"3840 × 2160"_s);
        QCOMPARE(resolutionText({1920, 1200}), u"1920 × 1200 (16:10)"_s);
        QCOMPARE(resolutionText({2560, 1080}), u"2560 × 1080 (21:9)"_s);
        QCOMPARE(resolutionText({3440, 1440}), u"3440 × 1440 (21:9)"_s);
        QCOMPARE(resolutionText({1280, 1024}), u"1280 × 1024 (5:4)"_s);
        QCOMPARE(resolutionText({1366, 768}), u"1366 × 768 (16:9)"_s);
        QCOMPARE(resolutionText({1000, 777}), u"1000 × 777"_s);
        QCOMPARE(resolutionText({0, 0}), u"0 × 0"_s);
        QCOMPARE(rateText(60.0), u"60 Hz"_s);
        QCOMPARE(rateText(59.94), u"59.94 Hz"_s);
        QCOMPARE(rateText(59.9), u"59.9 Hz"_s);
        QCOMPARE(rateText(143.9998), u"144 Hz"_s);
        QCOMPARE(rateText(143.98), u"143.98 Hz"_s);
    }

    void namesAreMadeSafe()
    {
        QCOMPARE(safeText(u"  Dell\u0007  U2723QE\n"_s), u"Dell U2723QE"_s);
        QCOMPARE(safeText(u"A"_s + QChar(0x202E) + u"B"_s + QChar(0x200B) + u"C"_s), u"ABC"_s); // bidi override, zero width space
        QCOMPARE(safeText(QString(500, u'x'), 20).size(), 20);
        QCOMPARE(safeText(u"\u0000\u0001"_s), QString());
        QCOMPARE(safeText(u"<b>x</b>"_s), u"<b>x</b>"_s); // shown as plain text, not stripped
    }

    void aScreenSnapsToTheEdgeOfAnother()
    {
        const QList<QRect> others{{0, 0, 2000, 1200}};
        const QSize size(1080, 1920);
        // near its right edge, a bit low: beside it, top aligned
        QCOMPARE(snapPosition(others, size, {2030, 40}), QPoint(2000, 0));
        // far to the right and far down: still touching, never floating
        const QPoint far = snapPosition(others, size, {5000, 4000});
        QVERIFY(connected({others[0], QRect(far, size)}));
        QVERIFY(!overlapping({others[0], QRect(far, size)}));
        // dropped on top of it: pushed out along the nearest side
        const QPoint over = snapPosition(others, size, {900, 100});
        QVERIFY(!overlapping({others[0], QRect(over, size)}));
        QVERIFY(connected({others[0], QRect(over, size)}));
        // left of it, bottom aligned
        QCOMPARE(snapPosition(others, size, {-1100, -700}), QPoint(-1080, 1200 - 1920));
        // above it, centred
        QCOMPARE(snapPosition({{0, 0, 1920, 1080}}, QSize(1000, 500), {430, -520}), QPoint(460, -500));
        // alone: where it was dropped
        QCOMPARE(snapPosition({}, size, {17, 23}), QPoint(17, 23));
    }

    void aScreenNeverSnapsOverAThird()
    {
        const QList<QRect> others{{0, 0, 1920, 1080}, {1920, 0, 1920, 1080}};
        const QSize size(1920, 1080);
        for (int x = -2500; x < 6000; x += 331) {
            for (int y = -1500; y < 2500; y += 277) {
                const QPoint p = snapPosition(others, size, {x, y});
                const QRect moved(p, size);
                QVERIFY2(!moved.intersects(others[0]) && !moved.intersects(others[1]), qPrintable(QStringLiteral("%1,%2").arg(x).arg(y)));
                QVERIFY(connected({others[0], others[1], moved}));
            }
        }
    }

    void connectedMeansSharedEdges()
    {
        QVERIFY(connected({}));
        QVERIFY(connected({{0, 0, 100, 100}}));
        QVERIFY(connected({{0, 0, 100, 100}, {100, 20, 50, 50}}));
        QVERIFY(connected({{0, 0, 100, 100}, {100, 0, 100, 100}, {0, 100, 100, 100}}));
        QVERIFY(!connected({{0, 0, 100, 100}, {101, 0, 100, 100}})); // a one pixel gap
        QVERIFY(!connected({{0, 0, 100, 100}, {100, 100, 100, 100}})); // a lone corner
        QVERIFY(!connected({{0, 0, 100, 100}, {100, 0, 100, 100}, {500, 500, 10, 10}}));
        QVERIFY(overlapping({{0, 0, 100, 100}, {99, 0, 100, 100}}));
        QVERIFY(!overlapping({{0, 0, 100, 100}, {100, 0, 100, 100}}));
    }

    void theTopLeftBecomesTheOrigin()
    {
        QCOMPARE(originOffset({}), QPoint());
        QCOMPARE(originOffset({{-100, 50, 10, 10}, {20, -30, 10, 10}}), QPoint(-100, -30));
        QCOMPARE(originOffset({{0, 0, 10, 10}, {10, 0, 10, 10}}), QPoint(0, 0));
    }

    void screensFollowAnEdgeThatMoves()
    {
        // 4K at 1.7x (2259x1270) with a portrait screen to its right
        const QList<QRect> screens{{0, 0, 2259, 1270}, {2259, 0, 1080, 1920}};
        // the first gets smaller (1.0x -> 2560x1440 at 1.7x is 1506x847)
        const QList<QPoint> places = afterResize(screens, 0, {2259, 1270}, {1506, 847});
        QCOMPARE(places[0], QPoint(0, 0));
        QCOMPARE(places[1], QPoint(1506, 0));
        QVERIFY(connected({QRect(places[0], QSize(1506, 847)), QRect(places[1], QSize(1080, 1920))}));
        // a screen below follows the height
        const QList<QRect> stacked{{0, 0, 1920, 1080}, {0, 1080, 1920, 1080}};
        const QList<QPoint> down = afterResize(stacked, 0, {1920, 1080}, {1920, 1200});
        QCOMPARE(down[1], QPoint(0, 1200));
        // a screen to the left of the changed one stays
        const QList<QRect> left{{0, 0, 1000, 1000}, {1000, 0, 1000, 1000}};
        QCOMPARE(afterResize(left, 1, {1000, 1000}, {1500, 1000})[0], QPoint(0, 0));
        // a bad index changes nothing
        QCOMPARE(afterResize(left, 7, {1, 1}, {2, 2})[1], QPoint(1000, 0));
    }

    void theCountdownRunsOutOnce()
    {
        RevertTimer timer(nullptr, 10);
        QSignalSpy expired(&timer, &RevertTimer::expired);
        QSignalSpy steps(&timer, &RevertTimer::remainingChanged);
        timer.start(3);
        QVERIFY(timer.running());
        QCOMPARE(timer.remaining(), 3);
        QTRY_COMPARE(expired.count(), 1);
        QCOMPARE(timer.remaining(), 0);
        QVERIFY(!timer.running());
        // 3, 2, 1, 0
        QCOMPARE(steps.count(), 4);
        QTest::qWait(60);
        QCOMPARE(expired.count(), 1);
    }

    void stoppingTheCountdownNeverExpires()
    {
        RevertTimer timer(nullptr, 10);
        QSignalSpy expired(&timer, &RevertTimer::expired);
        timer.start(5);
        QTest::qWait(25);
        timer.stop();
        QVERIFY(!timer.running());
        QCOMPARE(timer.remaining(), 0);
        QTest::qWait(100);
        QCOMPARE(expired.count(), 0);
        // and it starts again from the top
        timer.start(2);
        QTRY_COMPARE(expired.count(), 1);
    }

    void restartingTheCountdownStartsOver()
    {
        RevertTimer timer(nullptr, 20);
        QSignalSpy expired(&timer, &RevertTimer::expired);
        timer.start(3);
        QTest::qWait(30);
        timer.start(3);
        QCOMPARE(timer.remaining(), 3);
        QTest::qWait(30);
        QCOMPARE(expired.count(), 0);
        QTRY_COMPARE(expired.count(), 1);
    }
};

QTEST_GUILESS_MAIN(DisplayLogicTest)
#include "displaylogic_test.moc"
