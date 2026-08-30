#include "omarchy-kids-control/poll_runner.h"

#include "omarchy-kids-control/agent_client.h"
#include "omarchy-kids-control/host_registry.h"
#include "omarchy-kids-control/status_cache.h"

#include <ctime>
#include <string>
#include <utility>

namespace omarchy_kids::control {

namespace {

std::string nowIso8601Utc() {
    const std::time_t now = std::time(nullptr);
    std::tm utc{};
    gmtime_r(&now, &utc);
    char buf[32];
    std::strftime(buf, sizeof(buf), "%Y-%m-%dT%H:%M:%SZ", &utc);
    return buf;
}

} // namespace

void runStatusPoll() {
    HostRegistry registry;
    StatusCache cache;

    for (const HostEntry& host : registry.hosts()) {
        const auto result = AgentClient::run(host, {"status", "--json"});

        HostStatus status;
        status.name = host.name;
        status.online = result.ok;
        status.checkedAt = nowIso8601Utc();
        status.lastError = result.ok ? "" : "unreachable over SSH";
        cache.set(std::move(status));
    }

    cache.write();
}

} // namespace omarchy_kids::control
