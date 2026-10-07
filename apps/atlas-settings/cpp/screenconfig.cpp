#include "screenconfig.h"

#include "displaylogic.h"
#include "safetext.h"

#include <KScreen/ConfigMonitor>
#include <KScreen/GetConfigOperation>
#include <KScreen/Mode>
#include <KScreen/Output>
#include <KScreen/Screen>
#include <KScreen/SetConfigOperation>

#ifdef ATLAS_SETTINGS_TEST_HOOKS
// Only for the test hook below: libkscreen's own Fake backend reads a screen
// layout from a JSON file instead of the compositor.
#include <kscreen/backendmanager_p.h>
#endif

#include <QDebug>
#include <QFile>
#include <QtMath>

#include <algorithm>

using namespace Qt::StringLiterals;
using namespace DisplayLogic;
using AtlasText::safeText;

namespace
{
using KScreen::ConfigPtr;
using KScreen::OutputPtr;

// libkscreen's Wayland backend sets Writable, PrimaryDisplay, PerOutputScaling
// and SynchronousOutputChanges; the Fake backend needs them said.
constexpr int FakeFeatures = int(KScreen::Config::Feature::PrimaryDisplay) | int(KScreen::Config::Feature::Writable) | int(KScreen::Config::Feature::PerOutputScaling)
    | int(KScreen::Config::Feature::SynchronousOutputChanges);

QList<Mode> modesOf(const OutputPtr &output)
{
    QList<Mode> modes;
    const auto list = output->modes();
    for (const KScreen::ModePtr &m : list) {
        modes.append({m->id(), m->size(), double(m->refreshRate())});
    }
    return modes;
}

// "16:9" for a familiar ratio, else "".
QString ratioOf(const QSize &size)
{
    const QString text = resolutionText(size);
    const qsizetype open = text.indexOf(u'(');
    return open < 0 ? QString() : text.mid(open + 1, text.size() - open - 2);
}

int degreesOf(KScreen::Output::Rotation r)
{
    switch (r) {
    case KScreen::Output::Left:
    case KScreen::Output::Flipped90:
        return 90;
    case KScreen::Output::Inverted:
    case KScreen::Output::Flipped180:
        return 180;
    case KScreen::Output::Right:
    case KScreen::Output::Flipped270:
        return 270;
    default:
        return 0;
    }
}

// Plasma's Orientation buttons: 90° Clockwise is Left, 90° Counterclockwise
// is Right.
std::optional<KScreen::Output::Rotation> rotationOf(int degrees)
{
    switch (degrees) {
    case 0:
        return KScreen::Output::None;
    case 90:
        return KScreen::Output::Left;
    case 180:
        return KScreen::Output::Inverted;
    case 270:
        return KScreen::Output::Right;
    default:
        return std::nullopt;
    }
}

bool hdrAvailable(const OutputPtr &o)
{
    using C = KScreen::Output::Capability;
    return o->capabilities().testFlag(C::HighDynamicRange) && o->capabilities().testFlag(C::WideColorGamut);
}

// The connected, enabled screens, in a fixed order (by ID).
QList<OutputPtr> enabledOutputs(const ConfigPtr &config)
{
    QList<OutputPtr> list;
    const auto outputs = config->outputs();
    for (const OutputPtr &o : outputs) {
        if (o->isConnected() && o->isEnabled()) {
            list.append(o);
        }
    }
    std::sort(list.begin(), list.end(), [](const OutputPtr &a, const OutputPtr &b) {
        return a->id() < b->id();
    });
    return list;
}

// Runs `mutate`, which changes the logical size of `o` (a resolution, a scale,
// a rotation), and moves the screens beside it so that they still touch it.
void resizing(const ConfigPtr &config, const OutputPtr &o, const std::function<void()> &mutate)
{
    const QList<OutputPtr> enabled = enabledOutputs(config);
    QList<QRect> before;
    int index = -1;
    for (int i = 0; i < enabled.size(); ++i) {
        before.append(QRect(enabled[i]->pos(), config->logicalSizeForOutputInt(*enabled[i])));
        if (enabled[i]->id() == o->id()) {
            index = i;
        }
    }
    const QSize oldLogical = config->logicalSizeForOutputInt(*o);
    mutate();
    const QSize newLogical = config->logicalSizeForOutputInt(*o);
    o->setExplicitLogicalSize(newLogical);
    if (index < 0) {
        return;
    }
    const QList<QPoint> places = afterResize(before, index, oldLogical, newLogical);
    for (int i = 0; i < enabled.size(); ++i) {
        enabled[i]->setPos(places[i]);
    }
}
}

