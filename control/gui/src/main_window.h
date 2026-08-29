#pragma once

#include <QWidget>

#include <map>
#include <string>

#include "omarchy-kids-control/host_registry.h"

class QListWidget;
class QListWidgetItem;
class QLabel;
class QPushButton;
class QTimer;

// Minimal-but-real dashboard: a list of paired children, a status/security-
// events panel for whichever one is selected, and a way to pair a new one
// (see PairingDialog). Polls the selected host's `omarchy-kids-agent
// status`/`report` over SSH (see AgentClient) on a timer; severe security
// events get a local `notify-send` (issue #10's "active notification on the
// parent's computer" — there was nothing on this end to deliver that to
// before this existed). Not yet done: app-unlock/tier-switch controls and
// usage-stat charts (see the vault note "Omarchy Kids - Implementierung
// Control Center") — this first slice is read-only.
class MainWindow : public QWidget {
    Q_OBJECT

public:
    MainWindow(QWidget* parent = nullptr);

private slots:
    void openPairingDialog();
    void pollSelectedHost();

private:
    void refreshHostList();
    // Runs the two SSH round trips on a worker thread (AgentClient::run
    // blocks for up to its timeout) and marshals the result back to this
    // object's thread via a queued invocation — a poll must never freeze
    // the window.
    void startPoll(const omarchy_kids::control::HostEntry& host);
    void applyPollResult(
        const std::string& hostName,
        bool statusOk, const QString& statusText,
        bool reportOk, const QString& reportJson);
    void notifySevereEvents(const std::string& hostName, const QString& reportJson);

    omarchy_kids::control::HostRegistry registry_;
    QListWidget* hostList_ = nullptr;
    QLabel* statusLabel_ = nullptr;
    QListWidget* eventsList_ = nullptr;
    QPushButton* refreshButton_ = nullptr;
    QTimer* pollTimer_ = nullptr;

    // Dedupes notifications: the newest `occurred_at` already surfaced for
    // each host, so re-polling the same still-current severe event doesn't
    // re-notify every tick.
    std::map<std::string, std::string> lastNotifiedAt_;
    int pollGeneration_ = 0;
};
