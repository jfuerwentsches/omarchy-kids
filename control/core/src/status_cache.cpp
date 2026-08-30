#include "omarchy-kids-control/status_cache.h"

#include <cstdlib>
#include <fstream>

namespace omarchy_kids::control {

namespace {

// Minimal JSON string escaping — host names/error text here are either
// parent-chosen labels or our own fixed strings, but escaping properly
// costs nothing and avoids ever emitting invalid JSON for the QML side to
// choke on.
std::string escapeJson(const std::string& raw) {
    std::string out;
    out.reserve(raw.size() + 2);
    for (char c : raw) {
        switch (c) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default:
                if (static_cast<unsigned char>(c) < 0x20) {
                    char buf[8];
                    std::snprintf(buf, sizeof(buf), "\\u%04x", c);
                    out += buf;
                } else {
                    out += c;
                }
        }
    }
    return out;
}

} // namespace

std::filesystem::path StatusCache::defaultPath() {
    std::filesystem::path configDir;
    if (const char* xdg = std::getenv("XDG_CONFIG_HOME"); xdg && *xdg) {
        configDir = xdg;
    } else if (const char* home = std::getenv("HOME"); home && *home) {
        configDir = std::filesystem::path(home) / ".config";
    } else {
        configDir = ".config"; // last resort, shouldn't happen in practice
    }
    return configDir / "omarchy-kids-control" / "status-cache.json";
}

StatusCache::StatusCache(std::filesystem::path path) : path_(std::move(path)) {}

void StatusCache::set(HostStatus status) {
    entries_.push_back(std::move(status));
}

void StatusCache::write() const {
    std::filesystem::create_directories(path_.parent_path());

    std::string json = "[\n";
    for (size_t i = 0; i < entries_.size(); ++i) {
        const HostStatus& e = entries_[i];
        json += "  {\"name\": \"" + escapeJson(e.name) + "\", ";
        json += std::string("\"online\": ") + (e.online ? "true" : "false") + ", ";
        json += "\"checkedAt\": \"" + escapeJson(e.checkedAt) + "\", ";
        json += "\"lastError\": \"" + escapeJson(e.lastError) + "\"}";
        if (i + 1 < entries_.size()) {
            json += ",";
        }
        json += "\n";
    }
    json += "]\n";

    // Write to a temp file and rename over the target: the Quickshell plugin
    // has this file open with watchChanges — a half-written file read mid-
    // write would otherwise risk a transient JSON parse failure on its side.
    const std::filesystem::path tmpPath = path_.string() + ".tmp";
    {
        std::ofstream out(tmpPath, std::ios::trunc);
        out << json;
    }
    std::filesystem::rename(tmpPath, path_);
}

} // namespace omarchy_kids::control
