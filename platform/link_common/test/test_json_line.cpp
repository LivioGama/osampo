#include "json_line.h"
// Les tests reposent sur assert : on le garde actif meme sous -DNDEBUG (Release).
#undef NDEBUG
#include <cassert>
#include <cstdio>
#include <cstring>

static void trame_complete()
{
    const char *s = "{\"value\":62.5,\"label\":\"trame 12\",\"state\":\"ok\",\"age\":3}";
    JsonLine l;
    assert(l.parse(s, strlen(s)));
    assert(l.has("value"));
    assert(l.number("value", -1.0f) == 62.5f);
    assert(l.text("label", "defaut") == "trame 12");
    assert(l.text("state", "defaut") == "ok");
    assert(l.integer("age", -1) == 3);
}

static void cle_absente_rend_le_repli()
{
    const char *s = "{\"value\":71}";
    JsonLine l;
    assert(l.parse(s, strlen(s)));
    assert(!l.has("state"));
    assert(l.text("state", "inchange") == "inchange");
    assert(l.number("autre", 42.0f) == 42.0f);
    assert(l.integer("age", 7) == 7);
}

static void json_invalide_est_refuse()
{
    const char *s = "{\"value\":";
    JsonLine l;
    assert(!l.parse(s, strlen(s)));
}

static void tampon_non_termine_par_nul()
{
    // ce que fournira le transport BLE : des octets bruts, pas une chaine C.
    const char buf[] = {'{','"','v','"',':','1','2','}','P','O','U','B','E','L','L','E'};
    JsonLine l;
    assert(l.parse(buf, 8));
    assert(l.number("v", -1.0f) == 12.0f);
}

static void mauvais_type_rend_le_repli()
{
    const char *s = "{\"value\":\"texte\",\"label\":5}";
    JsonLine l;
    assert(l.parse(s, strlen(s)));
    assert(l.number("value", 9.0f) == 9.0f);
    assert(l.text("label", "repli") == "repli");
}

static void reutilisation_de_l_objet()
{
    JsonLine l;
    const char *a = "{\"value\":1}";
    const char *b = "{\"value\":2}";
    assert(l.parse(a, strlen(a)));
    assert(l.number("value", -1.0f) == 1.0f);
    assert(l.parse(b, strlen(b)));
    assert(l.number("value", -1.0f) == 2.0f);
}

int main()
{
    trame_complete();
    cle_absente_rend_le_repli();
    json_invalide_est_refuse();
    tampon_non_termine_par_nul();
    mauvais_type_rend_le_repli();
    reutilisation_de_l_objet();
    printf("test_json_line: 6 cas OK\n");
    return 0;
}
