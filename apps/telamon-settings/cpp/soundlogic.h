#pragma once

#include <QString>
#include <QVariantMap>

// The rules of the Sound page that need no sound server: volume as a
// percentage, which devices and streams the page lists, and what to call
// them. Plain Qt types, tested without PulseAudio (tests/soundlogic_test.cpp);
// soundmixer.cpp feeds them from PulseAudioQt's objects.
namespace SoundLogic
{
// `volume` (0 .. `normal`, where `normal` is 100 %) as a whole percentage,
// which can be above 100.
int volumePercent(qint64 volume, qint64 normal);
// A volume in the server's units for `percent`, kept within 0 .. `maxPercent`.
qint64 volumeFromPercent(int percent, qint64 normal, int maxPercent = 100);

// A source that only mirrors what a sink plays ("Monitor of ..."), not a
// microphone.
bool isMonitor(const QString &name, const QVariantMap &properties);
// A stream the system plays for its own events (a notification's ding): not
// something to set a volume for.
bool isEventSound(const QVariantMap &properties);

// What kind of device it is, for its icon: headphones, headset, speaker,
// monitor (HDMI or DisplayPort, a TV), bluetooth, usb, microphone, webcam or
// other. From the form factor and the bus the server reports, and the name.
QString deviceKind(const QString &formFactor, const QString &bus, const QString &name, bool input);

// What an app's stream is called: the name the stream gives its app, else its
// client's, else the stream's own, else "Unknown App"; safe to show.
QString appName(const QString &clientName, const QVariantMap &streamProperties);
// The theme icon the app's stream names, "" when it names none or a path.
QString appIcon(const QVariantMap &streamProperties);
}
