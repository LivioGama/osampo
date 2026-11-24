#include "storage.h"
#include "esp_log.h"
#include "nvs.h"
#include "nvs_flash.h"
#include "nvs_migration.h"

#include <vector>

static const char *TAG = "storage";
namespace board_183_sq {

static const char *NS = kEspace;

static std::vector<std::string> cles(const char *ns)
{
    std::vector<std::string> out;
    nvs_iterator_t it = nullptr;
    esp_err_t e = nvs_entry_find(NVS_DEFAULT_PART_NAME, ns, NVS_TYPE_STR, &it);
    while (e == ESP_OK) {
        nvs_entry_info_t info;
        nvs_entry_info(it, &info);
        out.emplace_back(info.key);
        e = nvs_entry_next(&it);
    }
    nvs_release_iterator(it);
    return out;
}

// Recopie les cles de l'ancien espace (nom de travail du projet), puis efface de l'ancien
// espace tout ce qui est en securite dans le nouveau. Une cle en echec y reste et sera
// retentee au demarrage suivant.
static void migrer()
{
    const auto anciennes = cles(kEspaceAncien);
    if (anciennes.empty()) return;
    nvs_handle_t a, n;
    if (nvs_open(kEspaceAncien, NVS_READWRITE, &a) != ESP_OK) return;
    if (nvs_open(NS, NVS_READWRITE, &n) != ESP_OK) { nvs_close(a); return; }
    int copiees = 0;
    std::vector<std::string> echouees;
    for (const auto &k : cles_a_migrer(anciennes, cles(NS))) {
        size_t len = 0;
        esp_err_t e = nvs_get_str(a, k.c_str(), nullptr, &len);
        if (e == ESP_ERR_NVS_NOT_FOUND || (e == ESP_OK && len == 0)) continue;  // rien a garder
        std::string v(len, '\0');
        if (e == ESP_OK) e = nvs_get_str(a, k.c_str(), v.data(), &len);
        if (e == ESP_OK) e = nvs_set_str(n, k.c_str(), v.c_str());
        if (e == ESP_OK) { copiees++; continue; }
        ESP_LOGW(TAG, "migration nvs: %s: %s", k.c_str(), esp_err_to_name(e));
        echouees.push_back(k);
    }
    esp_err_t e = nvs_commit(n);
    if (e == ESP_OK) {
        if (echouees.empty()) nvs_erase_all(a);
        else for (const auto &k : cles_a_effacer(anciennes, echouees)) nvs_erase_key(a, k.c_str());
        nvs_commit(a);
    }
    nvs_close(n);
    nvs_close(a);
    ESP_LOGI(TAG, "migration nvs: %d cles (%s -> %s)%s", copiees, kEspaceAncien, NS,
             e != ESP_OK ? " echec du commit" : echouees.empty() ? "" : " incomplete, reprise au prochain demarrage");
}

void storage_init()
{
    esp_err_t e = nvs_flash_init();
    if (e == ESP_ERR_NVS_NO_FREE_PAGES || e == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_LOGW(TAG, "NVS a reformater");
        ESP_ERROR_CHECK(nvs_flash_erase());
        e = nvs_flash_init();
    }
    ESP_ERROR_CHECK(e);
    migrer();
}

std::string storage_load(const char *key)
{
    nvs_handle_t h;
    esp_err_t e = nvs_open(NS, NVS_READONLY, &h);
    if (e != ESP_OK) {
        // espace de noms absent au premier demarrage : normal
        if (e != ESP_ERR_NVS_NOT_FOUND) ESP_LOGW(TAG, "nvs_open(%s): %s", key, esp_err_to_name(e));
        return {};
    }
    std::string out;
    size_t len = 0;
    e = nvs_get_str(h, key, nullptr, &len);
    if (e == ESP_OK && len > 0) {
        out.resize(len);
        e = nvs_get_str(h, key, out.data(), &len);
        if (e == ESP_OK) out.resize(len ? len - 1 : 0);
        else out.clear();
    }
    if (e != ESP_OK && e != ESP_ERR_NVS_NOT_FOUND) ESP_LOGW(TAG, "nvs_get_str(%s): %s", key, esp_err_to_name(e));
    nvs_close(h);
    return out;
}

bool storage_save(const char *key, const std::string &value)
{
    nvs_handle_t h;
    esp_err_t e = nvs_open(NS, NVS_READWRITE, &h);
    if (e != ESP_OK) { ESP_LOGE(TAG, "nvs_open: %s", esp_err_to_name(e)); return false; }
    e = nvs_set_str(h, key, value.c_str());
    if (e == ESP_OK) e = nvs_commit(h);
    if (e != ESP_OK) ESP_LOGE(TAG, "ecriture de %s: %s", key, esp_err_to_name(e));
    nvs_close(h);
    return e == ESP_OK;
}

} // namespace board_183_sq
