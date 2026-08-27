#include <QApplication>
#include <QLabel>

// Qt GUI frontend: multi-child dashboard, usage stats, app unlocks, tier
// changes. Theme-synced via ~/.local/state/omarchy/current/theme/colors.toml,
// same pattern as Omarchy's native apps (Omacalc, Omawrite, Omacut).
int main(int argc, char *argv[]) {
    QApplication app(argc, argv);

    QLabel label("omarchy-kids-control (placeholder)");
    label.show();

    return app.exec();
}
