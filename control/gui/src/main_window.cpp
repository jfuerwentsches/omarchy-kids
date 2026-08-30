#include "main_window.h"
#include "pairing_dialog.h"

#include "omarchy-kids-control/agent_client.h"

#include <algorithm>
#include <vector>

#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMetaObject>
#include <QProcess>
#include <QPushButton>
#include <QSpinBox>
#include <QString>
#include <QStringList>
#include <QThreadPool>
#include <QTimer>
#include <QVBoxLayout>

using omarchy_kids::control::AgentClient;
using omarchy_kids::control::HostEntry;

namespace {
// While a host is selected in the dashboard, re-check it this often —
// there's no push channel from agentd, only pull (see AgentClient).
constexpr int kPollIntervalMs = 30000;
} // namespace

MainWindow::MainWindow(QWidget* parent) : QWidget(parent) {
    setWindowTitle(tr("Omarchy Kids Control Center"));

    hostList_ = new QListWidget(this);
    connect(hostList_, &QListWidget::currentRowChanged, this, &MainWindow::pollSelectedHost);

    statusLabel_ = new QLabel(tr("Select a child to see their status."), this);
    statusLabel_->setWordWrap(true);

    eventsList_ = new QListWidget(this);

    refreshButton_ = new QPushButton(tr("Refresh"), this);
    connect(refreshButton_, &QPushButton::clicked, this, &MainWindow::pollSelectedHost);

    auto* pairButton = new QPushButton(tr("Pair a new child..."), this);
    connect(pairButton, &QPushButton::clicked, this, &MainWindow::openPairingDialog);

    unlockAppEdit_ = new QLineEdit(this);
    unlockAppEdit_->setPlaceholderText(tr("desktop id, e.g. org.kde.gcompris"));
    unlockMinutesSpin_ = new QSpinBox(this);
    unlockMinutesSpin_->setRange(1, 24 * 60);
    unlockMinutesSpin_->setValue(30);
    unlockMinutesSpin_->setSuffix(tr(" min"));
    unlockButton_ = new QPushButton(tr("Unlock"), this);
    connect(unlockButton_, &QPushButton::clicked, this, &MainWindow::unlockSelectedApp);

    auto* unlockLayout = new QHBoxLayout();
    unlockLayout->addWidget(new QLabel(tr("Unlock app:"), this));
    unlockLayout->addWidget(unlockAppEdit_, /*stretch=*/1);
    unlockLayout->addWidget(unlockMinutesSpin_);
    unlockLayout->addWidget(unlockButton_);

    auto* layout = new QVBoxLayout(this);
    layout->addWidget(hostList_);
    layout->addWidget(statusLabel_);
    layout->addLayout(unlockLayout);
    layout->addWidget(new QLabel(tr("Security events (this week):"), this));
    layout->addWidget(eventsList_);
    layout->addWidget(refreshButton_);
    layout->addWidget(pairButton);
    setLayout(layout);

    // Only the currently selected host is polled — there's no headless
    // polling mode yet (see root CLAUDE.md "Status"), so a severe event on
    // an unselected/unopened host won't notify until this host is selected
    // again. Tracked as a known limitation, not silently assumed away.
    pollTimer_ = new QTimer(this);
    connect(pollTimer_, &QTimer::timeout, this, &MainWindow::pollSelectedHost);
    pollTimer_->start(kPollIntervalMs);

    refreshHostList();
}

void MainWindow::refreshHostList() {
    hostList_->clear();
    for (const HostEntry& host : registry_.hosts()) {
        auto* item = new QListWidgetItem(
            QStringLiteral("%1 — %2:%3")
                .arg(QString::fromStdString(host.name), QString::fromStdString(host.hostname))
                .arg(host.sshPort));
        item->setData(Qt::UserRole, QString::fromStdString(host.name));
        hostList_->addItem(item);
    }
}

void MainWindow::openPairingDialog() {
    PairingDialog dialog(registry_, this);
    if (dialog.exec() == QDialog::Accepted) {
        refreshHostList();
    }
}

void MainWindow::pollSelectedHost() {
    const int row = hostList_->currentRow();
    const auto& hosts = registry_.hosts();
    if (row < 0 || row >= static_cast<int>(hosts.size())) {
        return;
    }
    startPoll(hosts[static_cast<size_t>(row)]);
}

