#include "displaylogic.h"

#include <QChar>
#include <QtMath>

#include <algorithm>
#include <cmath>
#include <limits>

namespace DisplayLogic
{
namespace
{
bool sameRate(double a, double b)
{
    return std::abs(a - b) < 0.001;
}

// Whether two rectangles share part of an edge (touching, not overlapping).
bool touching(const QRect &a, const QRect &b)
{
    const bool side = (a.x() + a.width() == b.x() || b.x() + b.width() == a.x()) && std::min(a.y() + a.height(), b.y() + b.height()) > std::max(a.y(), b.y());
    const bool stacked = (a.y() + a.height() == b.y() || b.y() + b.height() == a.y()) && std::min(a.x() + a.width(), b.x() + b.width()) > std::max(a.x(), b.x());
    return side || stacked;
}

// `value` moved onto the nearest of `targets` when that is within `area`.
int aligned(int value, std::initializer_list<int> targets, int area)
{
    int best = value;
    int bestDistance = area + 1;
    for (const int t : targets) {
        const int d = std::abs(t - value);
        if (d <= area && d < bestDistance) {
            best = t;
            bestDistance = d;
        }
    }
    return best;
}
}

int clampScalePercent(double scale)
{
    if (std::isnan(scale)) {
        return 100;
    }
    const double steps = std::round(scale * 100.0 / ScaleStepPercent);
    const int percent = int(std::clamp(steps, -1000.0, 1000.0)) * ScaleStepPercent;
    return std::clamp(percent, MinScalePercent, MaxScalePercent);
}

double scaleFromPercent(int percent)
{
    // 1/120 steps, as KWin keeps it: 170 % is 204/120, exactly 1.7.
    return std::round(percent * 120.0 / 100.0) / 120.0;
}

QString scaleText(double scale)
{
    return QStringLiteral("%1%").arg(qRound(scale * 100.0));
}

QList<QSize> resolutions(const QList<Mode> &modes)
{
    QList<QSize> sizes;
    for (const Mode &m : modes) {
        if (!sizes.contains(m.size)) {
            sizes.append(m.size);
        }
    }
    std::sort(sizes.begin(), sizes.end(), [](const QSize &a, const QSize &b) {
        return a.width() != b.width() ? a.width() > b.width() : a.height() > b.height();
    });
    return sizes;
}

QList<double> refreshRates(const QList<Mode> &modes, const QSize &size)
{
    QList<double> rates;
    for (const Mode &m : modes) {
        if (m.size != size) {
            continue;
        }
        const bool seen = std::any_of(rates.cbegin(), rates.cend(), [&](double r) {
            return sameRate(r, m.rate);
        });
        if (!seen) {
            rates.append(m.rate);
        }
    }
    std::stable_sort(rates.begin(), rates.end(), std::greater<>());
    return rates;
}

QString modeFor(const QList<Mode> &modes, const QSize &size, double currentRate)
{
    const Mode *best = nullptr;
    for (const Mode &m : modes) {
        if (m.size != size) {
            continue;
        }
        if (sameRate(m.rate, currentRate)) {
            return m.id;
        }
        if (!best || m.rate > best->rate) {
            best = &m;
        }
    }
    return best ? best->id : QString();
}

QString modeAtRate(const QList<Mode> &modes, const QSize &size, double rate)
{
    const Mode *best = nullptr;
    for (const Mode &m : modes) {
        if (m.size != size) {
            continue;
        }
        if (!best || std::abs(m.rate - rate) < std::abs(best->rate - rate)) {
            best = &m;
        }
    }
    return best ? best->id : QString();
}

QString resolutionText(const QSize &size, bool withRatio)
{
    // No thousands separators: "3840 × 2160".
    QString text = QStringLiteral("%1 × %2").arg(size.width()).arg(size.height());
    if (!withRatio || size.width() <= 0 || size.height() <= 0) {
        return text;
    }
    struct Ratio {
        int w, h;
    };
    static constexpr Ratio known[] = {{16, 9}, {16, 10}, {4, 3}, {5, 4}, {3, 2}, {21, 9}, {32, 9}, {32, 10}, {1, 1}};
    const double r = double(size.width()) / size.height();
    for (const Ratio &k : known) {
        const double kr = double(k.w) / k.h;
        // 21:9 screens are 2.33 to 2.40 wide.
        const double tolerance = k.w == 21 ? 0.04 * kr : 0.015 * kr;
        if (std::abs(r - kr) <= tolerance) {
            return text + QStringLiteral(" (%1:%2)").arg(k.w).arg(k.h);
        }
    }
    return text;
}

QString rateText(double rate)
{
    if (std::abs(rate - std::round(rate)) < 0.005) {
        return QStringLiteral("%1 Hz").arg(qRound(rate));
    }
    QString digits = QString::number(rate, 'f', 2);
    while (digits.endsWith(QLatin1Char('0'))) {
        digits.chop(1);
    }
    return QStringLiteral("%1 Hz").arg(digits);
}

QPoint snapPosition(const QList<QRect> &others, const QSize &size, const QPoint &dest, int area)
{
    if (others.isEmpty() || size.isEmpty()) {
        return dest;
    }
    QPoint best = dest;
    qint64 bestCost = std::numeric_limits<qint64>::max();
    auto consider = [&](const QPoint &p) {
        const QRect moved(p, size);
        for (const QRect &o : others) {
            if (moved.intersects(o)) {
                return;
            }
        }
        const qint64 cost = qint64(std::abs(p.x() - dest.x())) + std::abs(p.y() - dest.y());
        if (cost < bestCost) {
            bestCost = cost;
            best = p;
        }
    };
    for (const QRect &o : others) {
        // Beside it: its left or right edge, the height kept within its span.
        for (const int x : {o.x() - size.width(), o.x() + o.width()}) {
            int y = std::clamp(dest.y(), o.y() - size.height() + 1, o.y() + o.height() - 1);
            y = aligned(y, {o.y(), o.y() + o.height() - size.height(), o.y() + (o.height() - size.height()) / 2}, area);
            consider({x, y});
        }
        // Above or below it.
        for (const int y : {o.y() - size.height(), o.y() + o.height()}) {
            int x = std::clamp(dest.x(), o.x() - size.width() + 1, o.x() + o.width() - 1);
            x = aligned(x, {o.x(), o.x() + o.width() - size.width(), o.x() + (o.width() - size.width()) / 2}, area);
            consider({x, y});
        }
    }
    return bestCost == std::numeric_limits<qint64>::max() ? dest : best;
}

bool connected(const QList<QRect> &screens)
{
    if (screens.size() < 2) {
        return true;
    }
    QList<bool> seen(screens.size(), false);
    QList<int> todo{0};
    seen[0] = true;
    int count = 1;
    while (!todo.isEmpty()) {
        const int i = todo.takeLast();
        for (int j = 0; j < screens.size(); ++j) {
            if (!seen[j] && touching(screens[i], screens[j])) {
                seen[j] = true;
                ++count;
                todo.append(j);
            }
        }
    }
    return count == screens.size();
}

bool overlapping(const QList<QRect> &screens)
{
    for (int i = 0; i < screens.size(); ++i) {
        for (int j = i + 1; j < screens.size(); ++j) {
            if (screens[i].intersects(screens[j])) {
                return true;
            }
        }
    }
    return false;
}

QPoint originOffset(const QList<QRect> &screens)
{
    if (screens.isEmpty()) {
        return {};
    }
    int x = screens.first().x();
    int y = screens.first().y();
    for (const QRect &r : screens) {
        x = std::min(x, r.x());
        y = std::min(y, r.y());
    }
    return {x, y};
}

QList<QPoint> afterResize(const QList<QRect> &screens, int index, const QSize &oldSize, const QSize &newSize)
{
    QList<QPoint> places;
    places.reserve(screens.size());
    if (index < 0 || index >= screens.size()) {
        for (const QRect &r : screens) {
            places.append(r.topLeft());
        }
        return places;
    }
    const QPoint centre = QRect(screens[index].topLeft(), oldSize).center();
    const QPoint delta(newSize.width() - oldSize.width(), newSize.height() - oldSize.height());
    for (int i = 0; i < screens.size(); ++i) {
        QPoint p = screens[i].topLeft();
        if (i != index) {
            if (p.x() >= centre.x()) {
                p.rx() += delta.x();
            }
            if (p.y() >= centre.y()) {
                p.ry() += delta.y();
            }
        }
        places.append(p);
    }
    return places;
}
}
