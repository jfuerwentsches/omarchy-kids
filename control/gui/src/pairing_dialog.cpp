#include "pairing_dialog.h"
#include "omarchy-kids-control/host_registry.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFormLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLineEdit>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSpinBox>
#include <QVBoxLayout>

using omarchy_kids::control::HostEntry;
using omarchy_kids::control::HostRegistry;

namespace {

QString sanitizeForFilename(const QString& name) {
    QString out;
    out.reserve(name.size());
    for (const QChar c : name) {
        if (c.isLetterOrNumber() || c == QLatin1Char('-') || c == QLatin1Char('_')) {
            out.append(c);
        } else {
            out.append(QLatin1Char('_'));
        }
    }
    return out.isEmpty() ? QStringLiteral("child") : out;
}

} // namespace

PairingDialog::PairingDialog(HostRegistry& registry, QWidget* parent)
    : QDialog(parent), registry_(registry) {
    setWindowTitle(tr("Pair a child computer"));

    nameEdit_ = new QLineEdit(this);
    hostEdit_ = new QLineEdit(this);
    portEdit_ = new QSpinBox(this);
    portEdit_->setRange(1, 65535);
    portEdit_->setValue(7420);
    sidEdit_ = new QLineEdit(this);
    codeEdit_ = new QLineEdit(this);

    auto* form = new QFormLayout;
    form->addRow(tr("Child's name:"), nameEdit_);
    form->addRow(tr("Host (shown on the child's screen):"), hostEdit_);
    form->addRow(tr("Port:"), portEdit_);
    form->addRow(tr("Session ID:"), sidEdit_);
    form->addRow(tr("Pairing code:"), codeEdit_);

    pairButton_ = new QPushButton(tr("Pair"), this);
    connect(pairButton_, &QPushButton::clicked, this, &PairingDialog::startPairing);

    log_ = new QPlainTextEdit(this);
    log_->setReadOnly(true);

    auto* layout = new QVBoxLayout(this);
    layout->addLayout(form);
    layout->addWidget(pairButton_);
    layout->addWidget(log_);
    setLayout(layout);
}

QString PairingDialog::computeKeyPath(const QString& childName) const {
    const QString dir = QDir::homePath() + "/.local/share/omarchy-kids-control/keys";
    QDir().mkpath(dir);
    return dir + "/" + sanitizeForFilename(childName);
}

void PairingDialog::appendLog(const QString& text) {
    log_->appendPlainText(text);
}

void PairingDialog::fail(const QString& message) {
    appendLog(tr("Error: %1").arg(message));
    QMessageBox::warning(this, tr("Pairing failed"), message);
    pairButton_->setEnabled(true);
}

void PairingDialog::startPairing() {
    const QString name = nameEdit_->text().trimmed();
    const QString host = hostEdit_->text().trimmed();
    const QString sid = sidEdit_->text().trimmed();
    const QString code = codeEdit_->text().trimmed();

    if (name.isEmpty() || host.isEmpty() || sid.isEmpty() || code.isEmpty()) {
        QMessageBox::warning(this, tr("Missing information"), tr("Please fill in all fields."));
        return;
    }

    // A previous attempt (e.g. one stuck connecting to an unreachable/
    // firewalled host) is otherwise left running in the background:
    // besides leaking the process, its eventual, delayed reply can land on
    // whatever the *next* attempt started — including a mismatched-session
    // error on a server that has moved on to a different pairing window.
    // Disconnect first so its late signals can't touch state a new attempt
    // now owns.
    if (process_) {
        process_->disconnect(this);
        if (process_->state() != QProcess::NotRunning) {
            process_->kill();
            process_->waitForFinished(1000);
        }
        process_->deleteLater();
        process_ = nullptr;
    }

    keyPath_ = computeKeyPath(name);
    // `pair` refuses to overwrite an existing key file (a sensible guard
    // against clobbering a real, already-paired key) — but that also means
    // a leftover from an earlier failed/aborted attempt for this same
    // child name (wrong code, network hiccup, declined fingerprint) would
    // permanently block every retry. Since a failed attempt never reaches
    // registry_.addHost(), nothing outside this dialog can be depending on
    // that half-written key yet, so it's safe to clear before trying again.
    QFile::remove(keyPath_);
    QFile::remove(keyPath_ + ".pub");

    resultReceived_ = false;
    stdoutBuffer_.clear();
    pairButton_->setEnabled(false);
    appendLog(tr("Connecting to %1:%2 ...").arg(host).arg(portEdit_->value()));

    // omarchy-kids-pairing isn't packaged yet — assumed to already be on
    // PATH, the same assumption the rest of this monorepo's dev workflow
    // already makes (see setup-wizard/README.md).
    process_ = new QProcess(this);
    process_->setProgram("omarchy-kids-pairing");
    process_->setArguments({
        "pair",
        "--host", host,
        "--port", QString::number(portEdit_->value()),
        "--sid", sid,
        "--code", code,
        "--key-out", keyPath_,
    });
    errorHandled_ = false;
    connect(process_, &QProcess::readyReadStandardOutput, this, &PairingDialog::handleStdout);
    connect(process_, &QProcess::finished, this, &PairingDialog::handleFinished);
    connect(process_, &QProcess::errorOccurred, this, &PairingDialog::handleError);
    process_->start();
}

