#pragma once

#include <QWidget>

#include "omarchy-kids-control/host_registry.h"

class QListWidget;

// Minimal shell for now: a list of paired children plus a way to pair a new
// one (see PairingDialog). The real multi-child dashboard (usage stats, app
// unlocks, tier changes — see the vault note "Omarchy Kids - Implementierung
// Control Center") is deliberately out of scope until there's at least one
// real paired host to build it against.
class MainWindow : public QWidget {
    Q_OBJECT

public:
    MainWindow(QWidget* parent = nullptr);

private slots:
    void openPairingDialog();

private:
    void refreshHostList();

    omarchy_kids::control::HostRegistry registry_;
    QListWidget* hostList_ = nullptr;
};
