#include "kcmcatalog.h"

#include <KPluginMetaData>

#include <QCollator>
#include <QRegularExpression>
#include <QSet>
#include <QVariantMap>

#include <algorithm>

// Plugin metadata is shown as plain text, but capped: a plugin's name or
// description could be any length.
static QString capped(const QString &text, qsizetype max)
{
    if (text.size() <= max) {
        return text;
    }
    qsizetype cut = max - 1;
    // Not between the two halves of a surrogate pair.
    if (text.at(cut - 1).isHighSurrogate()) {
        --cut;
    }
    return text.left(cut) + QChar(0x2026);
}

// Only an icon theme name: a path or URL would make the icon load a file of
// the plugin's choosing.
static QString themeIcon(const QString &name)
{
    // anchoredPattern: "$" alone would also match before a final newline.
    static const QRegularExpression valid(QRegularExpression::anchoredPattern(QStringLiteral("[A-Za-z0-9._-]{1,128}")));
    return valid.match(name).hasMatch() ? name : QString();
}

QVariantList KcmCatalog::list()
{
    if (m_listed) {
        return m_list;
    }
    // System Settings' two plugin folders (QML and QWidgets KCMs).
    static const QStringList folders = {QStringLiteral("plasma/kcms/systemsettings"), QStringLiteral("plasma/kcms/systemsettings_qwidgets")};
    QList<KPluginMetaData> plugins;
    QSet<QString> seen;
    for (const QString &folder : folders) {
        for (const KPluginMetaData &md : KPluginMetaData::findPlugins(folder)) {
            // Settings opens only KCM names (settings-registry's kcm.rs);
            // another ID would be listed but refused when clicked.
            if (!md.isValid() || !md.pluginId().startsWith(QLatin1String("kcm")) || seen.contains(md.pluginId())) {
                continue;
            }
            // Plasma Mobile's (kcm_mobile_*) say which form factors they are for.
            const QStringList forms = md.formFactors();
            if (!forms.isEmpty() && !forms.contains(QLatin1String("desktop"))) {
                continue;
            }
            seen.insert(md.pluginId());
            plugins.append(md);
        }
    }
    QCollator collator;
    collator.setCaseSensitivity(Qt::CaseInsensitive);
    std::sort(plugins.begin(), plugins.end(), [&collator](const KPluginMetaData &a, const KPluginMetaData &b) {
        return collator.compare(a.name(), b.name()) < 0;
    });

    QVariantList out;
    out.reserve(plugins.size());
    for (const KPluginMetaData &md : std::as_const(plugins)) {
        out.append(QVariantMap{
            {QStringLiteral("id"), md.pluginId()},
            {QStringLiteral("name"), capped(md.name().isEmpty() ? md.pluginId() : md.name(), 200)},
            {QStringLiteral("description"), capped(md.description(), 400)},
            {QStringLiteral("icon"), themeIcon(md.iconName())},
            {QStringLiteral("category"), capped(md.value(QStringLiteral("X-KDE-System-Settings-Parent-Category")), 64)},
        });
    }
    m_list = out;
    m_listed = true;
    return m_list;
}

bool KcmCatalog::installed(const QString &name)
{
    // kcmshell6's folders: System Settings', Info Center's, the desktop's and
    // the plain one.
    static const QStringList folders = {
        QStringLiteral("plasma/kcms/systemsettings"),
        QStringLiteral("plasma/kcms/systemsettings_qwidgets"),
        QStringLiteral("plasma/kcms/kinfocenter"),
        QStringLiteral("plasma/kcms/desktop"),
        QStringLiteral("plasma/kcms"),
    };
    if (name.isEmpty()) {
        return false;
    }
    if (m_installed.contains(name)) {
        return true;
    }
    const bool found = std::any_of(folders.cbegin(), folders.cend(), [&name](const QString &folder) {
        return KPluginMetaData::findPluginById(folder, name).isValid();
    });
    if (found) {
        m_installed.insert(name);
    }
    return found;
}