void PairingDialog::handleStdout() {
    stdoutBuffer_.append(process_->readAllStandardOutput());

    int newlineIndex;
    while ((newlineIndex = stdoutBuffer_.indexOf('\n')) != -1) {
        const QByteArray lineBytes = stdoutBuffer_.left(newlineIndex);
        stdoutBuffer_.remove(0, newlineIndex + 1);
        const QString line = QString::fromUtf8(lineBytes);
        if (line.isEmpty()) {
            continue;
        }
        appendLog(line);

        if (line.startsWith(QStringLiteral("Key fingerprint: "))) {
            // Blocking here is deliberate: `pair` is paused at its own
            // stdin read at this point (see agent/pairing/src/main.rs), so
            // there is nothing else for this dialog to do until the parent
            // answers — a nested modal event loop is the simplest correct
            // way to express that.
            const QString fingerprint = line.mid(QStringLiteral("Key fingerprint: ").size());
            const auto answer = QMessageBox::question(
                this, tr("Confirm fingerprint"),
                tr("The child's screen should show this exact fingerprint:\n\n%1\n\nDoes it match?")
                    .arg(fingerprint),
                QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
            process_->write(answer == QMessageBox::Yes ? "y\n" : "n\n");
        } else if (line.startsWith(QStringLiteral("PAIR_RESULT: "))) {
            const QByteArray json = line.mid(QStringLiteral("PAIR_RESULT: ").size()).toUtf8();
            const QJsonDocument doc = QJsonDocument::fromJson(json);
            if (!doc.isObject()) {
                fail(tr("Could not parse the pairing result."));
                continue;
            }
            const QJsonObject obj = doc.object();

            HostEntry entry;
            entry.name = nameEdit_->text().trimmed().toStdString();
            entry.hostname = obj.value("hostname").toString().toStdString();
            entry.sshPort = obj.value("ssh_port").toInt(22);
            entry.username = obj.value("username").toString().toStdString();
            entry.keyPath = obj.value("key_path").toString().toStdString();
            entry.fingerprint = obj.value("fingerprint").toString().toStdString();
            entry.pairedAt = QDateTime::currentDateTimeUtc().toString(Qt::ISODate).toStdString();

            registry_.addHost(entry);
            resultReceived_ = true;
            appendLog(tr("Paired with %1 — saved.").arg(QString::fromStdString(entry.hostname)));
        }
    }
}

void PairingDialog::handleFinished(int exitCode, QProcess::ExitStatus status) {
    if (errorHandled_) {
        return; // handleError() already reported this (e.g. a crash)
    }

    if (status != QProcess::NormalExit || exitCode != 0) {
        const QString stderrText = QString::fromUtf8(process_->readAllStandardError());
        fail(stderrText.isEmpty() ? tr("omarchy-kids-pairing exited with code %1.").arg(exitCode)
                                   : stderrText);
        return;
    }

    if (resultReceived_) {
        accept();
    } else {
        fail(tr("Pairing process exited successfully but no result was received."));
    }
}

void PairingDialog::handleError(QProcess::ProcessError error) {
    if (errorHandled_) {
        return;
    }
    errorHandled_ = true;

    if (error == QProcess::FailedToStart) {
        fail(tr("Could not start omarchy-kids-pairing. Is it installed and on PATH?"));
    } else {
        fail(tr("omarchy-kids-pairing process error: %1").arg(process_->errorString()));
    }
}
