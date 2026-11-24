#pragma once
#include "driver/i2c_master.h"

namespace board_183_sq {
// Bus I2C unique (SDA 15, SCL 14, 400 kHz), cree au premier appel.
// Partage par le tactile, puis le codec et la RTC. Usage interne.
i2c_master_bus_handle_t i2c_bus();
} // namespace board_183_sq
