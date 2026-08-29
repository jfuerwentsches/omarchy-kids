#include "main_window.h"
#include "pairing_dialog.h"

#include <QListWidget>
#include <QPushButton>
#include <QString>
#include <QVBoxLayout>

using omarchy_kids::control::HostEntry;

MainWindow::MainWindow(QWidget* parent) : QWidget(parent) {
    setWindowTitle(tr("Omarchy Kids Control Center"));

    hostList_ = new QListWidget(this);

    auto* pairButton = new QPushButton(tr("Pair a new child..."), this);
    connect(pairButton, &QPushButton::clicked, this, &MainWindow::openPairingDialog);

    auto* layout = new QVBoxLayout(this);
    layout->addWidget(hostList_);
    layout->addWidget(pairButton);
    setLayout(layout);

    refreshHostList();
}

void MainWindow::refreshHostList() {
    hostList_->clear();
    for (const HostEntry& host : registry_.hosts()) {
        hostList_->addItem(
            QStringLiteral("%1 — %2:%3")
                .arg(QString::fromStdString(host.name), QString::fromStdString(host.hostname))
                .arg(host.sshPort));
    }
}

void MainWindow::openPairingDialog() {
    PairingDialog dialog(registry_, this);
    if (dialog.exec() == QDialog::Accepted) {
        refreshHostList();
    }
}
