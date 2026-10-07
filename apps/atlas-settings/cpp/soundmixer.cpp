#include "soundmixer.h"

#include "safetext.h"
#include "soundlogic.h"

#include <PulseAudioQt/Client>
#include <PulseAudioQt/Context>
#include <PulseAudioQt/Device>
#include <PulseAudioQt/Models>
#include <PulseAudioQt/Port>
#include <PulseAudioQt/Server>
#include <PulseAudioQt/Sink>
#include <PulseAudioQt/Source>
#include <PulseAudioQt/Stream>

using namespace Qt::StringLiterals;
using PulseAudioQt::AbstractModel;
using PulseAudioQt::Context;
using PulseAudioQt::Device;

namespace
{
QString deviceLabel(const Device *device)
{
    const QString text = AtlasText::safeText(device->description(), 80);
    return text.isEmpty() ? AtlasText::safeText(device->name(), 80) : text;
}

// The connector in use ("Headphones"), "" when it names none.
QString devicePort(const Device *device)
{
    const auto ports = device->ports();
    const quint32 active = device->activePortIndex();
    return active < quint32(ports.size()) ? AtlasText::safeText(ports.at(int(active))->description(), 60) : QString();
}
}

SoundFilter::SoundFilter(Kind kind, QObject *parent)
    : QSortFilterProxyModel(parent)
    , m_kind(kind)
{
    QAbstractItemModel *source = nullptr;
    switch (kind) {
    case Outputs:
        source = new PulseAudioQt::SinkModel(this);
        break;
    case Inputs:
        source = new PulseAudioQt::SourceModel(this);
        break;
    case Apps:
        source = new PulseAudioQt::SinkInputModel(this);
        break;
    }
    setSourceModel(source);
    setDynamicSortFilter(true);

    m_roles = source->roleNames();
    m_roles.insert(LabelRole, "label");
    m_roles.insert(KindRole, "kind");
    m_roles.insert(PortRole, "port");
    m_roles.insert(AppIconRole, "appIcon");

    connect(this, &QAbstractItemModel::rowsInserted, this, &SoundFilter::countChanged);
    connect(this, &QAbstractItemModel::rowsRemoved, this, &SoundFilter::countChanged);
    connect(this, &QAbstractItemModel::modelReset, this, &SoundFilter::countChanged);

    // A stream that pauses, a device that becomes the default: the model says
    // only which role changed, which the proxy doesn't re-filter for, and the
    // roles added here follow the model's, so both are redone, in one go.
    auto *refilter = new QTimer(this);
    refilter->setSingleShot(true);
    refilter->setInterval(30);
    connect(refilter, &QTimer::timeout, this, [this] {
        invalidateFilter();
    });
    connect(source, &QAbstractItemModel::dataChanged, this, [this, refilter](const QModelIndex &first, const QModelIndex &last) {
        refilter->start();
        const QModelIndex a = mapFromSource(first);
        const QModelIndex b = mapFromSource(last);
        if (a.isValid() && b.isValid()) {
            Q_EMIT dataChanged(a, b);
        }
    });
}

int SoundFilter::nameRole() const
{
    // PulseAudioQt names the roles after the properties, capitalised.
    for (auto it = m_roles.cbegin(); it != m_roles.cend(); ++it) {
        if (it.value().toLower() == "name") {
            return it.key();
        }
    }
    return -1;
}

QHash<int, QByteArray> SoundFilter::roleNames() const
{
    return m_roles;
}

QObject *SoundFilter::objectAt(const QModelIndex &sourceIndex) const
{
    return sourceModel()->data(sourceIndex, AbstractModel::PulseObjectRole).value<QObject *>();
}

bool SoundFilter::filterAcceptsRow(int row, const QModelIndex &parent) const
{
    QObject *object = objectAt(sourceModel()->index(row, 0, parent));
    if (m_kind == Apps) {
        const auto *stream = qobject_cast<PulseAudioQt::Stream *>(object);
        return stream && !stream->isVirtualStream() && !stream->isCorked() && stream->hasVolume() && !SoundLogic::isEventSound(stream->properties());
    }
    const auto *device = qobject_cast<Device *>(object);
    if (!device) {
        return false;
    }
    if (m_kind == Inputs && SoundLogic::isMonitor(device->name(), device->properties())) {
        return false;
    }
    // The ones the system makes up (a filter, a loopback) only when they are
    // what is in use.
    return !device->isVirtualDevice() || device->isDefault();
}

QVariant SoundFilter::data(const QModelIndex &index, int role) const
{
    if (role < LabelRole || role > AppIconRole) {
        return QSortFilterProxyModel::data(index, role);
    }
    QObject *object = objectAt(mapToSource(index));
    if (!object) {
        return {};
    }
    if (const auto *stream = qobject_cast<PulseAudioQt::Stream *>(object)) {
        switch (role) {
        case LabelRole:
            return SoundLogic::appName(stream->client() ? stream->client()->name() : QString(), stream->properties());
        case AppIconRole:
            return SoundLogic::appIcon(stream->properties());
        default:
            return QString();
        }
    }
    const auto *device = qobject_cast<Device *>(object);
    if (!device) {
        return {};
    }
    switch (role) {
    case LabelRole:
        return deviceLabel(device);
    case KindRole:
        return SoundLogic::deviceKind(device->formFactor(), device->properties().value(u"device.bus"_s).toString(), device->name(), m_kind == Inputs);
    case PortRole:
        return devicePort(device);
    default:
        return QString();
    }
}