void MainWindow::startPoll(const HostEntry& host) {
    statusLabel_->setText(tr("Checking %1 ...").arg(QString::fromStdString(host.name)));

    // AgentClient::run blocks (it's a synchronous fork/exec + wait, see
    // control/core/src/agent_client.cpp) for up to its timeout — running it
    // on the GUI thread would freeze the window for that long on every poll.
    // QMetaObject::invokeMethod's context-object overload is used to hop
    // back safely: if `this` is destroyed before the worker finishes, the
    // queued call is simply dropped instead of touching a dangling pointer.
    HostEntry hostCopy = host;
    QThreadPool::globalInstance()->start([this, hostCopy]() {
        const auto statusResult = AgentClient::run(hostCopy, {"status", "--json"});
        const auto reportResult = AgentClient::run(hostCopy, {"report", "--week", "--json"});

        const std::string hostName = hostCopy.name;
        const bool statusOk = statusResult.ok;
        const bool reportOk = reportResult.ok;
        const QString statusText = QString::fromStdString(statusResult.stdoutText);
        const QString reportText = QString::fromStdString(reportResult.stdoutText);

        QMetaObject::invokeMethod(
            this,
            [this, hostName, statusOk, statusText, reportOk, reportText]() {
                applyPollResult(hostName, statusOk, statusText, reportOk, reportText);
            },
            Qt::QueuedConnection);
    });
}

void MainWindow::applyPollResult(
    const std::string& hostName,
    bool statusOk, const QString& statusText,
    bool reportOk, const QString& reportJson) {
    // A poll started for a host that's no longer selected (the parent
    // clicked a different child while it was in flight) shouldn't overwrite
    // what's on screen now — but it still gets to update notification
    // bookkeeping below, since a severe event doesn't stop mattering just
    // because the window moved on.
    const QListWidgetItem* current = hostList_->currentItem();
    const bool isCurrentSelection =
        current && current->data(Qt::UserRole).toString().toStdString() == hostName;

    if (isCurrentSelection) {
        if (!statusOk) {
            statusLabel_->setText(tr("%1: unreachable over SSH.").arg(QString::fromStdString(hostName)));
        } else {
            const QJsonDocument doc = QJsonDocument::fromJson(statusText.toUtf8());
            const QJsonObject envelope = doc.object();
            if (!doc.isObject() || !envelope.value("ok").toBool()) {
                statusLabel_->setText(tr("%1: agentd error: %2")
                                           .arg(QString::fromStdString(hostName), envelope.value("error").toString()));
            } else {
                const QJsonObject data = envelope.value("data").toObject();
                QStringList unlocked;
                for (const QJsonValue& v : data.value("unlocked_apps").toArray()) {
                    unlocked << v.toString();
                }
                statusLabel_->setText(
                    tr("Tier: %1 | Unlocked: %2 | Budget: %3/%4 min | Blocked window: %5 | Active: %6")
                        .arg(data.value("tier").toString(),
                             unlocked.isEmpty() ? tr("(none)") : unlocked.join(", "))
                        .arg(data.value("daily_used_minutes").toInt())
                        .arg(data.value("daily_budget_minutes").toInt())
                        .arg(data.value("in_blocked_window").toBool() ? tr("yes") : tr("no"),
                             data.value("active_app").toString(tr("(none)"))));
            }
        }

        eventsList_->clear();
        if (reportOk) {
            const QJsonDocument doc = QJsonDocument::fromJson(reportJson.toUtf8());
            const QJsonObject envelope = doc.object();
            if (doc.isObject() && envelope.value("ok").toBool()) {
                const QJsonArray eventsArray = envelope.value("data").toObject().value("security_events").toArray();
                // Copied out of QJsonArray first: its iterators dereference to
                // QJsonValueRef, which std::sort can't swap directly.
                std::vector<QJsonObject> events;
                events.reserve(static_cast<size_t>(eventsArray.size()));
                for (const QJsonValue& v : eventsArray) {
                    events.push_back(v.toObject());
                }
                // Newest first — ISO 8601 timestamps sort correctly as plain
                // strings, so this needs no date parsing.
                std::sort(events.begin(), events.end(), [](const QJsonObject& a, const QJsonObject& b) {
                    return a.value("occurred_at").toString() > b.value("occurred_at").toString();
                });
                for (const QJsonObject& ev : events) {
                    const QString severity = ev.value("severity").toString();
                    auto* item = new QListWidgetItem(
                        tr("[%1] %2 (%3)%4")
                            .arg(ev.value("occurred_at").toString(), ev.value("event_type").toString(), severity,
                                 ev.value("detail").isNull() ? QString()
                                                              : QStringLiteral(" — ") + ev.value("detail").toString()));
                    if (severity == QStringLiteral("severe")) {
                        item->setForeground(Qt::red);
                    }
                    eventsList_->addItem(item);
                }
            }
        }
    }

    if (reportOk) {
        notifySevereEvents(hostName, reportJson);
    }
}