ScreenConfig::ScreenConfig(QObject *parent)
    : QObject(parent)
{
#ifdef ATLAS_SETTINGS_TEST_HOOKS
    // ATLAS_SETTINGS_FAKE_DISPLAYS=<file>: the screens of libkscreen's Fake
    // backend's JSON file, so tests and screenshots show screens and never
    // touch the real ones. The Fake backend only lives in this process.
    const QByteArray fake = qgetenv("ATLAS_SETTINGS_FAKE_DISPLAYS");
    if (!fake.isEmpty() && QFile::exists(QString::fromLocal8Bit(fake))) {
        qputenv("KSCREEN_BACKEND", "Fake");
        qputenv("KSCREEN_BACKEND_INPROCESS", "1");
        KScreen::BackendManager::instance()->setBackendArgs({
            {u"TEST_DATA"_s, QString::fromLocal8Bit(fake)},
            {u"SUPPORTED_FEATURES"_s, FakeFeatures},
        });
        m_fake = true;
    }
#endif
    m_reload.setSingleShot(true);
    m_reload.setInterval(300);
    connect(&m_reload, &QTimer::timeout, this, &ScreenConfig::refresh);
    connect(&m_countdown, &RevertTimer::remainingChanged, this, &ScreenConfig::confirmingChanged);
    connect(&m_countdown, &RevertTimer::expired, this, &ScreenConfig::revertChanges);
    connect(KScreen::ConfigMonitor::instance(), &KScreen::ConfigMonitor::configurationChanged, this, [this] {
        // Someone else changed the screens (a plugged cable, a script): read
        // them again unless this page is in the middle of its own change.
        if (!m_busy && !m_confirming) {
            m_reload.start();
        }
    });
}

ScreenConfig::~ScreenConfig()
{
    // Settings closed while "Keep these settings?" was up: nobody said yes,
    // so the old settings come back.
    if (m_confirming && m_previous) {
        auto *op = new KScreen::SetConfigOperation(m_previous);
        op->exec();
    }
}

void ScreenConfig::setCountdown(int seconds, int intervalMs)
{
    m_countdownSeconds = seconds;
    m_countdown.setInterval(intervalMs);
}

bool ScreenConfig::perOutputScaling() const
{
    return m_config && m_config->supportedFeatures().testFlag(KScreen::Config::Feature::PerOutputScaling);
}

bool ScreenConfig::hdrCapable(const KScreen::OutputPtr &o) const
{
    // The Fake backend has no capabilities to give: its first screen has HDR.
    return hdrAvailable(o) || (m_fake && o->id() == 1);
}

bool ScreenConfig::primarySupported() const
{
    return m_config && m_config->supportedFeatures().testFlag(KScreen::Config::Feature::PrimaryDisplay);
}

void ScreenConfig::setError(const QString &text)
{
    m_error = text;
    Q_EMIT changed();
}

