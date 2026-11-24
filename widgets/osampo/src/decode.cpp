#include "decode.h"

// JsonLine n a ni accesseur booleen ni entier 64 bits (platform/ est hors
// perimetre) :
//  - epoch passe par integer (int 32 bits, suffit jusqu en 2038) ; number
//    rendrait un float, trop court (24 bits de mantisse) pour une epoch. Elle
//    n est lue que si la cle est presente : la valeur stockee (int64) ne sert
//    jamais de repli, un cast en int la tronquerait ;
//  - claude_wait et actif voyagent en 0/1 via integer : un true JSON n est pas un nombre.
void decode(const JsonLine &line, DataFrame &d)
{
    if (line.has("epoch")) d.epoch = line.integer("epoch", 0);
    d.tz_s = line.integer("tz_s", d.tz_s);
    d.sage = line.integer("sage", d.sage);
    d.actif = line.integer("actif", d.actif ? 1 : 0) != 0;
    d.absence_min = line.integer("absence_min", d.absence_min);
    d.absence_finie_min = line.integer("absence_finie_min", d.absence_finie_min);
    d.absence_finie_age_min = line.integer("absence_finie_age_min", d.absence_finie_age_min);
    d.claude_wait = line.integer("claude_wait", d.claude_wait ? 1 : 0) != 0;
    d.age = line.integer("age", d.age);
}
