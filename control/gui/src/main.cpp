#include <QApplication>

#include "main_window.h"

// Qt GUI frontend: multi-child dashboard, usage stats, app unlocks, tier
// changes. Theme-synced via ~/.local/state/omarchy/current/theme/colors.toml,
// same pattern as Omarchy's native apps (Omacalc, Omawrite, Omacut) — not
// wired up yet, see MainWindow's own scope note.
int main(int argc, char *argv[]) {
    QApplication app(argc, argv);

    MainWindow window;
    window.show();

    return app.exec();
}