void ScreenConfig::refresh()
{
    if (m_reading) {
        return;
    }
    m_reading = true;
    auto *op = new KScreen::GetConfigOperation(KScreen::GetConfigOperation::NoOptions, this);
    // A compositor that never answers must not leave the page waiting.
    auto *guard = new QTimer(op);
    guard->setSingleShot(true);
    guard->setInterval(10000);
    connect(guard, &QTimer::timeout, this, [this, op] {
        m_reading = false;
        m_loaded = true;
        setError(tr("Settings couldn't reach the display service."));
        op->deleteLater();
    });
    guard->start();
    connect(op, &KScreen::ConfigOperation::finished, this, [this, guard](KScreen::ConfigOperation *finished) {
        guard->stop();
        m_reading = false;
        m_loaded = true;
        if (finished->hasError()) {
            qWarning() << "displays:" << finished->errorString();
            m_config.reset();
            m_outputs.clear();
            setError(tr("Settings couldn't read the displays."));
            return;
        }
        m_config = qobject_cast<KScreen::GetConfigOperation *>(finished)->config();
        if (m_config) {
            KScreen::ConfigMonitor::instance()->addConfig(m_config);
        }
        m_error.clear();
        publish();
    });
}

QList<QRect> ScreenConfig::placesOf(const KScreen::ConfigPtr &config, int except) const
{
    QList<QRect> rects;
    for (const OutputPtr &o : enabledOutputs(config)) {
        if (o->id() != except) {
            rects.append(QRect(o->pos(), config->logicalSizeForOutputInt(*o)));
        }
    }
    return rects;
}

void ScreenConfig::publish()
{
    m_outputs.clear();
    if (!m_config) {
        Q_EMIT changed();
        return;
    }
    QList<OutputPtr> list;
    const auto outputs = m_config->outputs();
    for (const OutputPtr &o : outputs) {
        if (o->isConnected()) {
            list.append(o);
        }
    }
    std::sort(list.begin(), list.end(), [](const OutputPtr &a, const OutputPtr &b) {
        return a->pos().x() != b->pos().x() ? a->pos().x() < b->pos().x() : (a->pos().y() != b->pos().y() ? a->pos().y() < b->pos().y() : a->id() < b->id());
    });

    // A name for each, with the connector's name added where two are alike.
    QStringList labels;
    for (const OutputPtr &o : std::as_const(list)) {
        QString label;
        if (o->type() == KScreen::Output::Panel) {
            label = tr("Built-in Display");
        } else {
            label = safeText(o->vendor() + u' ' + o->model(), 60);
            if (label.isEmpty()) {
                label = safeText(o->name(), 40);
            }
        }
        if (label.isEmpty()) {
            label = tr("Display");
        }
        labels.append(label);
    }
    for (int i = 0; i < list.size(); ++i) {
        if (labels.count(labels[i]) > 1) {
            labels[i] += u" ("_s + safeText(list[i]->name(), 20) + u')';
        }
    }

    for (int i = 0; i < list.size(); ++i) {
        const OutputPtr &o = list[i];
        const QSize logical = m_config->logicalSizeForOutputInt(*o);
        const QList<Mode> modes = modesOf(o);
        const KScreen::ModePtr current = o->currentMode();
        const QSize currentSize = current ? current->size() : QSize();
        const double currentRate = current ? double(current->refreshRate()) : 0;
        const QSize preferred = o->preferredMode() ? o->preferredMode()->size() : QSize();

        QVariantList resolutionList;
        for (const QSize &s : resolutions(modes)) {
            resolutionList.append(QVariantMap{
                {u"width"_s, s.width()},
                {u"height"_s, s.height()},
                {u"text"_s, resolutionText(s, false)},
                {u"ratio"_s, ratioOf(s)},
                {u"recommended"_s, s == preferred},
                {u"current"_s, s == currentSize},
            });
        }
        QVariantList rateList;
        for (const double r : refreshRates(modes, currentSize)) {
            rateList.append(QVariantMap{
                {u"hertz"_s, r},
                {u"text"_s, rateText(r)},
                {u"current"_s, std::abs(r - currentRate) < 0.001},
            });
        }

        QVariantMap map{
            {u"id"_s, o->id()},
            {u"name"_s, safeText(o->name(), 40)},
            {u"label"_s, labels[i]},
            {u"internal"_s, o->type() == KScreen::Output::Panel},
            {u"enabled"_s, o->isEnabled()},
            {u"primary"_s, o->priority() == 1},
            {u"x"_s, o->pos().x()},
            {u"y"_s, o->pos().y()},
            {u"width"_s, logical.width()},
            {u"height"_s, logical.height()},
            {u"pixelWidth"_s, currentSize.width()},
            {u"pixelHeight"_s, currentSize.height()},
            {u"scale"_s, o->scale()},
            {u"scalePercent"_s, qRound(o->scale() * 100.0)},
            {u"rotation"_s, degreesOf(o->rotation())},
            {u"resolutions"_s, resolutionList},
            {u"resolutionText"_s, currentSize.isValid() ? resolutionText(currentSize, false) : QString()},
            {u"rates"_s, rateList},
            {u"rate"_s, currentRate},
            {u"rateText"_s, currentRate > 0 ? rateText(currentRate) : QString()},
            {u"hdr"_s, o->isHdrEnabled()},
            // The Fake backend has no capabilities to give.
            {u"hdrSupported"_s, hdrCapable(o)},
        };
        m_outputs.append(map);
    }
    Q_EMIT changed();
}

