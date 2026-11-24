#pragma once
#include <cstddef>
#include <string>

struct cJSON;

// Une ligne JSON decodee, possedee par l objet.
//
// Tous les accesseurs rendent la valeur de repli quand la cle est absente ou
// d un autre type : c est la regle « cle absente = valeur inchangee » du
// contrat, l appelant passe la valeur courante en repli.
//
// Partage par les deux transports : link_socket sur desktop, link_ble sur la
// board. Aucune dependance a Slint, pour rester testable seul.
class JsonLine {
public:
    JsonLine() = default;
    ~JsonLine();

    JsonLine(const JsonLine &) = delete;
    JsonLine &operator=(const JsonLine &) = delete;

    // data n a pas besoin d etre termine par un octet nul : le transport BLE
    // fournit un tampon brut. D ou cJSON_ParseWithLength.
    // Rend false si la ligne n est pas un objet JSON valide.
    bool parse(const char *data, size_t len);

    bool has(const char *key) const;
    float number(const char *key, float fallback) const;
    int integer(const char *key, int fallback) const;
    std::string text(const char *key, const std::string &fallback) const;

private:
    void reset();

    cJSON *root_ = nullptr;
};
