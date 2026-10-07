#pragma once

#include <QObject>
#include <QPointer>
#include <QSortFilterProxyModel>
#include <QTimer>
#include <QVariantList>

#include <memory>

namespace PulseAudioQt
{
class Device;
class Server;
}

// The devices or the streams Sound lists, from PulseAudioQt's model (the
// sound server's own, over PipeWire's PulseAudio): outputs without the
// virtual ones, inputs without monitors, and the apps that are playing
// something, without the system's own event sounds. Every row has the model's
// roles (Name, Description, Volume, Muted, Default, PulseObject, ... as
// PulseAudioQt capitalises them) and these: `label`
// (a name that is safe to show), `kind` (see SoundLogic::deviceKind), `port`
// (the connector in use: "Headphones"), `appIcon` (an app's icon name).
class SoundFilter : public QSortFilterProxyModel
{
    Q_OBJECT
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)

public:
    enum Kind {
        Outputs,
        Inputs,
        Apps,
    };
    enum Role {
        LabelRole = Qt::UserRole + 100,
        KindRole,
        PortRole,
        AppIconRole,
    };

    SoundFilter(Kind kind, QObject *parent);

    QHash<int, QByteArray> roleNames() const override;
    // The role that has the device's name (the one the sound server knows it by).
    int nameRole() const;
    QVariant data(const QModelIndex &index, int role) const override;

Q_SIGNALS:
    void countChanged();

protected:
    bool filterAcceptsRow(int row, const QModelIndex &parent) const override;

private:
    QObject *objectAt(const QModelIndex &sourceIndex) const;
    Kind m_kind;
    QHash<int, QByteArray> m_roles;
};

// What the Sound page talks to: the lists above, the default output and input
// (their volume and mute are the PulseAudioQt objects' own properties:
// `defaultOutput.volume = x` sets it), and choosing the default. Volumes are
// the server's units; normalVolume is 100 %. Everything is live, nothing is
// applied: the server tells us when something changes, from anywhere.
class SoundMixer : public QObject
{
    Q_OBJECT
    // The sound server answered (and stays connected).
    Q_PROPERTY(bool ready READ ready NOTIFY stateChanged)
    // The last failure in plain words; "" for none.
    Q_PROPERTY(QString error READ error NOTIFY stateChanged)
    Q_PROPERTY(QObject *outputs READ outputs CONSTANT)
    Q_PROPERTY(QObject *inputs READ inputs CONSTANT)
    Q_PROPERTY(QObject *apps READ apps CONSTANT)
    // PulseAudioQt::Sink and Source of the default devices; null for none.
    Q_PROPERTY(QObject *defaultOutput READ defaultOutput NOTIFY defaultsChanged)
    Q_PROPERTY(QObject *defaultInput READ defaultInput NOTIFY defaultsChanged)
    Q_PROPERTY(qint64 normalVolume READ normalVolume CONSTANT)
    // Goes up whenever a device came, went or changed what it is called, so a
    // list built from outputChoices() can be redone.
    Q_PROPERTY(int revision READ revision NOTIFY revisionChanged)

public:
    explicit SoundMixer(QObject *parent = nullptr);

    bool ready() const;
    QString error() const;
    QObject *outputs() const;
    QObject *inputs() const;
    QObject *apps() const;
    QObject *defaultOutput() const;
    QObject *defaultInput() const;
    qint64 normalVolume() const;
    int revision() const
    {
        return m_revision;
    }

    // [{title, subtitle, value, keys}] for PickerSheet: `value` is the
    // device's name.
    Q_INVOKABLE QVariantList outputChoices() const;
    Q_INVOKABLE QVariantList inputChoices() const;
    // Makes the device named `name` the default. False for a name that isn't
    // a listed device.
    Q_INVOKABLE bool setDefaultOutput(const QString &name);
    Q_INVOKABLE bool setDefaultInput(const QString &name);
    // The kind (headphones, speaker, ...) of a PulseAudioQt device.
    Q_INVOKABLE QString kindOf(QObject *device) const;
    // A device's name, and the connector it uses, safe to show.
    Q_INVOKABLE QString labelOf(QObject *device) const;
    Q_INVOKABLE QString portOf(QObject *device) const;
    // `volume` as a percentage.
    Q_INVOKABLE int percent(qint64 volume) const;

Q_SIGNALS:
    void stateChanged();
    void defaultsChanged();
    void revisionChanged();

private:
    QVariantList choices(SoundFilter *list) const;
    bool setDefault(SoundFilter *list, const QString &name);
    void bump();
    void watchServer();

    SoundFilter *m_outputs;
    SoundFilter *m_inputs;
    SoundFilter *m_apps;
    QPointer<PulseAudioQt::Server> m_server;
    QTimer m_bump;
    int m_revision = 0;
};
