#pragma once

// Fakes of the services the page backends talk to (KWin's input devices,
// virtual desktops and effects, Plasma's shell), for the tests and for the
// smoke runs on a private session bus. Each one answers with the names and
// types of the real service.

#include <QObject>
#include <QStringList>

using namespace Qt::StringLiterals;

namespace fakes
{
// A KWin input device on the bus (org.kde.KWin.InputDevice).
class FakeDevice : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.KWin.InputDevice")
    Q_PROPERTY(bool pointer MEMBER pointer)
    Q_PROPERTY(bool touchpad MEMBER touchpad)
    Q_PROPERTY(bool supportsPointerAcceleration MEMBER supportsPointerAcceleration)
    Q_PROPERTY(double pointerAcceleration MEMBER pointerAcceleration)
    Q_PROPERTY(bool supportsNaturalScroll MEMBER supportsNaturalScroll)
    Q_PROPERTY(bool naturalScroll MEMBER naturalScroll)
    Q_PROPERTY(bool supportsLeftHanded MEMBER supportsLeftHanded)
    Q_PROPERTY(bool leftHanded MEMBER leftHanded)
    Q_PROPERTY(bool tapToClick MEMBER tapToClick)

public:
    bool pointer = true;
    bool touchpad = false;
    bool supportsPointerAcceleration = true;
    double pointerAcceleration = 0.0;
    bool supportsNaturalScroll = true;
    bool naturalScroll = false;
    bool supportsLeftHanded = true;
    bool leftHanded = false;
    bool tapToClick = false;
};

class FakeManager : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.KWin.InputDeviceManager")
    Q_PROPERTY(QStringList devicesSysNames MEMBER names)

public:
    QStringList names;
};

// Plasma's shell: evaluateScript(), which prints what a script prints.
class FakeShell : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.PlasmaShell")

public:
    QStringList scripts;
    QString answer;

public Q_SLOTS:
    Q_SCRIPTABLE QString evaluateScript(const QString &script)
    {
        scripts << script;
        return script.contains(u"JSON.stringify"_s) ? answer : QString();
    }
};

// KWin's virtual desktops, and the calls other settings make.
class FakeDesktops : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.KWin.VirtualDesktopManager")
    Q_PROPERTY(uint count MEMBER count)

public:
    uint count = 2;
    QStringList created;

public Q_SLOTS:
    Q_SCRIPTABLE void createDesktop(uint position, const QString &name)
    {
        created << QStringLiteral("%1:%2").arg(position).arg(name);
        ++count;
    }
};

class FakeKWin : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.KWin")

public:
    int reconfigured = 0;

public Q_SLOTS:
    Q_SCRIPTABLE void reconfigure()
    {
        ++reconfigured;
    }
};

class FakeEffects : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.kwin.Effects")

public:
    QStringList calls;

public Q_SLOTS:
    Q_SCRIPTABLE void loadEffect(const QString &name)
    {
        calls << u"load:"_s + name;
    }
    Q_SCRIPTABLE void unloadEffect(const QString &name)
    {
        calls << u"unload:"_s + name;
    }
    Q_SCRIPTABLE void reconfigureEffect(const QString &name)
    {
        calls << u"reconfigure:"_s + name;
    }
};
}
