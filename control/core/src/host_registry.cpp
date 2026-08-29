#include "omarchy-kids-control/host_registry.h"

#include <toml++/toml.hpp>

#include <algorithm>
#include <cstdlib>
#include <fstream>

namespace omarchy_kids::control {

std::filesystem::path HostRegistry::defaultConfigPath() {
    std::filesystem::path configDir;
    if (const char* xdg = std::getenv("XDG_CONFIG_HOME"); xdg && *xdg) {
        configDir = xdg;
    } else if (const char* home = std::getenv("HOME"); home && *home) {
        configDir = std::filesystem::path(home) / ".config";
    } else {
        configDir = ".config"; // last resort, shouldn't happen in practice
    }
    return configDir / "omarchy-kids-control" / "hosts.toml";
}

HostRegistry::HostRegistry() : HostRegistry(defaultConfigPath()) {}

HostRegistry::HostRegistry(std::filesystem::path configPath)
    : configPath_(std::move(configPath)) {
    load();
}

void HostRegistry::load() {
    hosts_.clear();
    if (!std::filesystem::exists(configPath_)) {
        return; // first run — nothing paired yet, not an error
    }

    toml::table root;
    try {
        root = toml::parse_file(configPath_.string());
    } catch (const toml::parse_error&) {
        // Malformed file — treat as empty rather than crashing the app;
        // the next addHost() call will overwrite it with valid TOML.
        return;
    }

    const auto* array = root["hosts"].as_array();
    if (!array) {
        return;
    }
    for (auto&& elem : *array) {
        const auto* t = elem.as_table();
        if (!t) {
            continue;
        }
        HostEntry entry;
        entry.name = (*t)["name"].value_or("");
        entry.hostname = (*t)["hostname"].value_or("");
        entry.sshPort = (*t)["ssh_port"].value_or(22);
        entry.keyPath = (*t)["key_path"].value_or("");
        entry.fingerprint = (*t)["fingerprint"].value_or("");
        entry.pairedAt = (*t)["paired_at"].value_or("");
        hosts_.push_back(std::move(entry));
    }
}

void HostRegistry::save() const {
    std::filesystem::create_directories(configPath_.parent_path());

    toml::array hostsArray;
    for (const auto& h : hosts_) {
        toml::table t;
        t.insert("name", h.name);
        t.insert("hostname", h.hostname);
        t.insert("ssh_port", h.sshPort);
        t.insert("key_path", h.keyPath);
        t.insert("fingerprint", h.fingerprint);
        t.insert("paired_at", h.pairedAt);
        hostsArray.push_back(std::move(t));
    }

    toml::table root;
    root.insert("hosts", std::move(hostsArray));

    std::ofstream out(configPath_, std::ios::trunc);
    out << root;
}

void HostRegistry::addHost(const HostEntry& entry) {
    auto it = std::find_if(hosts_.begin(), hosts_.end(), [&](const HostEntry& h) {
        return h.name == entry.name;
    });
    if (it != hosts_.end()) {
        *it = entry; // re-pairing the same child updates in place
    } else {
        hosts_.push_back(entry);
    }
    save();
}

} // namespace omarchy_kids::control
