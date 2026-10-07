#pragma once

#include <QList>
#include <QPoint>
#include <QRect>
#include <QSize>
#include <QString>

// The rules of the Displays page that need no screen: which resolutions and
// refresh rates a list of modes offers, which mode a change of resolution
// lands on, the scale steps, how the screens snap together and when an
// arrangement is acceptable. Plain Qt value types, so they are tested
// without libkscreen (tests/displaylogic_test.cpp); screenconfig.cpp feeds
// them from KScreen::Config.
namespace DisplayLogic
{
struct Mode {
    QString id;
    QSize size; // pixels
    double rate = 0; // Hz
};

// The scale Plasma 6.7's Displays page allows: 50 % to 300 % in 5 % steps
// (its slider has marks every 25 %). Every step is a whole number of 1/120
// (the Wayland fractional-scale unit), so KWin keeps it as set.
constexpr int MinScalePercent = 50;
constexpr int MaxScalePercent = 300;
constexpr int ScaleStepPercent = 5;
constexpr int ScaleMarkPercent = 25;

// `scale` as the nearest step within the range (NaN gives 100 %).
int clampScalePercent(double scale);
double scaleFromPercent(int percent);
// "170%".
QString scaleText(double scale);

// The distinct resolutions of `modes`, widest first (then tallest).
QList<QSize> resolutions(const QList<Mode> &modes);
// The distinct refresh rates (within 0.001 Hz) the modes of `size` offer,
// fastest first.
QList<double> refreshRates(const QList<Mode> &modes, const QSize &size);
// The mode to set for resolution `size`: the one with the rate the screen
// has now when there is one, else the fastest of that size. "" when no mode
// has that size.
QString modeFor(const QList<Mode> &modes, const QSize &size, double currentRate);
// The mode of `size` at the rate nearest to `rate`; "" when none has the size.
QString modeAtRate(const QList<Mode> &modes, const QSize &size, double rate);

// "3840 × 2160" and "3840 × 2160 (16:9)": the ratio is only named when it is
// a familiar one.
QString resolutionText(const QSize &size, bool withRatio = true);
// "60 Hz", "59.94 Hz", "143.98 Hz": at most two decimals, none when whole.
QString rateText(double rate);

// The arrangement. A screen's rectangle is its place and size in logical
// pixels, as the compositor lays them out.

// Where a screen of `size` dragged to `dest` lands: touching one of `others`
// along an edge, never overlapping any, and lined up with that edge's
// start, middle or end when within `area` pixels. `dest` itself with
// nothing to snap to.
QPoint snapPosition(const QList<QRect> &others, const QSize &size, const QPoint &dest, int area = 80);
// Whether every screen reaches every other through screens that share part of
// an edge (a gap or a lone corner is not acceptable).
bool connected(const QList<QRect> &screens);
// True when two screens overlap.
bool overlapping(const QList<QRect> &screens);
// The offset to subtract from every position so the top left is 0, 0.
QPoint originOffset(const QList<QRect> &screens);
// The screens' places after screen `index` changed from `oldSize` to
// `newSize`: the ones right of its centre move by the change in width, the
// ones below it by the change in height, so what touched it still does.
QList<QPoint> afterResize(const QList<QRect> &screens, int index, const QSize &oldSize, const QSize &newSize);
}
