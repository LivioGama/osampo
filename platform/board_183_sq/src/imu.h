#pragma once
#include <functional>

#include "board_183_sq.h"

namespace board_183_sq {
// QMI8658 (QMI8658_ADDR) sur le bus I2C partage. Usage interne ; l'API publique
// (imu_start, ImuSample) est dans board_183_sq.h.
bool imu_begin(std::function<void(const ImuSample &)> cb);
} // namespace board_183_sq
