#pragma once

#include <filesystem>
#include <string>
#include <vector>

namespace omarchy_kids::control {

// One paired child's last-known reachability, as of the most recent poll.
struct HostStatus {
    std::string name;
    bool online = false;
    std::string checkedAt; // ISO 8601
    std::string lastError; // empty when online
};

// Local snapshot of every paired child's online status, written by the
// headless poll mode (`omarchy-kids-control --poll`, run periodically by a
// systemd user timer) and read by the parent-computer Quickshell plugin —
// see the vault note "Omarchy Kids - Implementierung Control Center",
// "Trust-Boundary-Entscheidung": the plugin never speaks SSH itself, only
// this cache. Deliberately hand-written JSON, not TOML like HostRegistry —
// the only other reader is QML, which parses JSON natively and has no TOML
// support.
//
// Unlike HostRegistry this holds no state across runs: each poll checks
// every currently-paired host and calls set() for each, so the file written
// by write() is always a full, fresh snapshot rather than an incremental
// update.
class StatusCache {
public:
    explicit StatusCache(std::filesystem::path path = defaultPath());

    void set(HostStatus status);
    void write() const;

    static std::filesystem::path defaultPath();

private:
    std::filesystem::path path_;
    std::vector<HostStatus> entries_;
};

} // namespace omarchy_kids::control
