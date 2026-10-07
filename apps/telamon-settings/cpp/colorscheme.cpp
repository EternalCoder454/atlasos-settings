#include "colorscheme.h"

#include <KConfig>
#include <KConfigGroup>

#include <QRegularExpression>

using namespace Qt::StringLiterals;

bool ColorSchemeConfig::validName(const QString &name)
{
    static const QRegularExpression re(u"^[A-Za-z0-9._][A-Za-z0-9 ._-]{0,63}$"_s);
    return re.match(name).hasMatch();
}

QString ColorSchemeConfig::current() const
{
    // Globals included: the system's default (/etc/xdg/kdeglobals) stands
    // when the user's file says nothing.
    KConfig config(u"kdeglobals"_s);
    const QString name = KConfigGroup(&config, u"General"_s).readEntry("ColorScheme", QString());
    return validName(name) ? name : QString();
}
