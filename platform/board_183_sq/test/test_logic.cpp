#undef NDEBUG
#include <algorithm>
#include <cassert>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include "axp2101.h"
#include "es8311.h"
#include "line_buffer.h"
#include "nvs_migration.h"
#include "pcf85063.h"
#include "vbat_log.h"
#include "wav.h"

using namespace board_183_sq;

static void migration_nvs() {
    using V = std::vector<std::string>;
    assert((cles_a_migrer({"etat", "cfg", "volume", "tz", "bilan"}, {}) == V{"etat", "cfg", "volume", "tz", "bilan"}));
    assert((cles_a_migrer({"etat", "cfg"}, {"etat"}) == V{"cfg"}));
    assert(cles_a_migrer({}, {"etat"}).empty());
    assert((cles_a_effacer({"etat", "cfg", "tz"}, {}) == V{"etat", "cfg", "tz"}));
    assert((cles_a_effacer({"etat", "cfg", "tz"}, {"cfg"}) == V{"etat", "tz"}));
    assert(cles_a_effacer({"etat"}, {"etat"}).empty());
    assert(std::string(kEspaceAncien) == "tagamoshi");
    assert(std::string(kEspace) == "osampo");
}

static void roundtrip(int64_t ep, const uint8_t exp[7]) {
    uint8_t r[7];
    epoch_to_pcf(ep, r);
    assert(memcmp(r, exp, 7) == 0);
    int64_t back = 0;
    assert(pcf_to_epoch(r, back));
    assert(back == ep);
}

static void test_rtc() {
    // 2026-11-20 03:00:00 UTC (vendredi = 5), date -u -d @1795143600
    const uint8_t a[7] = {0x00, 0x00, 0x03, 0x20, 0x05, 0x11, 0x26};
    roundtrip(1795143600LL, a);
    // 2000-01-01 00:00:00 (samedi = 6)
    const uint8_t b[7] = {0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x00};
    roundtrip(946684800LL, b);
    // 2099-12-31 23:59:59 (jeudi = 4)
    const uint8_t c[7] = {0x59, 0x59, 0x23, 0x31, 0x04, 0x12, 0x99};
    roundtrip(4102444799LL, c);
    // 2024-02-29 12:34:56 (jeudi, bissextile) = 1709210096
    const uint8_t d[7] = {0x56, 0x34, 0x12, 0x29, 0x04, 0x02, 0x24};
    roundtrip(1709210096LL, d);
}

static void oscillateur_arrete_invalide() {
    uint8_t r[7] = {0x80, 0x00, 0x03, 0x20, 0x05, 0x11, 0x26};
    int64_t e = 42;
    assert(!pcf_to_epoch(r, e));
}

static void bornes_rejetees() {
    int64_t e;
    uint8_t m13[7] = {0x00, 0x00, 0x03, 0x20, 0x05, 0x13, 0x26};
    assert(!pcf_to_epoch(m13, e));
    uint8_t m0[7] = {0x00, 0x00, 0x03, 0x20, 0x05, 0x00, 0x26};
    assert(!pcf_to_epoch(m0, e));
    uint8_t j31[7] = {0x00, 0x00, 0x03, 0x31, 0x05, 0x11, 0x26};  // 31 novembre
    assert(!pcf_to_epoch(j31, e));
    uint8_t h24[7] = {0x00, 0x00, 0x24, 0x20, 0x05, 0x11, 0x26};
    assert(!pcf_to_epoch(h24, e));
    uint8_t bad[7] = {0x0A, 0x00, 0x03, 0x20, 0x05, 0x11, 0x26};  // BCD invalide
    assert(!pcf_to_epoch(bad, e));
}

// WAV synthetique : fmt (canaux, frequence) + chunk optionnel + data de 4 echantillons.
static std::vector<uint8_t> make_wav(uint16_t ch, uint32_t rate, bool junk_chunk) {
    std::vector<uint8_t> v;
    auto s = [&](const char *t) { v.insert(v.end(), t, t + 4); };
    auto w32 = [&](uint32_t x) { for (int i = 0; i < 4; ++i) v.push_back((x >> (8 * i)) & 0xFF); };
    auto w16 = [&](uint16_t x) { v.push_back(x & 0xFF); v.push_back(x >> 8); };
    s("RIFF"); w32(0); s("WAVE");
    s("fmt "); w32(16); w16(1); w16(ch); w32(rate); w32(rate * 2 * ch); w16(2 * ch); w16(16);
    if (junk_chunk) { s("LIST"); w32(5); for (int i = 0; i < 5; ++i) v.push_back('x'); v.push_back(0); }
    s("data"); w32(8);
    for (int i = 0; i < 4; ++i) w16(static_cast<uint16_t>(i + 1));
    return v;
}

