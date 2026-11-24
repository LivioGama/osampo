#include "json_line.h"
#include <cJSON.h>

JsonLine::~JsonLine() { reset(); }

void JsonLine::reset()
{
    if (root_) {
        cJSON_Delete(root_);
        root_ = nullptr;
    }
}

bool JsonLine::parse(const char *data, size_t len)
{
    reset();
    root_ = cJSON_ParseWithLength(data, len);
    if (root_ && !cJSON_IsObject(root_)) {
        reset();
    }
    return root_ != nullptr;
}

bool JsonLine::has(const char *key) const
{
    return root_ && cJSON_GetObjectItem(root_, key) != nullptr;
}

float JsonLine::number(const char *key, float fallback) const
{
    if (!root_) {
        return fallback;
    }
    const cJSON *item = cJSON_GetObjectItem(root_, key);
    if (!cJSON_IsNumber(item)) {
        return fallback;
    }
    return static_cast<float>(item->valuedouble);
}

int JsonLine::integer(const char *key, int fallback) const
{
    if (!root_) {
        return fallback;
    }
    const cJSON *item = cJSON_GetObjectItem(root_, key);
    if (!cJSON_IsNumber(item)) {
        return fallback;
    }
    return item->valueint;
}

std::string JsonLine::text(const char *key, const std::string &fallback) const
{
    if (!root_) {
        return fallback;
    }
    const cJSON *item = cJSON_GetObjectItem(root_, key);
    if (!cJSON_IsString(item) || item->valuestring == nullptr) {
        return fallback;
    }
    return std::string(item->valuestring);
}
