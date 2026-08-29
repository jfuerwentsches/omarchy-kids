#include "omarchy-kids-control/agent_client.h"

#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include <chrono>
#include <thread>

namespace omarchy_kids::control {

namespace {

// fork/exec + pipe instead of popen()/system(): those go through a shell,
// and the values we're passing (hostname, key path) aren't trusted shell
// input (see the header comment on AgentClient::run).
AgentCommandResult runCaptured(const std::vector<std::string>& argv, std::chrono::seconds timeout) {
    AgentCommandResult result;

    int pipefd[2];
    if (pipe(pipefd) != 0) {
        return result;
    }

    pid_t pid = fork();
    if (pid < 0) {
        close(pipefd[0]);
        close(pipefd[1]);
        return result;
    }

    if (pid == 0) {
        // Child: stdout -> pipe, stderr -> /dev/null (ssh's own diagnostics
        // aren't part of the protocol and would otherwise corrupt the
        // captured JSON if anything ever leaked to stdout).
        close(pipefd[0]);
        dup2(pipefd[1], STDOUT_FILENO);
        int devnull = open("/dev/null", O_WRONLY);
        if (devnull >= 0) {
            dup2(devnull, STDERR_FILENO);
            close(devnull);
        }
        close(pipefd[1]);

        std::vector<char*> cargv;
        cargv.reserve(argv.size() + 1);
        for (const auto& a : argv) {
            cargv.push_back(const_cast<char*>(a.c_str()));
        }
        cargv.push_back(nullptr);
        execvp(cargv[0], cargv.data());
        _exit(127); // execvp failed (ssh not on PATH, etc.)
    }

    // Parent
    close(pipefd[1]);
    fcntl(pipefd[0], F_SETFL, O_NONBLOCK);

    // Fail-safe wall-clock timeout on top of `ssh -o ConnectTimeout=...`:
    // that flag only bounds the TCP/handshake phase, not a wedged ssh
    // process, and a hung poll must never freeze the dashboard.
    const auto deadline = std::chrono::steady_clock::now() + timeout;
    bool timedOut = false;
    int status = 0;
    for (;;) {
        pid_t r = waitpid(pid, &status, WNOHANG);
        if (r == pid) {
            break;
        }

        char buf[4096];
        ssize_t n;
        while ((n = read(pipefd[0], buf, sizeof(buf))) > 0) {
            result.stdoutText.append(buf, static_cast<size_t>(n));
        }

        if (std::chrono::steady_clock::now() >= deadline) {
            kill(pid, SIGKILL);
            waitpid(pid, &status, 0);
            timedOut = true;
            break;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    char buf[4096];
    ssize_t n;
    while ((n = read(pipefd[0], buf, sizeof(buf))) > 0) {
        result.stdoutText.append(buf, static_cast<size_t>(n));
    }
    close(pipefd[0]);

    if (!timedOut && WIFEXITED(status)) {
        result.exitCode = WEXITSTATUS(status);
        result.ok = (result.exitCode == 0);
    }
    return result;
}

} // namespace

AgentCommandResult AgentClient::run(
    const HostEntry& host,
    const std::vector<std::string>& agentArgs,
    std::chrono::seconds timeout) {
    std::vector<std::string> argv = {
        "ssh",
        "-i", host.keyPath,
        "-p", std::to_string(host.sshPort),
        "-o", "BatchMode=yes",
        "-o", "ConnectTimeout=5",
        "-o", "StrictHostKeyChecking=accept-new",
        // Must be user@host, not just host: ssh otherwise defaults to
        // whichever local account Control Center itself runs as, which
        // essentially never matches the child account the key lives under
        // (found via issue #29's real end-to-end test — pairing worked,
        // but nothing had ever actually driven a real SSH call this way
        // before, since `username` didn't even exist on HostEntry until
        // that same test surfaced the gap in the pairing protocol itself).
        host.username + "@" + host.hostname,
    };
    argv.insert(argv.end(), agentArgs.begin(), agentArgs.end());
    return runCaptured(argv, timeout);
}

} // namespace omarchy_kids::control