void MainWindow::unlockSelectedApp() {
    const int row = hostList_->currentRow();
    const auto& hosts = registry_.hosts();
    if (row < 0 || row >= static_cast<int>(hosts.size())) {
        return;
    }
    const QString app = unlockAppEdit_->text().trimmed();
    if (app.isEmpty()) {
        statusLabel_->setText(tr("Enter a desktop id to unlock (e.g. org.kde.gcompris)."));
        return;
    }

    HostEntry hostCopy = hosts[static_cast<size_t>(row)];
    const int minutes = unlockMinutesSpin_->value();
    unlockButton_->setEnabled(false);
    statusLabel_->setText(tr("Unlocking %1 on %2 ...").arg(app, QString::fromStdString(hostCopy.name)));

    QThreadPool::globalInstance()->start([this, hostCopy, app, minutes]() {
        const auto result = AgentClient::run(
            hostCopy,
            {"unlock", app.toStdString(), "--minutes", std::to_string(minutes), "--json"});

        const std::string hostName = hostCopy.name;
        const bool ok = result.ok;
        const QString resultJson = QString::fromStdString(result.stdoutText);

        QMetaObject::invokeMethod(
            this,
            [this, hostName, ok, resultJson]() { applyUnlockResult(hostName, ok, resultJson); },
            Qt::QueuedConnection);
    });
}

void MainWindow::applyUnlockResult(const std::string& hostName, bool ok, const QString& resultJson) {
    unlockButton_->setEnabled(true);

    const QListWidgetItem* current = hostList_->currentItem();
    const bool isCurrentSelection =
        current && current->data(Qt::UserRole).toString().toStdString() == hostName;
    if (!isCurrentSelection) {
        return;
    }

    if (!ok) {
        statusLabel_->setText(tr("%1: unlock failed — unreachable over SSH.").arg(QString::fromStdString(hostName)));
        return;
    }
    const QJsonDocument doc = QJsonDocument::fromJson(resultJson.toUtf8());
    const QJsonObject envelope = doc.object();
    if (!doc.isObject() || !envelope.value("ok").toBool()) {
        statusLabel_->setText(tr("%1: unlock rejected by agentd: %2")
                                   .arg(QString::fromStdString(hostName), envelope.value("error").toString()));
        return;
    }
    unlockAppEdit_->clear();
    // Re-poll immediately so the status line/unlocked-apps list reflects the
    // new unlock right away instead of waiting for the next timer tick.
    pollSelectedHost();
}

void MainWindow::notifySevereEvents(const std::string& hostName, const QString& reportJson) {
    const QJsonDocument doc = QJsonDocument::fromJson(reportJson.toUtf8());
    if (!doc.isObject()) {
        return;
    }
    const QJsonObject envelope = doc.object();
    if (!envelope.value("ok").toBool()) {
        return;
    }
    const QJsonArray events = envelope.value("data").toObject().value("security_events").toArray();

    // First poll of a host this session: just learn where "already seen"
    // starts, without notifying — otherwise every pre-existing severe event
    // in that child's history would fire a notification the moment Control
    // Center opens, which is noise, not a real-time alert.
    const auto it = lastNotifiedAt_.find(hostName);
    const bool firstPollThisSession = (it == lastNotifiedAt_.end());
    const std::string newestSeen = firstPollThisSession ? std::string() : it->second;
    std::string newestThisPoll = newestSeen;

    for (const QJsonValue& v : events) {
        const QJsonObject ev = v.toObject();
        if (ev.value("severity").toString() != QStringLiteral("severe")) {
            continue;
        }
        const std::string occurredAt = ev.value("occurred_at").toString().toStdString();
        if (occurredAt > newestThisPoll) {
            newestThisPoll = occurredAt;
        }
        if (firstPollThisSession || occurredAt <= newestSeen) {
            continue;
        }
        QProcess::startDetached(
            "notify-send",
            {tr("Omarchy Kids: %1").arg(QString::fromStdString(hostName)),
             tr("%1%2")
                 .arg(ev.value("event_type").toString(),
                      ev.value("detail").isNull() ? QString() : QStringLiteral(" — ") + ev.value("detail").toString())});
    }
    lastNotifiedAt_[hostName] = newestThisPoll;
}
