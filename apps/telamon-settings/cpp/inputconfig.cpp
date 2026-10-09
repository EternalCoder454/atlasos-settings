#include "inputconfig.h"

#include "kdeutil.h"

#include <QDBusArgument>
#include <QDBusVariant>
#include <QFile>
#include <QRegularExpression>
#include <QXmlStreamReader>

#include <memory>

using namespace Qt::StringLiterals;

namespace
{
constexpr auto DeviceInterface = "org.kde.KWin.InputDevice";
constexpr auto ManagerInterface = "org.kde.KWin.InputDeviceManager";
constexpr int MaxDevices = 64;
constexpr int MaxLayouts = 4;

QString layoutOf(const QString &id)
{
    const qsizetype open = id.indexOf(u'(');
    return open < 0 ? id : id.left(open);
}

QString variantOf(const QString &id)
{
    const qsizetype open = id.indexOf(u'(');
    return open < 0 ? QString() : id.mid(open + 1, id.size() - open - 2);
}

QString idOf(const QString &layout, const QString &variant)
{
    return variant.isEmpty() ? layout : layout + u'(' + variant + u')';
}

// A reply's first value, which QtDBus hands over as a QDBusVariant or as a
// map or list still wrapped in its QDBusArgument.
template<typename T>
T unwrap(const QVariant &value)
{
    QVariant v = value;
    if (v.canConvert<QDBusVariant>()) {
        v = v.value<QDBusVariant>().variant();
    }
    if (v.canConvert<QDBusArgument>()) {
        return qdbus_cast<T>(v.value<QDBusArgument>());
    }
    return v.value<T>();
}
}

InputConfig::InputConfig(QObject *parent)
    : QObject(parent)
{
}

bool InputConfig::validLayoutId(const QString &id)
{
    static const QRegularExpression re(u"\\A[A-Za-z0-9_-]{1,32}(\\([A-Za-z0-9_-]{1,32}\\))?\\z"_s);
    return re.match(id).hasMatch();
}

QVariantList InputConfig::availableLayouts()
{
    if (m_availableRead) {
        return m_available;
    }
    m_availableRead = true;
    // xkeyboard-config's list of layouts and their variants: what
    // kcm_keyboard offers.
    QFile file(m_rulesPath.isEmpty() ? u"/usr/share/X11/xkb/rules/evdev.xml"_s : m_rulesPath);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 16 * 1024 * 1024) {
        return m_available;
    }
    QXmlStreamReader xml(&file);
    QStringList path;
    QString layout;
    QString layoutDescription;
    QString variant;
    QString name;
    QString description;
    auto bounded = [](const QString &s) {
        return s.simplified().left(100);
    };
    int variants = 0;
    while (!xml.atEnd() && m_available.size() < 6000) {
        switch (xml.readNext()) {
        case QXmlStreamReader::StartElement:
            path << xml.name().toString();
            if (xml.name() == u"configItem"_s) {
                name.clear();
                description.clear();
            }
            break;
        case QXmlStreamReader::Characters:
            if (path.size() >= 2 && path.at(path.size() - 2) == u"configItem"_s) {
                if (path.last() == u"name"_s) {
                    name += xml.text();
                } else if (path.last() == u"description"_s) {
                    description += xml.text();
                }
            }
            break;
        case QXmlStreamReader::EndElement:
            if (xml.name() == u"configItem"_s) {
                // A configItem of a layout (xkbConfigRegistry/layoutList/
                // layout/configItem) or of a variant (.../variant/configItem).
                const bool inVariant = path.size() >= 2 && path.at(path.size() - 2) == u"variant"_s;
                const bool inLayout = path.size() >= 2 && path.at(path.size() - 2) == u"layout"_s && path.contains(u"layoutList"_s);
                if (inLayout && validLayoutId(name.trimmed())) {
                    layout = name.trimmed();
                    layoutDescription = bounded(description);
                    variants = 0;
                    m_available.append(QVariantMap{{u"id"_s, layout},
                                                   {u"layout"_s, layout},
                                                   {u"variant"_s, QString()},
                                                   {u"name"_s, layoutDescription.isEmpty() ? layout : layoutDescription},
                                                   {u"keys"_s, (layout + u' ' + layoutDescription).toLower()}});
                } else if (inVariant && !layout.isEmpty() && validLayoutId(name.trimmed()) && !name.trimmed().contains(u'(') && variants++ < 200) {
                    variant = name.trimmed();
                    const QString text = bounded(description);
                    m_available.append(QVariantMap{{u"id"_s, idOf(layout, variant)},
                                                   {u"layout"_s, layout},
                                                   {u"variant"_s, variant},
                                                   {u"name"_s, text.isEmpty() ? idOf(layout, variant) : text},
                                                   {u"keys"_s, (idOf(layout, variant) + u' ' + text).toLower()}});
                }
            }
            if (!path.isEmpty()) {
                path.removeLast();
            }
            break;
        default:
            break;
        }
    }
    return m_available;
}