static void wav_genere() {
    auto v = make_wav(1, 16000, false);  // data a l'offset 36, corps a 44
    Pcm p{};
    assert(wav_parse(v.data(), v.size(), p));
    assert(p.count == 4);
    assert(p.samples == reinterpret_cast<const int16_t *>(v.data() + 44));
}

static void wav_data_tronque() {
    auto v = make_wav(1, 16000, false);
    v.resize(v.size() - 4);  // data declare 8 octets, 4 presents
    Pcm p{};
    assert(wav_parse(v.data(), v.size(), p));
    assert(p.count == 2);
}

static void wav_data_avant_fmt() {
    std::vector<uint8_t> v;
    auto s = [&](const char *t) { v.insert(v.end(), t, t + 4); };
    auto w32 = [&](uint32_t x) { for (int i = 0; i < 4; ++i) v.push_back((x >> (8 * i)) & 0xFF); };
    s("RIFF"); w32(0); s("WAVE");
    s("data"); w32(4); w32(0);
    Pcm p{};
    assert(!wav_parse(v.data(), v.size(), p));
}

static void wav_chunk_geant_rejete() {
    std::vector<uint8_t> v;
    auto s = [&](const char *t) { v.insert(v.end(), t, t + 4); };
    auto w32 = [&](uint32_t x) { for (int i = 0; i < 4; ++i) v.push_back((x >> (8 * i)) & 0xFF); };
    s("RIFF"); w32(0); s("WAVE");
    s("JUNK"); w32(0xFFFFFFFFu); w32(0); w32(0);
    s("data"); w32(0);
    Pcm p{};
    assert(!wav_parse(v.data(), v.size(), p));
}

static void wav_chunk_list_saute() {
    auto v = make_wav(1, 16000, true);
    Pcm p{};
    assert(wav_parse(v.data(), v.size(), p));
    assert(p.count == 4);
    assert(p.samples[0] == 1 && p.samples[3] == 4);
}

static void wav_stereo_rejete() {
    auto v = make_wav(2, 16000, false);
    Pcm p{};
    assert(!wav_parse(v.data(), v.size(), p));
}

static void wav_44100_rejete() {
    auto v = make_wav(1, 44100, false);
    Pcm p{};
    assert(!wav_parse(v.data(), v.size(), p));
}

static void wav_invalide() {
    Pcm p{};
    uint8_t junk[16] = {0};
    assert(!wav_parse(junk, sizeof junk, p));
    assert(!wav_parse(junk, 4, p));
}

static void morceaux_recolles() {
    LineBuffer lb;
    std::vector<std::string> out;
    auto cb = [&](const std::string &l) { out.push_back(l); };
    lb.feed("{\"ep", 4, cb);
    assert(out.empty());
    const char *b = "och\":1}\n";
    lb.feed(b, strlen(b), cb);
    assert(out.size() == 1 && out[0] == "{\"epoch\":1}");
}

static void ligne_trop_longue_rejetee() {
    LineBuffer lb;
    std::vector<std::string> out;
    auto cb = [&](const std::string &l) { out.push_back(l); };
    std::string big(1100, 'a');
    big += '\n';
    big += "{\"a\":1}\n";
    lb.feed(big.data(), big.size(), cb);
    assert(out.size() == 1 && out[0] == "{\"a\":1}");
}

static void ligne_de_1000_octets_acceptee() {
    LineBuffer lb;
    std::vector<std::string> out;
    auto cb = [&](const std::string &l) { out.push_back(l); };
    std::string ok(1000, 'a');
    ok += '\n';
    std::string trop(1100, 'b');
    trop += "\nfin\n";
    lb.feed(ok.data(), ok.size(), cb);
    lb.feed(trop.data(), trop.size(), cb);
    assert(out.size() == 2 && out[0].size() == 1000 && out[1] == "fin");
}

static void ligne_limite_exacte() {
    LineBuffer lb(8);
    std::vector<std::string> out;
    auto cb = [&](const std::string &l) { out.push_back(l); };
    const char *s = "12345678\n123456789\nok\n";
    lb.feed(s, strlen(s), cb);
    assert(out.size() == 2 && out[0] == "12345678" && out[1] == "ok");
}

static void depassement_puis_feed_suivant() {
    LineBuffer lb(8);
    std::vector<std::string> out;
    auto cb = [&](const std::string &l) { out.push_back(l); };
    const char *a = "aaaaaaaaaaaaaaaa";  // depasse, pas de \n
    lb.feed(a, strlen(a), cb);
    assert(out.empty());
    const char *b = "bbb\nok\n";  // fin de la ligne rejetee, puis ligne valide
    lb.feed(b, strlen(b), cb);
    assert(out.size() == 1 && out[0] == "ok");
}

