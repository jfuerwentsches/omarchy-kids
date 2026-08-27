#pragma once

namespace omarchy_kids::control {

// Manages the list of known child hosts (name, host/IP, SSH key reference,
// last known status), the SSH subprocess wrapper, and the status cache read
// by the Quickshell plugin. See docs for the full design.
class HostRegistry {
public:
    HostRegistry() = default;
};

} // namespace omarchy_kids::control
