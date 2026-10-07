// A stand-in for KWin and Plasma's shell on the session bus of a smoke run
// (scripts of the page smoke tests): a mouse and a touchpad, three virtual
// desktops and a dock at the bottom that hides itself. It prints what it is
// asked to change. Only ever run it on a private bus.

#include "fakes.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDebug>
#include <QTimer>

int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qWarning() << "fake_desktop: no session bus";
        return 1;
    }
    fakes::FakeShell shell;
    shell.answer = QStringLiteral(R"({"dock": {"location": "bottom", "hiding": "autohide", "height": 60}, "bar": {"count": 3, "hides": true}})");
    fakes::FakeDesktops desktops;
    desktops.count = 3;
    fakes::FakeKWin kwin;
    fakes::FakeEffects effects;
    fakes::FakeManager manager;
    fakes::FakeDevice mouse;
    fakes::FakeDevice pad;
    pad.touchpad = true;
    pad.naturalScroll = true;
    pad.tapToClick = true;
    pad.pointerAcceleration = 0.2;
    manager.names = {QStringLiteral("event1"), QStringLiteral("event2")};

    bus.registerService(QStringLiteral("org.kde.plasmashell"));
    bus.registerService(QStringLiteral("org.kde.KWin"));
    bus.registerObject(QStringLiteral("/PlasmaShell"), &shell, QDBusConnection::ExportAllSlots);
    bus.registerObject(QStringLiteral("/VirtualDesktopManager"), &desktops, QDBusConnection::ExportAllSlots | QDBusConnection::ExportAllProperties);
    bus.registerObject(QStringLiteral("/KWin"), &kwin, QDBusConnection::ExportAllSlots);
    bus.registerObject(QStringLiteral("/Effects"), &effects, QDBusConnection::ExportAllSlots);
    bus.registerObject(QStringLiteral("/org/kde/KWin/InputDevice"), &manager, QDBusConnection::ExportAllProperties);
    bus.registerObject(QStringLiteral("/org/kde/KWin/InputDevice/event1"), &mouse, QDBusConnection::ExportAllProperties);
    bus.registerObject(QStringLiteral("/org/kde/KWin/InputDevice/event2"), &pad, QDBusConnection::ExportAllProperties);
    qInfo() << "fake_desktop: ready";
    return app.exec();
}
