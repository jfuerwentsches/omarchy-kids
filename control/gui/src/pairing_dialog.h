#pragma once

#include <QByteArray>
#include <QDialog>
#include <QProcess>
#include <QString>

class QLineEdit;
class QSpinBox;
class QPushButton;
class QPlainTextEdit;

namespace omarchy_kids::control {
class HostRegistry;
} // namespace omarchy_kids::control

// Drives `omarchy-kids-pairing pair` as a subprocess rather than
// reimplementing the SPAKE2 protocol in C++ (see the vault note's decision,
// 2026-08-29: reuse the already-verified Rust implementation — same
// "shell out to a trusted binary" pattern this project already uses for
// `ssh`). Shows the fingerprint the subprocess reports and only confirms
// once the parent has actually clicked through it; the CLI's own doc
// comment used to call its auto-confirm behavior a stand-in for exactly
// this dialog.
class PairingDialog : public QDialog {
    Q_OBJECT

public:
    explicit PairingDialog(omarchy_kids::control::HostRegistry& registry, QWidget* parent = nullptr);

private slots:
    void startPairing();
    void handleStdout();
    void handleFinished(int exitCode, QProcess::ExitStatus status);
    void handleError(QProcess::ProcessError error);

private:
    void appendLog(const QString& text);
    void fail(const QString& message);
    QString computeKeyPath(const QString& childName) const;

    omarchy_kids::control::HostRegistry& registry_;

    QLineEdit* nameEdit_ = nullptr;
    QLineEdit* hostEdit_ = nullptr;
    QSpinBox* portEdit_ = nullptr;
    QLineEdit* sidEdit_ = nullptr;
    QLineEdit* codeEdit_ = nullptr;
    QPushButton* pairButton_ = nullptr;
    QPlainTextEdit* log_ = nullptr;

    QProcess* process_ = nullptr;
    QByteArray stdoutBuffer_;
    QString keyPath_;
    bool resultReceived_ = false;
    bool errorHandled_ = false;
};
