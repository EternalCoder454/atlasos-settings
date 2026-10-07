#include "powerconfig.h"

#include <KConfig>
#include <KConfigGroup>

using namespace Qt::StringLiterals;

namespace
{
// The file, groups and keys of kcm_powerdevilprofilesconfig (Plasma 6.7).
constexpr auto File = "powerdevilrc";
constexpr auto DisplayGroup = "Display";
constexpr auto SuspendGroup = "SuspendAndShutdown";

constexpr int ActionNothing = 0;
constexpr int ActionSleep = 1;
// The most a timeout may be: a day.
constexpr int MaxSeconds = 24 * 3600;

// PowerDevil's defaults for a profile (ProfileDefaults), used for what the
// file doesn't say.
struct Defaults {
    int screenOff;
    int sleep;
};

Defaults defaultsFor(const QString &profile)
{
    if (profile == u"Battery") {
        return {300, 600};
    }
    if (profile == u"LowBattery") {
        return {120, 300};
    }
    return {600, 900};
}

KConfig open()
{
    return KConfig(QString::fromLatin1(File), KConfig::NoGlobals);
}

KConfigGroup group(KConfig &config, const QString &profile, const char *sub)
{
    return KConfigGroup(&config, profile).group(QString::fromLatin1(sub));
}

bool saved(KConfig &config)
{
    // KConfig writes a new file and renames it over the old one.
    return config.sync();
}

int seconds(int value)
{
    return value < 0 ? 0 : qMin(value, MaxSeconds);
}
}

bool PowerConfig::validProfile(const QString &profile)
{
    return profile == u"AC" || profile == u"Battery" || profile == u"LowBattery";
}

QVariantMap PowerConfig::read(const QString &profile) const
{
    if (!validProfile(profile)) {
        return {};
    }
    KConfig config = open();
    const Defaults defaults = defaultsFor(profile);
    const KConfigGroup display = group(config, profile, DisplayGroup);
    const KConfigGroup suspend = group(config, profile, SuspendGroup);

    int screenOff = 0;
    if (display.readEntry("TurnOffDisplayWhenIdle", true)) {
        screenOff = seconds(display.readEntry("TurnOffDisplayIdleTimeoutSec", defaults.screenOff));
    }
    int sleep = 0;
    if (suspend.readEntry("AutoSuspendAction", ActionSleep) != ActionNothing) {
        sleep = seconds(suspend.readEntry("AutoSuspendIdleTimeoutSec", defaults.sleep));
    }
    return {
        {u"screenOff"_s, screenOff},
        {u"sleep"_s, sleep},
        {u"lid"_s, suspend.readEntry("LidAction", ActionSleep)},
        {u"powerButton"_s, suspend.readEntry("PowerButtonAction", 16)},
    };
}

bool PowerConfig::setScreenOff(const QString &profile, int secs)
{
    if (!validProfile(profile) || secs < 0 || secs > MaxSeconds) {
        return false;
    }
    KConfig config = open();
    KConfigGroup display = group(config, profile, DisplayGroup);
    display.writeEntry("TurnOffDisplayWhenIdle", secs > 0, KConfigBase::Persistent | KConfigBase::Notify);
    display.writeEntry("TurnOffDisplayIdleTimeoutSec", secs > 0 ? secs : -1, KConfigBase::Persistent | KConfigBase::Notify);
    return saved(config);
}

bool PowerConfig::setSleep(const QString &profile, int secs)
{
    if (!validProfile(profile) || secs < 0 || secs > MaxSeconds) {
        return false;
    }
    KConfig config = open();
    KConfigGroup suspend = group(config, profile, SuspendGroup);
    if (secs == 0) {
        suspend.writeEntry("AutoSuspendAction", ActionNothing, KConfigBase::Persistent | KConfigBase::Notify);
    } else {
        const int current = suspend.readEntry("AutoSuspendAction", ActionSleep);
        suspend.writeEntry("AutoSuspendAction", current == ActionNothing ? ActionSleep : current, KConfigBase::Persistent | KConfigBase::Notify);
        suspend.writeEntry("AutoSuspendIdleTimeoutSec", secs, KConfigBase::Persistent | KConfigBase::Notify);
    }
    return saved(config);
}

bool PowerConfig::setLid(const QString &profile, int action)
{
    // Nothing, sleep, hibernate, shut down, lock, turn off the screen.
    static const QList<int> allowed{0, 1, 2, 8, 32, 64};
    if (!validProfile(profile) || !allowed.contains(action)) {
        return false;
    }
    KConfig config = open();
    group(config, profile, SuspendGroup).writeEntry("LidAction", action, KConfigBase::Persistent | KConfigBase::Notify);
    return saved(config);
}

bool PowerConfig::setPowerButton(const QString &profile, int action)
{
    // Nothing, sleep, shut down, the logout screen, turn off the screen.
    static const QList<int> allowed{0, 1, 8, 16, 64};
    if (!validProfile(profile) || !allowed.contains(action)) {
        return false;
    }
    KConfig config = open();
    group(config, profile, SuspendGroup).writeEntry("PowerButtonAction", action, KConfigBase::Persistent | KConfigBase::Notify);
    return saved(config);
}