void ScreenConfig::change(int id, bool risky, const Edit &edit)
{
    if (!m_config || m_busy) {
        return;
    }
    ConfigPtr next = m_config->clone();
    const OutputPtr output = next->output(id);
    if (!output || !output->isConnected() || !edit(output, next)) {
        return;
    }
    apply(next, risky);
}

void ScreenConfig::apply(const KScreen::ConfigPtr &next, bool risky)
{
    // Screens keep their places relative to each other; the top left is 0, 0.
    const QList<OutputPtr> enabled = enabledOutputs(next);
    QList<QRect> rects;
    for (const OutputPtr &o : enabled) {
        rects.append(QRect(o->pos(), next->logicalSizeForOutputInt(*o)));
    }
    if (enabled.isEmpty() || overlapping(rects) || !connected(rects)) {
        setError(tr("The screens have to touch each other, without overlapping."));
        publish();
        return;
    }
    const QPoint origin = originOffset(rects);
    if (!origin.isNull()) {
        for (const OutputPtr &o : std::as_const(enabled)) {
            o->setPos(o->pos() - origin);
        }
    }
    if (!KScreen::Config::canBeApplied(next)) {
        setError(tr("These displays can't be set up that way."));
        publish();
        return;
    }

    m_error.clear();
    m_notice.clear();
    m_busy = true;
    Q_EMIT changed();
    // As it is now: libkscreen's monitor updates m_config in place whenever
    // the compositor changes, so by the time this finishes it is the new state.
    const ConfigPtr before = m_config->clone();
    auto *op = new KScreen::SetConfigOperation(next, this);
    connect(op, &KScreen::ConfigOperation::finished, this, [this, next, before, risky](KScreen::ConfigOperation *finished) {
        m_busy = false;
        if (finished->hasError()) {
            qWarning() << "displays:" << finished->errorString();
            setError(tr("The displays couldn't be changed."));
            refresh();
            return;
        }
        if (risky) {
            // The first change of a series is the one to go back to.
            if (!m_confirming) {
                m_previous = before;
            }
            m_config = next;
            publish();
            startConfirm();
        } else {
            m_config = next;
            publish();
        }
        // What the compositor made of it.
        refresh();
    });
}

void ScreenConfig::startConfirm()
{
    m_confirming = true;
    m_countdown.start(m_countdownSeconds);
    Q_EMIT confirmingChanged();
}

void ScreenConfig::finishConfirm()
{
    m_countdown.stop();
    m_confirming = false;
    m_previous.reset();
    Q_EMIT confirmingChanged();
}

void ScreenConfig::keepChanges()
{
    if (m_confirming) {
        finishConfirm();
    }
}

