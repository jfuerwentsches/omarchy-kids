#include <QApplication>

#include <string>

#include "main_window.h"

#include "omarchy-kids-control/poll_runner.h"

// Qt GUI frontend: multi-child dashboard, usage stats, app unlocks, tier
// changes. Theme-synced via ~/.local/state/omarchy/current/theme/colors.toml,
// same pattern as Omarchy's native apps (Omacalc, Omawrite, Omacut) — not
// wired up yet, see MainWindow's own scope note.
//
// `--poll` is the headless mode a systemd user timer invokes (see
// control/packaging/systemd/): checks every paired host's online status and
// writes it to StatusCache for the parent-computer Quickshell plugin to
// read. Deliberately dispatched before QApplication is constructed — the
// poll needs no display/event loop, and running it from a systemd timer
// with no graphical session attached must not depend on one being present.
int main(int argc, char *argv[]) {
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--poll") {
            omarchy_kids::control::runStatusPoll();
            return 0;
        }
    }

    QApplication app(argc, argv);

    MainWindow window;
    window.show();

    return app.exec();
}
