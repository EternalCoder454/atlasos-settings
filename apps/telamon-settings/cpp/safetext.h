#pragma once

#include <QString>

namespace TelamonText
{
// A name from outside (a screen's vendor, a sound card, an app's stream) made
// safe to show as plain text: control, invisible format, private-use and
// unassigned characters removed, runs of whitespace collapsed to one space,
// at most `max` characters.
QString safeText(const QString &text, int max = 80);
}
