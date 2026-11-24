#include "console.h"
#include <memory>
#include <mutex>
#include "driver/usb_serial_jtag.h"
#include "driver/usb_serial_jtag_vfs.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "line_buffer.h"
#include "slint.h"

static const char *TAG = "console";

namespace board_183_sq {
void console_deliver(const std::string &line);
namespace {

std::mutex g_mu;
std::shared_ptr<std::function<void(const std::string &)>> g_cb;

void reader_task(void *)
{
    LineBuffer lb;
    char buf[128];
    for (;;) {
        const int n = usb_serial_jtag_read_bytes(buf, sizeof buf, pdMS_TO_TICKS(50));
        if (n <= 0) continue;
        lb.feed(buf, (size_t)n, console_deliver);
    }
}

} // namespace

void console_deliver(const std::string &line)
{
    std::shared_ptr<std::function<void(const std::string &)>> cb;
    {
        std::lock_guard<std::mutex> g(g_mu);
        cb = g_cb;
    }
    if (!cb) return;
    slint::invoke_from_event_loop([cb, line] { (*cb)(line); });
}

void console_init()
{
    usb_serial_jtag_driver_config_t cfg = USB_SERIAL_JTAG_DRIVER_CONFIG_DEFAULT();
    cfg.rx_buffer_size = 1024;
    cfg.tx_buffer_size = 1024;
    ESP_ERROR_CHECK(usb_serial_jtag_driver_install(&cfg));
    usb_serial_jtag_vfs_use_driver(); // printf / logs passent par le pilote
    xTaskCreate(reader_task, "console_rx", 4096, nullptr, 3, nullptr);
    ESP_LOGI(TAG, "console USB prete");
}

void console_set_handler(std::function<void(const std::string &)> cb)
{
    std::lock_guard<std::mutex> g(g_mu);
    g_cb = cb ? std::make_shared<std::function<void(const std::string &)>>(std::move(cb)) : nullptr;
}

} // namespace board_183_sq
