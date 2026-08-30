#pragma once

namespace omarchy_kids::control {

// Headless status poll: checks every host in the local HostRegistry over
// SSH (`omarchy-kids-agent status --json`, see AgentClient) and writes the
// result to StatusCache. This is the whole body of `omarchy-kids-control
// --poll` (see gui/src/main.cpp) — kept in core, not gui, so it needs no Qt
// event loop/display and so the TUI frontend could drive the same poll
// later without duplicating it.
void runStatusPoll();

} // namespace omarchy_kids::control