QVariantList InputConfig::layouts() const
{
    KConfig config = kdeutil::user(u"kxkbrc"_s);
    const KConfigGroup g(&config, u"Layout"_s);
    const QStringList list = g.readEntry("LayoutList", QStringList());
    const QStringList variants = g.readEntry("VariantList", QStringList());
    QVariantList result;
    for (qsizetype i = 0; i < list.size() && i < MaxLayouts; ++i) {
        const QString layout = list.at(i);
        const QString variant = i < variants.size() ? variants.at(i) : QString();
        const QString id = idOf(layout, variant);
        if (!validLayoutId(id)) {
            continue;
        }
        result.append(QVariantMap{{u"id"_s, id}, {u"layout"_s, layout}, {u"variant"_s, variant}});
    }
    return result;
}

bool InputConfig::setLayouts(const QStringList &list)
{
    QStringList ids = list;
    if (ids.isEmpty() || ids.size() > MaxLayouts || ids.removeDuplicates() > 0) {
        return false;
    }
    QStringList layouts;
    QStringList variants;
    QStringList names;
    for (const QString &id : ids) {
        if (!validLayoutId(id)) {
            return false;
        }
        layouts << layoutOf(id);
        variants << variantOf(id);
        names << QString();
    }
    // A list of one empty string would be saved as "\0", which
    // setxkbmap-style readers choke on: kcm_keyboard saves an empty list.
    if (variants.size() == 1 && variants.constFirst().isEmpty()) {
        variants.clear();
    }
    if (names.size() == 1) {
        names.clear();
    }
    KConfig config = kdeutil::user(u"kxkbrc"_s);
    KConfigGroup g(&config, u"Layout"_s);
    g.writeEntry("Use", true, kdeutil::Notify);
    g.writeEntry("LayoutList", layouts, kdeutil::Notify);
    g.writeEntry("VariantList", variants, kdeutil::Notify);
    g.writeEntry("DisplayNames", names, kdeutil::Notify);
    return config.sync();
}

QVariantMap InputConfig::keyboard() const
{
    KConfig config = kdeutil::user(u"kcminputrc"_s);
    const KConfigGroup g(&config, u"Keyboard"_s);
    const int numLock = g.readEntry("NumLock", 2);
    return {
        {u"delay"_s, std::clamp(g.readEntry("RepeatDelay", 600), 100, 5000)},
        {u"rate"_s, std::clamp(g.readEntry("RepeatRate", 25.0), 0.2, 200.0)},
        {u"numLock"_s, numLock == 0 ? u"on"_s : numLock == 1 ? u"off"_s : u"keep"_s},
    };
}

bool InputConfig::setRepeat(int delay, double rate)
{
    if (delay < 100 || delay > 5000 || !(rate >= 0.2 && rate <= 200.0)) {
        return false;
    }
    KConfig config = kdeutil::user(u"kcminputrc"_s);
    KConfigGroup g(&config, u"Keyboard"_s);
    g.writeEntry("RepeatDelay", delay, kdeutil::Notify);
    g.writeEntry("RepeatRate", rate, kdeutil::Notify);
    return config.sync();
}

bool InputConfig::setNumLock(const QString &state)
{
    // kcm_keyboard's values: STATE_ON 0, STATE_OFF 1, STATE_UNCHANGED 2.
    int value = 2;
    if (state == u"on"_s) {
        value = 0;
    } else if (state == u"off"_s) {
        value = 1;
    } else if (state != u"keep"_s) {
        return false;
    }
    KConfig config = kdeutil::user(u"kcminputrc"_s);
    KConfigGroup(&config, u"Keyboard"_s).writeEntry("NumLock", value, kdeutil::Notify);
    return config.sync();
}

void InputConfig::refresh()
{
    const int generation = ++m_generation;
    auto listNames = [this, generation](const QString &path, bool retry, auto &&self) -> void {
        kdeutil::call(this,
                      u"org.kde.KWin"_s,
                      path,
                      u"org.freedesktop.DBus.Properties"_s,
                      u"Get"_s,
                      {QString::fromLatin1(ManagerInterface), u"devicesSysNames"_s},
                      [this, generation, retry, self](const QDBusMessage &reply) {
                          if (generation != m_generation) {
                              return;
                          }
                          if (reply.type() != QDBusMessage::ReplyMessage || reply.arguments().isEmpty()) {
                              if (retry) {
                                  self(u"/org/kde/KWin/InputDeviceManager"_s, false, self);
                              } else {
                                  m_list.clear();
                                  m_devices = {{u"known"_s, false}};
                                  Q_EMIT devicesChanged();
                              }
                              return;
                          }
                          static const QRegularExpression valid(u"\\A[A-Za-z0-9_-]{1,32}\\z"_s);
                          QStringList names;
                          for (const QString &n : unwrap<QStringList>(reply.arguments().at(0))) {
                              if (valid.match(n).hasMatch() && names.size() < MaxDevices) {
                                  names << n;
                              }
                          }
                          m_list.clear();
                          if (names.isEmpty()) {
                              publish();
                              return;
                          }
                          m_waiting = int(names.size());
                          for (const QString &n : names) {
                              readDevice(n);
                          }
                      });
    };
    listNames(u"/org/kde/KWin/InputDevice"_s, true, listNames);
}