static void crlf_accepte() {
    LineBuffer lb;
    std::vector<std::string> out;
    auto cb = [&](const std::string &l) { out.push_back(l); };
    const char *s = "abc\r\ndef\r\n";
    lb.feed(s, strlen(s), cb);
    assert(out.size() == 2 && out[0] == "abc" && out[1] == "def");
}

// SP5d2 : lignes Bluetooth recollees
static void line_buffer_reset() {
    LineBuffer lb(4096);
    std::vector<std::string> v;
    auto on = [&](const std::string &l) { v.push_back(l); };
    lb.feed("{\"a\":", 5, on);
    lb.reset(); // deconnexion au milieu
    lb.feed("{\"b\":1}\n", 8, on);
    assert(v.size() == 1 && v[0] == "{\"b\":1}");
}
static void line_buffer_morceaux_de_20() {
    LineBuffer lb(4096);
    std::string ligne = "{" + std::string(300, 'x') + "}\n";
    std::vector<std::string> v;
    for (size_t i = 0; i < ligne.size(); i += 20)
        lb.feed(ligne.data() + i, std::min<size_t>(20, ligne.size() - i),
                [&](const std::string &l) { v.push_back(l); });
    assert(v.size() == 1 && v[0] == ligne.substr(0, ligne.size() - 1));
}
static void line_buffer_4096_rejete() {
    LineBuffer lb(4096);
    std::string trop = std::string(5000, 'x') + "\n{\"ok\":1}\n";
    std::vector<std::string> v;
    lb.feed(trop.data(), trop.size(), [&](const std::string &l) { v.push_back(l); });
    assert(v.size() == 1 && v[0] == "{\"ok\":1}");
    assert(lb.rejected() == 1);
    lb.reset();
    assert(lb.rejected() == 1); // le compteur survit a la deconnexion
}

// SP5d2 : AXP2101
static void axp_decodage() {
    assert(axp_appui_long(kAxpPkeyLong));
    assert(!axp_appui_long(kAxpPkeyShort));
    assert(axp_appui_long(kAxpPkeyLong | kAxpPkeyShort | 0x01));
    assert(!axp_appui_long(0x00));
    assert(axp_vbat_mv(0x0F, 0xA0) == 4000);   // 0x0FA0
    assert(axp_vbat_mv(0xEF, 0xA0) == 4000);   // bits hauts ignores (5 bits utiles)
    assert(axp_vbat_mv(0x0D, 0x48) == 3400);
    assert(axp_avec_appui_long(0xFF, kAxpIrqLevel2s) == 0xEF); // 1110 1111
    assert(axp_avec_appui_long(0x00, kAxpIrqLevel2s) == 0x20);
}

void es8311_volume() {
    // 0xBF = 0 dB, pas de 0,5 dB ; 0xB2 = -6,5 dB = l'ancien 70 % (C10).
    assert(es8311_vol_reg(0.0f) == 0xBF);
    assert(es8311_vol_reg(-6.5f) == 0xB2);
    assert(es8311_vol_reg(-0.5f) == 0xBE);
    assert(es8311_vol_reg(-12.5f) == 0xA6);
    assert(es8311_vol_reg(-200.0f) == 0x00);
    assert(es8311_vol_reg(100.0f) == 0xFF);
}

static void anneau_garde_les_720_dernieres() {
    VbatLog log;
    for (int i = 1; i <= 800; ++i) log.push(static_cast<uint16_t>(3000 + i));
    std::vector<uint16_t> v = log.values();
    assert(v.size() == 720);
    assert(v.front() == 3081 && v.back() == 3800);
    VbatLog relu;
    assert(relu.decode(log.encode()));
    assert(relu.values() == v);
    assert(!relu.decode("x"));
    assert(relu.values() == v); // inchange si invalide
    VbatLog vide;
    assert(vide.decode(vide.encode()) && vide.values().empty());
}

int main() {
    migration_nvs();
    test_rtc();
    oscillateur_arrete_invalide();
    bornes_rejetees();
    wav_genere();
    wav_data_tronque();
    wav_data_avant_fmt();
    wav_chunk_geant_rejete();
    wav_chunk_list_saute();
    wav_stereo_rejete();
    wav_44100_rejete();
    wav_invalide();
    morceaux_recolles();
    ligne_trop_longue_rejetee();
    ligne_de_1000_octets_acceptee();
    ligne_limite_exacte();
    depassement_puis_feed_suivant();
    crlf_accepte();
    line_buffer_reset();
    line_buffer_morceaux_de_20();
    line_buffer_4096_rejete();
    axp_decodage();
    es8311_volume();
    anneau_garde_les_720_dernieres();
    puts("test_logic OK");
    return 0;
}
