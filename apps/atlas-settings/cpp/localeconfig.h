#pragma once

#include <QObject>
#include <QVariantMap>

// The user's language and formats as Plasma keeps them: plasma-localerc,
// [Formats] LANG and LC_* and [Translations] LANGUAGE, the keys Plasma
// 6.7's kcm_regionandlang writes and startplasma reads at sign-in. Written
// through KConfig (atomically), never by hand. A key that isn't set
// follows the language ("Inherit Language" in the KCM). Local files only:
// nothing here waits on D-Bus.
class LocaleConfig : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // {lang, language, time, numeric}: LANG, LANGUAGE, LC_TIME and
    // LC_NUMERIC as written, "" for the ones not set.
    Q_INVOKABLE QVariantMap read() const;

    // The language: LANG = `locale` (with .UTF-8) and LANGUAGE its
    // language_REGION, as the KCM writes them. False for a name that isn't
    // a locale, or when the file can't be written.
    Q_INVOKABLE bool setLanguage(const QString &locale);

    // Numbers, dates, currency, measurement and paper as in `locale` (every
    // LC_* the KCM writes), or "" to follow the language again.
    Q_INVOKABLE bool setFormats(const QString &locale);

    // Only how times are written (LC_TIME), or "" to follow the formats.
    Q_INVOKABLE bool setTimeFormat(const QString &locale);

    // Whether `locale` is a locale name this writes (the check
    // settings_sys::locale::valid_locale makes).
    Q_INVOKABLE static bool validLocale(const QString &locale);

private:
    bool write(const QStringList &keys, const QString &value);
};