SoundMixer::SoundMixer(QObject *parent)
    : QObject(parent)
{
    Context::setApplicationId(u"net.eterneon.atlas.settings"_s);
    Context *context = Context::instance();
    m_outputs = new SoundFilter(SoundFilter::Outputs, this);
    m_inputs = new SoundFilter(SoundFilter::Inputs, this);
    m_apps = new SoundFilter(SoundFilter::Apps, this);

    m_bump.setSingleShot(true);
    m_bump.setInterval(30);
    connect(&m_bump, &QTimer::timeout, this, [this] {
        ++m_revision;
        Q_EMIT revisionChanged();
    });
    for (SoundFilter *list : {m_outputs, m_inputs}) {
        connect(list, &QAbstractItemModel::rowsInserted, this, &SoundMixer::bump);
        connect(list, &QAbstractItemModel::rowsRemoved, this, &SoundMixer::bump);
        connect(list, &QAbstractItemModel::modelReset, this, &SoundMixer::bump);
        connect(list, &QAbstractItemModel::dataChanged, this, &SoundMixer::bump);
    }
    connect(context, &Context::stateChanged, this, [this] {
        watchServer();
        Q_EMIT stateChanged();
        Q_EMIT defaultsChanged();
    });
    watchServer();
}

void SoundMixer::watchServer()
{
    PulseAudioQt::Server *server = Context::instance()->server();
    if (server == m_server) {
        return;
    }
    m_server = server;
    if (!server) {
        return;
    }
    connect(server, &PulseAudioQt::Server::defaultSinkChanged, this, [this] {
        Q_EMIT defaultsChanged();
        bump();
    });
    connect(server, &PulseAudioQt::Server::defaultSourceChanged, this, [this] {
        Q_EMIT defaultsChanged();
        bump();
    });
}

void SoundMixer::bump()
{
    m_bump.start();
}

bool SoundMixer::ready() const
{
    return Context::instance()->state() == Context::State::Ready;
}

QString SoundMixer::error() const
{
    switch (Context::instance()->state()) {
    case Context::State::Failed:
    case Context::State::Terminated:
        return tr("Settings couldn't reach the sound system.");
    default:
        return QString();
    }
}

QObject *SoundMixer::outputs() const
{
    return m_outputs;
}

QObject *SoundMixer::inputs() const
{
    return m_inputs;
}

QObject *SoundMixer::apps() const
{
    return m_apps;
}

QObject *SoundMixer::defaultOutput() const
{
    return m_server ? static_cast<QObject *>(m_server->defaultSink()) : nullptr;
}

QObject *SoundMixer::defaultInput() const
{
    return m_server ? static_cast<QObject *>(m_server->defaultSource()) : nullptr;
}

qint64 SoundMixer::normalVolume() const
{
    return PulseAudioQt::normalVolume();
}

QVariantList SoundMixer::choices(SoundFilter *list) const
{
    const int nameRole = list->nameRole();
    QVariantList out;
    for (int row = 0; row < list->rowCount() && nameRole >= 0; ++row) {
        const QModelIndex index = list->index(row, 0);
        const QString label = list->data(index, SoundFilter::LabelRole).toString();
        const QString port = list->data(index, SoundFilter::PortRole).toString();
        out.append(QVariantMap{
            {u"title"_s, label},
            {u"subtitle"_s, port},
            {u"value"_s, list->data(index, nameRole).toString()},
            {u"keys"_s, (label + u' ' + port).toLower()},
        });
    }
    return out;
}

QVariantList SoundMixer::outputChoices() const
{
    return choices(m_outputs);
}

QVariantList SoundMixer::inputChoices() const
{
    return choices(m_inputs);
}

bool SoundMixer::setDefault(SoundFilter *list, const QString &name)
{
    // Only a device the page lists.
    const int role = list->nameRole();
    for (int row = 0; row < list->rowCount() && role >= 0; ++row) {
        const QModelIndex index = list->index(row, 0);
        if (list->data(index, role).toString() != name) {
            continue;
        }
        auto *device = qobject_cast<Device *>(list->data(index, AbstractModel::PulseObjectRole).value<QObject *>());
        if (!device) {
            return false;
        }
        device->setDefault(true);
        return true;
    }
    return false;
}

bool SoundMixer::setDefaultOutput(const QString &name)
{
    return setDefault(m_outputs, name);
}

bool SoundMixer::setDefaultInput(const QString &name)
{
    return setDefault(m_inputs, name);
}

QString SoundMixer::kindOf(QObject *object) const
{
    const auto *device = qobject_cast<Device *>(object);
    if (!device) {
        return u"other"_s;
    }
    const bool input = qobject_cast<PulseAudioQt::Source *>(object) != nullptr;
    return SoundLogic::deviceKind(device->formFactor(), device->properties().value(u"device.bus"_s).toString(), device->name(), input);
}

QString SoundMixer::labelOf(QObject *object) const
{
    const auto *device = qobject_cast<Device *>(object);
    return device ? deviceLabel(device) : QString();
}

QString SoundMixer::portOf(QObject *object) const
{
    const auto *device = qobject_cast<Device *>(object);
    return device ? devicePort(device) : QString();
}

int SoundMixer::percent(qint64 volume) const
{
    return SoundLogic::volumePercent(volume, PulseAudioQt::normalVolume());
}
