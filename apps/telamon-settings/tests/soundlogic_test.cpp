#include "soundlogic.h"

#include <QTest>

using namespace SoundLogic;
using namespace Qt::StringLiterals;

class SoundLogicTest : public QObject
{
    Q_OBJECT

private Q_SLOTS:
    void volumeIsAWholePercentage()
    {
        const qint64 normal = 65536;
        QCOMPARE(volumePercent(0, normal), 0);
        QCOMPARE(volumePercent(normal, normal), 100);
        QCOMPARE(volumePercent(normal / 2, normal), 50);
        QCOMPARE(volumePercent(normal * 3 / 2, normal), 150);
        QCOMPARE(volumePercent(19661, normal), 30); // 30 % of 65536, rounded
        QCOMPARE(volumePercent(-5, normal), 0);
        QCOMPARE(volumePercent(100, 0), 0);
    }

    void aPercentageGoesBackToTheSameWholePercentage()
    {
        const qint64 normal = 65536;
        for (int p = 0; p <= 150; ++p) {
            QCOMPARE(volumePercent(volumeFromPercent(p, normal, 150), normal), p);
        }
    }

    void aVolumeStaysWithinTheLimit()
    {
        const qint64 normal = 65536;
        QCOMPARE(volumeFromPercent(250, normal), normal);
        QCOMPARE(volumeFromPercent(250, normal, 150), normal * 3 / 2);
        QCOMPARE(volumeFromPercent(-20, normal), 0);
        QCOMPARE(volumeFromPercent(50, normal, -3), 0);
    }

    void monitorsAreNotMicrophones()
    {
        QVERIFY(isMonitor(u"alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"_s, {}));
        QVERIFY(isMonitor(u"x"_s, {{u"device.class"_s, u"monitor"_s}}));
        QVERIFY(!isMonitor(u"alsa_input.usb-Blue_Yeti.analog-stereo"_s, {{u"device.class"_s, u"sound"_s}}));
        QVERIFY(!isMonitor(u"monitor-of-nothing"_s, {}));
    }

    void eventSoundsAreNotApps()
    {
        QVERIFY(isEventSound({{u"media.role"_s, u"event"_s}}));
        QVERIFY(!isEventSound({{u"media.role"_s, u"music"_s}}));
        QVERIFY(!isEventSound({}));
    }

    void devicesHaveAKind()
    {
        QCOMPARE(deviceKind(u"headphone"_s, {}, u"x"_s, false), u"headphones"_s);
        QCOMPARE(deviceKind(u"headset"_s, u"bluetooth"_s, u"x"_s, false), u"headset"_s);
        QCOMPARE(deviceKind(u"internal"_s, u"pci"_s, u"alsa_output.pci.analog-stereo"_s, false), u"speaker"_s);
        QCOMPARE(deviceKind(u"internal"_s, u"pci"_s, u"alsa_input.pci.analog-stereo"_s, true), u"microphone"_s);
        QCOMPARE(deviceKind({}, u"pci"_s, u"alsa_output.pci-0000_01_00.1.hdmi-stereo"_s, false), u"monitor"_s);
        QCOMPARE(deviceKind({}, u"bluetooth"_s, u"bluez_output.AA_BB.1"_s, false), u"bluetooth"_s);
        QCOMPARE(deviceKind({}, {}, u"bluez_output.AA_BB.1"_s, false), u"bluetooth"_s);
        QCOMPARE(deviceKind({}, u"usb"_s, u"alsa_output.usb-DAC.analog-stereo"_s, false), u"usb"_s);
        QCOMPARE(deviceKind({}, u"usb"_s, u"alsa_input.usb-Blue_Yeti.mono"_s, true), u"microphone"_s);
        QCOMPARE(deviceKind(u"webcam"_s, u"usb"_s, u"alsa_input.usb-cam"_s, true), u"webcam"_s);
        QCOMPARE(deviceKind(u"tv"_s, {}, u"x"_s, false), u"monitor"_s);
        QCOMPARE(deviceKind({}, {}, u"something"_s, false), u"other"_s);
        QCOMPARE(deviceKind({}, {}, u"something"_s, true), u"microphone"_s);
    }

    void appsAreNamedByTheirOwnName()
    {
        QCOMPARE(appName(u"Firefox"_s, {{u"application.name"_s, u"Firefox Web Browser"_s}}), u"Firefox Web Browser"_s);
        QCOMPARE(appName(u"Firefox"_s, {}), u"Firefox"_s);
        QCOMPARE(appName({}, {{u"application.name"_s, u"Spotify"_s}}), u"Spotify"_s);
        QCOMPARE(appName({}, {{u"media.name"_s, u"Playback"_s}}), u"Playback"_s);
        QCOMPARE(appName({}, {}), u"Unknown App"_s);
        // from outside: made safe, and capped
        QCOMPARE(appName(u"  Evil\u0007\n  App "_s, {}), u"Evil App"_s);
        QCOMPARE(appName(QString(500, u'x'), {}).size(), 60);
        // plain text is plain text
        QCOMPARE(appName(u"<b>bold</b>"_s, {}), u"<b>bold</b>"_s);
    }

    void anAppIconIsAThemeNameNeverAPath()
    {
        QCOMPARE(appIcon({{u"application.icon_name"_s, u"firefox"_s}}), u"firefox"_s);
        QCOMPARE(appIcon({{u"application.icon_name"_s, u"org.mozilla.firefox"_s}}), u"org.mozilla.firefox"_s);
        QCOMPARE(appIcon({{u"application.icon_name"_s, u"/etc/passwd"_s}}), QString());
        QCOMPARE(appIcon({{u"application.icon_name"_s, u"../../x"_s}}), QString());
        QCOMPARE(appIcon({{u"application.icon_name"_s, u"file:///x.png"_s}}), QString());
        QCOMPARE(appIcon({{u"application.icon_name"_s, QString(100, u'a')}}), QString());
        QCOMPARE(appIcon({}), QString());
    }
};

QTEST_GUILESS_MAIN(SoundLogicTest)
#include "soundlogic_test.moc"
