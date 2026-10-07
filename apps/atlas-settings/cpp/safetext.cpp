#include "safetext.h"

#include <QChar>

namespace AtlasText
{
QString safeText(const QString &text, int max)
{
    QString out;
    bool space = true; // leading blanks are dropped
    for (const QChar c : text) {
        if (c.isSpace()) {
            if (!space) {
                out += QLatin1Char(' ');
            }
            space = true;
            continue;
        }
        switch (c.category()) {
        case QChar::Other_Control:
        case QChar::Other_Format:
        case QChar::Other_Surrogate:
        case QChar::Other_PrivateUse:
        case QChar::Other_NotAssigned:
        case QChar::Separator_Line:
        case QChar::Separator_Paragraph:
            continue;
        default:
            break;
        }
        out += c;
        space = false;
    }
    if (out.endsWith(QLatin1Char(' '))) {
        out.chop(1);
    }
    return out.left(max);
}
}
