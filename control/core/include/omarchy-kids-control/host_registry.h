#pragma once

#include <filesystem>
#include <string>
#include <vector>

namespace omarchy_kids::control {

// One paired child host — everything Control Center needs to reach and
// identify it again. See the vault note "Omarchy Kids - Implementierung
// Control Center", "Datenmodell (Entwurf)".
struct HostEntry {
    std::string name;        // parent-chosen label, e.g. the child's name
    std::string hostname;    // host/IP to SSH to
    int sshPort = 22;
    std::string username;    // child account to SSH as (chosen freely during
                              // Omarchy's own account setup, reported back by
                              // `serve`'s Confirm payload during pairing —
                              // without it there's no way to know whose
                              // authorized_keys the paired key lives in)
    std::string keyPath;     // private key path (public half is <keyPath>.pub)
    std::string fingerprint; // SHA256 fingerprint shown/confirmed at pairing time
    std::string pairedAt;    // ISO 8601 timestamp
};

// Manages the list of known child hosts, persisted as TOML at
// defaultConfigPath() (see the vault note's data-model sketch — this is the
// literal TOML it describes, via tomlplusplus). Loaded eagerly on
// construction; each mutation persists immediately — this list is small and
// touched rarely (on pairing), not worth batching writes for.
class HostRegistry {
public:
    HostRegistry();
    explicit HostRegistry(std::filesystem::path configPath);

    const std::vector<HostEntry>& hosts() const { return hosts_; }

    // Adds a newly paired host, or updates the existing entry with the same
    // name (re-pairing the same child — e.g. after a DHCP lease changed its
    // address) rather than creating a duplicate.
    void addHost(const HostEntry& entry);

    static std::filesystem::path defaultConfigPath();

private:
    void load();
    void save() const;

    std::filesystem::path configPath_;
    std::vector<HostEntry> hosts_;
};

} // namespace omarchy_kids::control