void InputConfig::readDevice(const QString &sysName)
{
    const int generation = m_generation;
    kdeutil::call(this,
                  u"org.kde.KWin"_s,
                  u"/org/kde/KWin/InputDevice/"_s + sysName,
                  u"org.freedesktop.DBus.Properties"_s,
                  u"GetAll"_s,
                  {QString::fromLatin1(DeviceInterface)},
                  [this, generation, sysName](const QDBusMessage &reply) {
                      if (generation != m_generation) {
                          return;
                      }
                      if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
                          const QVariantMap props = unwrap<QVariantMap>(reply.arguments().at(0));
                          if (props.value(u"pointer"_s).toBool()) {
                              m_list.append({sysName, props.value(u"touchpad"_s).toBool(), props});
                          }
                      }
                      if (--m_waiting <= 0) {
                          publish();
                      }
                  });
}

void InputConfig::publish()
{
    bool canSpeed = false;
    bool canNatural = false;
    bool canTap = false;
    bool canLeft = false;
    int touchpads = 0;
    QVariant speed;
    QVariant natural;
    QVariant tap;
    QVariant left;
    // Touchpads last: a mouse answers for the shared rows first.
    QList<Device> sorted = m_list;
    std::stable_sort(sorted.begin(), sorted.end(), [](const Device &a, const Device &b) {
        return !a.touchpad && b.touchpad;
    });
    for (const Device &d : std::as_const(sorted)) {
        touchpads += d.touchpad ? 1 : 0;
        if (d.props.value(u"supportsPointerAcceleration"_s).toBool()) {
            canSpeed = true;
            if (!speed.isValid()) {
                speed = std::clamp(d.props.value(u"pointerAcceleration"_s).toDouble(), -1.0, 1.0);
            }
        }
        if (d.props.value(u"supportsNaturalScroll"_s).toBool()) {
            canNatural = true;
            // A touchpad's setting is the one people mean.
            if (d.touchpad || !natural.isValid()) {
                natural = d.props.value(u"naturalScroll"_s).toBool();
            }
        }
        if (d.touchpad && d.props.contains(u"tapToClick"_s)) {
            canTap = true;
            if (!tap.isValid()) {
                tap = d.props.value(u"tapToClick"_s).toBool();
            }
        }
        if (d.props.value(u"supportsLeftHanded"_s).toBool()) {
            canLeft = true;
            if (!left.isValid()) {
                left = d.props.value(u"leftHanded"_s).toBool();
            }
        }
    }
    m_devices = {
        {u"known"_s, true},
        {u"pointers"_s, int(m_list.size())},
        {u"touchpads"_s, touchpads},
        {u"speed"_s, speed.isValid() ? speed.toDouble() : 0.0},
        {u"naturalScroll"_s, natural.toBool()},
        {u"tap"_s, tap.toBool()},
        {u"leftHanded"_s, left.toBool()},
        {u"canSpeed"_s, canSpeed},
        {u"canNatural"_s, canNatural},
        {u"canTap"_s, canTap},
        {u"canLeftHanded"_s, canLeft},
    };
    Q_EMIT devicesChanged();
}

void InputConfig::setProperty(const QString &name, const QVariant &value, bool touchpadOnly)
{
    static const QHash<QString, QString> supports{
        {u"pointerAcceleration"_s, u"supportsPointerAcceleration"_s},
        {u"naturalScroll"_s, u"supportsNaturalScroll"_s},
        {u"leftHanded"_s, u"supportsLeftHanded"_s},
    };
    for (Device &d : m_list) {
        if (touchpadOnly && !d.touchpad) {
            continue;
        }
        const QString support = supports.value(name);
        if (!support.isEmpty() && !d.props.value(support).toBool()) {
            continue;
        }
        d.props.insert(name, value);
        kdeutil::call(this,
                      u"org.kde.KWin"_s,
                      u"/org/kde/KWin/InputDevice/"_s + d.sysName,
                      u"org.freedesktop.DBus.Properties"_s,
                      u"Set"_s,
                      {QString::fromLatin1(DeviceInterface), name, QVariant::fromValue(QDBusVariant(value))},
                      [this](const QDBusMessage &reply) {
                          if (reply.type() == QDBusMessage::ErrorMessage) {
                              Q_EMIT failed(tr("The desktop didn't accept the change."));
                              refresh();
                          }
                      });
    }
    publish();
}

void InputConfig::setSpeed(double speed)
{
    setProperty(u"pointerAcceleration"_s, std::clamp(speed, -1.0, 1.0));
}

void InputConfig::setNaturalScroll(bool on)
{
    setProperty(u"naturalScroll"_s, on);
}

void InputConfig::setTapToClick(bool on)
{
    setProperty(u"tapToClick"_s, on, true);
}

void InputConfig::setLeftHanded(bool on)
{
    setProperty(u"leftHanded"_s, on);
}
