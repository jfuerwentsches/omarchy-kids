#pragma once

#include <chrono>
#include <string>
#include <vector>

#include "omarchy-kids-control/host_registry.h"

namespace omarchy_kids::control {

// Result of one `ssh ... omarchy-kids-agent <args>` round trip. `ok` only
// reflects the SSH command's own exit code — it says nothing about whether
// agentd itself accepted the request (`stdoutText` is the raw `--json`
// response envelope; the caller parses `ok`/`data`/`error` out of that).
struct AgentCommandResult {
    bool ok = false;
    int exitCode = -1;
    std::string stdoutText;
};

// Runs `omarchy-kids-agent <args>` on a paired child host over the
// `command=`-restricted SSH channel installed at pairing time (see
// `agent/pairing/src/main.rs`'s `install_pubkey` and
// docs/agent-protocol.md's `effective_args()` note — the forced command
// only sees what we pass here via `$SSH_ORIGINAL_COMMAND`, so these args
// become the real remote command line).
//
// Deliberately execvp's `ssh` directly (no shell) instead of building a
// shell command string: `host.hostname`/`host.keyPath` can originate from
// LAN-discovered pairing data or manual entry, so this must never let their
// contents be interpreted as shell syntax.
//
// Host-key verification is pinned to the child's real sshd host key
// (`host.sshHostPublicKey`, captured through the SPAKE2-authenticated
// pairing exchange — issue #33) rather than plain `accept-new` TOFU, via a
// per-call known_hosts file (see `PinnedKnownHosts` in the .cpp). Falls
// back to TOFU only for a host paired before that field existed.
class AgentClient {
public:
    static AgentCommandResult run(
        const HostEntry& host,
        const std::vector<std::string>& agentArgs,
        std::chrono::seconds timeout = std::chrono::seconds(8));
};

} // namespace omarchy_kids::control