void ScreenConfig::revertChanges()
{
    if (!m_confirming) {
        return;
    }
    const ConfigPtr previous = m_previous;
    finishConfirm();
    if (!previous) {
        refresh();
        return;
    }
    m_busy = true;
    Q_EMIT changed();
    auto *op = new KScreen::SetConfigOperation(previous, this);
    connect(op, &KScreen::ConfigOperation::finished, this, [this, previous](KScreen::ConfigOperation *finished) {
        m_busy = false;
        if (finished->hasError()) {
            qWarning() << "displays:" << finished->errorString();
            setError(tr("The earlier settings couldn't be restored."));
        } else {
            m_config = previous->clone();
            m_notice = tr("Went back to your earlier display settings.");
            publish();
        }
        refresh();
    });
}

void ScreenConfig::setResolution(int id, int width, int height)
{
    change(id, true, [&](const OutputPtr &o, const ConfigPtr &config) {
        const KScreen::ModePtr current = o->currentMode();
        const QString mode = modeFor(modesOf(o), QSize(width, height), current ? double(current->refreshRate()) : 0);
        if (mode.isEmpty() || mode == o->currentModeId()) {
            return false;
        }
        resizing(config, o, [&] {
            o->setCurrentModeId(mode);
            o->setSize(o->currentMode()->size());
        });
        return true;
    });
}

void ScreenConfig::setScale(int id, double scale)
{
    change(id, true, [&](const OutputPtr &o, const ConfigPtr &config) {
        const double wanted = scaleFromPercent(clampScalePercent(scale));
        if (qFuzzyCompare(o->scale(), wanted)) {
            return false;
        }
        resizing(config, o, [&] {
            o->setScale(wanted);
        });
        return true;
    });
}

void ScreenConfig::setRefreshRate(int id, double hertz)
{
    change(id, true, [&](const OutputPtr &o, const ConfigPtr &) {
        const KScreen::ModePtr current = o->currentMode();
        if (!current) {
            return false;
        }
        const QString mode = modeAtRate(modesOf(o), current->size(), hertz);
        if (mode.isEmpty() || mode == o->currentModeId()) {
            return false;
        }
        o->setCurrentModeId(mode);
        return true;
    });
}

void ScreenConfig::setRotation(int id, int degrees)
{
    const auto rotation = rotationOf(degrees);
    if (!rotation) {
        return;
    }
    change(id, true, [&](const OutputPtr &o, const ConfigPtr &config) {
        if (o->rotation() == *rotation) {
            return false;
        }
        resizing(config, o, [&] {
            o->setRotation(*rotation);
        });
        return true;
    });
}

void ScreenConfig::setHdr(int id, bool on)
{
    change(id, true, [&](const OutputPtr &o, const ConfigPtr &) {
        if (o->isHdrEnabled() == on || (on && !hdrCapable(o))) {
            return false;
        }
        // As Plasma's page does: wide colour gamut goes with HDR.
        o->setHdrEnabled(on);
        o->setWcgEnabled(on);
        return true;
    });
}

void ScreenConfig::setPrimary(int id)
{
    change(id, false, [&](const OutputPtr &o, const ConfigPtr &config) {
        if (o->priority() == 1) {
            return false;
        }
        config->setOutputPriority(o, 1);
        return true;
    });
}

QPoint ScreenConfig::snapped(int id, int x, int y) const
{
    if (!m_config) {
        return {x, y};
    }
    const OutputPtr o = m_config->output(id);
    if (!o) {
        return {x, y};
    }
    return snapPosition(placesOf(m_config, id), m_config->logicalSizeForOutputInt(*o), {x, y});
}

void ScreenConfig::moveOutput(int id, int x, int y)
{
    change(id, true, [&](const OutputPtr &o, const ConfigPtr &config) {
        const QPoint to = snapPosition(placesOf(config, id), config->logicalSizeForOutputInt(*o), {x, y});
        if (to == o->pos()) {
            return false;
        }
        o->setPos(to);
        return true;
    });
}
