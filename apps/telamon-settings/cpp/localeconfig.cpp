#include "localeconfig.h"

#include <KConfig>
#include <KConfigGroup>

#include <QRegularExpression>

using namespace Qt::StringLiterals;

namespace
{
// The file and groups kcm_regionandlang uses (its .kcfg, Plasma 6.7).
constexpr auto File = "plasma-localerc";
constexpr auto Formats = "Formats";
constexpr auto Translations = "Translations";

// What the KCM's Formats page sets besides LANG.
const QStringList formatKeys()
{
    return {u"LC_NUMERIC"_s,
            u"LC_TIME"_s,
            u"LC_MONETARY"_s,
            u"LC_MEASUREMENT"_s,
            u"LC_PAPER"_s,
            u"LC_ADDRESS"_s,
            u"LC_NAME"_s,
            u"LC_TELEPHONE"_s};
}

// "de_DE.UTF-8" for "de_DE" or "de_DE.UTF-8"; "C.UTF-8" for "C".
QString withCodeset(const QString &locale)
{
    if (locale.contains(u'.')) {
        return locale;
    }
    const qsizetype at = locale.indexOf(u'@');
    return at < 0 ? locale + u".UTF-8"_s : locale.left(at) + u".UTF-8"_s + locale.mid(at);
}

KConfig open()
{
    return KConfig(QString::fromLatin1(File), KConfig::NoGlobals);
}
}

bool LocaleConfig::validLocale(const QString &locale)
{
    // As settings_sys::locale::valid_locale (crates/settings-sys/src/locale.rs).
    static const QRegularExpression re(u"^(C|POSIX|C\\.UTF-8|C\\.utf8|[a-z]{2,3}(_[A-Z0-9]{2,3})?(\\.[A-Za-z0-9-]+)?(@[a-z]+)?)$"_s);
    return locale.size() <= 32 && re.match(locale).hasMatch();
}

QVariantMap LocaleConfig::read() const
{
    KConfig config = open();
    const KConfigGroup formats(&config, QString::fromLatin1(Formats));
    const KConfigGroup translations(&config, QString::fromLatin1(Translations));
    // Shown as plain text, and only names that pass the check: the file is
    // the user's, but anything can have written it.
    auto checked = [](const QString &v) {
        return validLocale(v) ? v : QString();
    };
    QString language = translations.readEntry("LANGUAGE", QString());
    static const QRegularExpression languages(u"^[a-z]{2,3}(_[A-Z0-9]{2,3})?(@[a-z]+)?(:[a-z]{2,3}(_[A-Z0-9]{2,3})?(@[a-z]+)?){0,15}$"_s);
    if (!languages.match(language).hasMatch()) {
        language.clear();
    }
    return {
        {u"lang"_s, checked(formats.readEntry("LANG", QString()))},
        {u"language"_s, language},
        {u"time"_s, checked(formats.readEntry("LC_TIME", QString()))},
        {u"numeric"_s, checked(formats.readEntry("LC_NUMERIC", QString()))},
    };
}

bool LocaleConfig::write(const QStringList &keys, const QString &value)
{
    if (!value.isEmpty() && !validLocale(value)) {
        return false;
    }
    KConfig config = open();
    KConfigGroup formats(&config, QString::fromLatin1(Formats));
    for (const QString &key : keys) {
        if (value.isEmpty()) {
            formats.deleteEntry(key);
        } else {
            formats.writeEntry(key, withCodeset(value));
        }
    }
    // KConfig writes a new file and renames it over the old one.
    return config.sync();
}

bool LocaleConfig::setLanguage(const QString &locale)
{
    if (!validLocale(locale) || locale.startsWith(u'C') || locale == u"POSIX"_s) {
        return false;
    }
    KConfig config = open();
    KConfigGroup formats(&config, QString::fromLatin1(Formats));
    KConfigGroup translations(&config, QString::fromLatin1(Translations));
    formats.writeEntry("LANG", withCodeset(locale));
    // LANGUAGE holds language_REGION without the codeset ("de_DE").
    QString language = locale.section(u'.', 0, 0);
    if (const qsizetype at = locale.indexOf(u'@'); at >= 0 && !language.contains(u'@')) {
        language += locale.mid(at);
    }
    translations.writeEntry("LANGUAGE", language);
    return config.sync();
}

bool LocaleConfig::setFormats(const QString &locale)
{
    return write(formatKeys(), locale);
}

bool LocaleConfig::setTimeFormat(const QString &locale)
{
    return write({u"LC_TIME"_s}, locale);
}
