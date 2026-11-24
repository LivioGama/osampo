// Bluetooth de la board (SP5d2) : peripherique NimBLE, un service, une
// caracteristique d'ecriture (LIGNES) et une de notification (ETAT). Le daemon
// ecrit sur LIGNES les memes lignes JSON que sur l'USB, en morceaux ; on les
// recolle et on les remet a la console. La board renvoie ses lignes d'etat et
// d'evenements au daemon par notifications de ETAT (SPEC-001).
#include <algorithm>
#include <atomic>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>

#include "board_183_sq.h"
#include "console.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "host/ble_hs.h"
#include "line_buffer.h"
#include "nimble/nimble_port.h"
#include "nimble/nimble_port_freertos.h"
#include "services/gap/ble_svc_gap.h"
#include "services/gatt/ble_svc_gatt.h"
#include "slint.h"

static const char *TAG = "ble";

namespace board_183_sq {
namespace {

// 7a6d0001-5c3e-4b8e-9d2a-74616d6f7368, 7a6d0002-... et 7a6d0003-... (octets en ordre inverse)
const ble_uuid128_t kService = BLE_UUID128_INIT(0x68, 0x73, 0x6f, 0x6d, 0x61, 0x74, 0x2a, 0x9d, 0x8e, 0x4b, 0x3e,
                                                0x5c, 0x01, 0x00, 0x6d, 0x7a);
const ble_uuid128_t kLignes = BLE_UUID128_INIT(0x68, 0x73, 0x6f, 0x6d, 0x61, 0x74, 0x2a, 0x9d, 0x8e, 0x4b, 0x3e,
                                               0x5c, 0x02, 0x00, 0x6d, 0x7a);
const ble_uuid128_t kEtat = BLE_UUID128_INIT(0x68, 0x73, 0x6f, 0x6d, 0x61, 0x74, 0x2a, 0x9d, 0x8e, 0x4b, 0x3e,
                                             0x5c, 0x03, 0x00, 0x6d, 0x7a);
constexpr size_t kLigneMax = 4096; // comme LIGNE_MAX du daemon (B4)
constexpr size_t kMorceau = 20;    // octets par notification (SPEC-001)
constexpr int kEssaisNotif = 20;   // reprises si la pile manque de tampons
constexpr int kAttenteNotifMs = 10;

uint8_t g_addr_type;
uint16_t g_etat_handle = 0;        // rempli par la pile a l'enregistrement du service
std::atomic<bool> g_abonne{false}; // le central est abonne aux notifications de ETAT
std::mutex g_mu_abonne;
std::shared_ptr<std::function<void()>> g_cb_abonne;
std::atomic<bool> g_connecte{false};
std::atomic<uint16_t> g_conn{BLE_HS_CONN_HANDLE_NONE};
// Ecrit par la tache NimBLE seulement (ecritures et deconnexions) : pas de verrou.
LineBuffer g_lignes(kLigneMax);
size_t g_rejetees = 0;

int on_write(uint16_t, uint16_t, ble_gatt_access_ctxt *ctxt, void *)
{
    if (ctxt->op != BLE_GATT_ACCESS_OP_WRITE_CHR) return BLE_ATT_ERR_UNLIKELY;
    char buf[512];
    uint16_t n = 0;
    if (ble_hs_mbuf_to_flat(ctxt->om, buf, sizeof buf, &n) != 0) return BLE_ATT_ERR_INVALID_ATTR_VALUE_LEN;
    g_lignes.feed(buf, n, console_deliver);
    if (g_lignes.rejected() != g_rejetees) {
        g_rejetees = g_lignes.rejected();
        ESP_LOGW(TAG, "ligne trop longue jetee (> %u octets)", (unsigned)kLigneMax);
    }
    return 0;
}

// Remplies par ble_start (zero partout ailleurs ; la derniere entree vide termine).
ble_gatt_chr_def kCaracteristiques[3];
ble_gatt_svc_def kServices[2];

// ETAT est en notification seule : aucune lecture ni ecriture.
int on_etat(uint16_t, uint16_t, ble_gatt_access_ctxt *, void *) { return BLE_ATT_ERR_READ_NOT_PERMITTED; }

int on_gap(ble_gap_event *ev, void *);

void prevenir_abonnement()
{
    std::shared_ptr<std::function<void()>> cb;
    {
        std::lock_guard<std::mutex> g(g_mu_abonne);
        cb = g_cb_abonne;
    }
    if (cb) slint::invoke_from_event_loop([cb] { (*cb)(); });
}

void annoncer()
{
    ble_hs_adv_fields f{};
    f.flags = BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP;
    const char *nom = ble_svc_gap_device_name();
    f.name = reinterpret_cast<const uint8_t *>(nom);
    f.name_len = std::strlen(nom);
    f.name_is_complete = 1;
    ble_gap_adv_set_fields(&f);
    ble_hs_adv_fields r{};
    r.uuids128 = &kService;
    r.num_uuids128 = 1;
    r.uuids128_is_complete = 1;
    ble_gap_adv_rsp_set_fields(&r);
    ble_gap_adv_params p{};
    p.conn_mode = BLE_GAP_CONN_MODE_UND;
    p.disc_mode = BLE_GAP_DISC_MODE_GEN;
    const int rc = ble_gap_adv_start(g_addr_type, nullptr, BLE_HS_FOREVER, &p, on_gap, nullptr);
    if (rc != 0 && rc != BLE_HS_EALREADY) ESP_LOGE(TAG, "annonce impossible (rc=%d)", rc);
}

int on_gap(ble_gap_event *ev, void *)
{
    switch (ev->type) {
    case BLE_GAP_EVENT_CONNECT:
        if (ev->connect.status == 0) {
            g_lignes.reset();
            g_abonne = false;
            g_connecte = true;
            g_conn = ev->connect.conn_handle;
            ESP_LOGI(TAG, "connecte");
        } else {
            annoncer();
        }
        break;
    case BLE_GAP_EVENT_DISCONNECT:
        g_lignes.reset(); // une ligne coupee ne se recolle pas (B4)
        g_connecte = false;
        g_abonne = false;
        g_conn = BLE_HS_CONN_HANDLE_NONE;
        ESP_LOGI(TAG, "deconnecte (raison %d)", ev->disconnect.reason);
        annoncer();
        break;
    case BLE_GAP_EVENT_SUBSCRIBE:
        if (ev->subscribe.attr_handle == g_etat_handle) {
            const bool abonne = ev->subscribe.cur_notify != 0;
            const bool etait = g_abonne.exchange(abonne);
            ESP_LOGI(TAG, "ETAT %s", abonne ? "abonne" : "desabonne");
            if (abonne && !etait) prevenir_abonnement();
        }
        break;
    case BLE_GAP_EVENT_MTU:
        ESP_LOGI(TAG, "mtu=%d", ev->mtu.value);
        break;
    case BLE_GAP_EVENT_ADV_COMPLETE:
        annoncer();
        break;
    default:
        break;
    }
    return 0;
}

void on_sync()
{
    ble_hs_id_infer_auto(0, &g_addr_type);
    annoncer();
}

void host_task(void *)
{
    nimble_port_run();
    nimble_port_freertos_deinit();
}

} // namespace

bool ble_start(const char *nom)
{
    // La NVS est deja initialisee par init() (storage).
    if (nimble_port_init() != ESP_OK) {
        ESP_LOGE(TAG, "pile Bluetooth indisponible, chiot sans Bluetooth");
        return false;
    }
    kCaracteristiques[0].uuid = &kLignes.u;
    kCaracteristiques[0].access_cb = on_write;
    kCaracteristiques[0].flags = BLE_GATT_CHR_F_WRITE | BLE_GATT_CHR_F_WRITE_NO_RSP;
    kCaracteristiques[1].uuid = &kEtat.u;
    kCaracteristiques[1].access_cb = on_etat;
    kCaracteristiques[1].val_handle = &g_etat_handle;
    kCaracteristiques[1].flags = BLE_GATT_CHR_F_NOTIFY;
    kServices[0].type = BLE_GATT_SVC_TYPE_PRIMARY;
    kServices[0].uuid = &kService.u;
    kServices[0].characteristics = kCaracteristiques;
    ble_svc_gap_init();
    ble_svc_gatt_init();
    if (ble_gatts_count_cfg(kServices) != 0 || ble_gatts_add_svcs(kServices) != 0) {
        ESP_LOGE(TAG, "service GATT refuse");
        return false;
    }
    ble_svc_gap_device_name_set(nom);
    ble_hs_cfg.sync_cb = on_sync;
    nimble_port_freertos_init(host_task);
    ESP_LOGI(TAG, "Bluetooth demarre, annonce \"%s\"", nom);
    return true;
}

bool ble_connected() { return g_connecte.load(); }

void on_ble_subscribed(std::function<void()> cb)
{
    std::lock_guard<std::mutex> g(g_mu_abonne);
    g_cb_abonne = cb ? std::make_shared<std::function<void()>>(std::move(cb)) : nullptr;
}

bool ble_send_line(const std::string &ligne)
{
    const uint16_t conn = g_conn.load();
    if (!g_abonne.load() || conn == BLE_HS_CONN_HANDLE_NONE) return false;
    std::string donnees = ligne;
    if (donnees.empty() || donnees.back() != '\n') donnees += '\n';
    for (size_t i = 0; i < donnees.size(); i += kMorceau) {
        const size_t n = std::min(kMorceau, donnees.size() - i);
        int rc = BLE_HS_ENOMEM;
        for (int essai = 0; essai < kEssaisNotif && rc == BLE_HS_ENOMEM; ++essai) {
            if (essai > 0) vTaskDelay(pdMS_TO_TICKS(kAttenteNotifMs)); // laisser la pile vider ses tampons
            os_mbuf *om = ble_hs_mbuf_from_flat(donnees.data() + i, n);
            if (!om) {
                rc = BLE_HS_ENOMEM;
                continue;
            }
            rc = ble_gatts_notify_custom(conn, g_etat_handle, om); // prend om dans tous les cas
        }
        if (rc != 0) {
            ESP_LOGW(TAG, "notification ETAT abandonnee (rc=%d)", rc);
            return false;
        }
    }
    return true;
}

void ble_stop()
{
    // Coupure annoncee au PC : il voit "liaison perdue" tout de suite, sans
    // attendre le delai de supervision.
    const uint16_t h = g_conn.load();
    if (h != BLE_HS_CONN_HANDLE_NONE) ble_gap_terminate(h, BLE_ERR_REM_USER_CONN_TERM);
}

} // namespace board_183_sq
