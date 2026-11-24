// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>

#include "decode.h"
#include <cstdio>
#include <cstring>

int main()
{
    // trame complete
    {
        DataFrame d;
        JsonLine l;
        // JsonLine n a pas d accesseur booleen : claude_wait voyage en 0/1.
        const char *s = "{\"epoch\":1795100000,\"tz_s\":3600,\"sage\":42,"
                        "\"actif\":1,\"absence_min\":7,\"absence_finie_min\":31,"
                        "\"absence_finie_age_min\":2,\"claude_wait\":1,\"age\":3}";
        assert(l.parse(s, strlen(s)));
        decode(l, d);
        assert(d.epoch == 1795100000LL);
        assert(d.tz_s == 3600);
        assert(d.sage == 42);
        assert(d.actif == true);
        assert(d.absence_min == 7);
        assert(d.absence_finie_min == 31 && d.absence_finie_age_min == 2);
        assert(d.claude_wait == true);
        assert(d.age == 3);
        assert(!d.online && "decode ne touche jamais online");
    }

    // trame partielle : seule sage change
    {
        DataFrame d;
        d.epoch = 1795000000LL;
        d.tz_s = 7200;
        d.sage = 3;
        d.actif = true;
        d.absence_min = 60;
        d.claude_wait = true;
        d.age = 2;
        JsonLine l;
        const char *s = "{\"sage\":12}";
        assert(l.parse(s, strlen(s)));
        decode(l, d);
        assert(d.sage == 12);
        assert(d.actif == true);
        assert(d.absence_min == 60);
        assert(d.epoch == 1795000000LL);
        assert(d.tz_s == 7200);
        assert(d.claude_wait == true);
        assert(d.age == 2);
        assert(!d.online);
    }

    // online n est jamais touche, meme quand il vaut true
    {
        DataFrame d;
        d.online = true;
        JsonLine l;
        const char *s = "{\"age\":1,\"online\":0}";
        assert(l.parse(s, strlen(s)));
        decode(l, d);
        assert(d.online == true);
    }

    // JSON invalide : parse refuse, trame courante intacte.
    {
        DataFrame d;
        d.sage = 33;
        JsonLine l;
        const char *s = "{\"sage\":12,\"age\":";
        const bool ok = l.parse(s, strlen(s));
        assert(!ok && "un JSON tronque doit etre refuse");
        if (ok) {
            decode(l, d);
        }
        assert(d.sage == 33);
    }

    // R-aa : epoch absente, valeur stockee > 2^31 : intacte (pas de cast int).
    {
        DataFrame d;
        d.epoch = 3000000000LL;
        JsonLine l;
        const char *s = "{\"age\":1}";
        assert(l.parse(s, strlen(s)));
        decode(l, d);
        assert(d.epoch == 3000000000LL);
    }

    std::printf("test_decode: 5 cas OK\n");
    return 0;
}
