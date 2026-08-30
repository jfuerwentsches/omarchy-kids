import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Parent-computer headerbar widget: an icon showing whether paired
// children's computers are online, and a popup breaking that down per
// child. Reads a local cache this widget never writes itself — see the
// vault note "Omarchy Kids - Implementierung Control Center",
// "Trust-Boundary-Entscheidung". The cache is written by
// `omarchy-kids-control --poll`, meant to run periodically via a systemd
// user timer (control/packaging/systemd/omarchy-kids-control-poll.timer);
// this widget only reads it and never speaks SSH.
//
// Pairing itself (mDNS/QR discovery, SPAKE2 confirmation) lives entirely in
// the Control Center GUI's PairingDialog — this widget has no copy of that
// flow, it only launches the GUI (which offers "Pair a new child...").
Panel {
  id: root
  moduleName: "omarchy-kids.control"
  ipcTarget: "omarchy-kids.control"

  property string home: Quickshell.env("HOME")
  property var hosts: []

  readonly property int onlineCount: hosts.filter(function(h) { return h.online }).length
  readonly property bool anyOffline: hosts.length > 0 && onlineCount < hosts.length

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function openControlCenter() {
    Quickshell.execDetached(["omarchy-kids-control"])
  }

  FileView {
    path: root.home + "/.config/omarchy-kids-control/status-cache.json"
    watchChanges: true
    printErrors: false
    onLoaded: root.parseCache(text())
    onFileChanged: reload()
    onLoadFailed: root.hosts = []
  }

  function parseCache(content) {
    try {
      var parsed = JSON.parse(String(content || "[]"))
      root.hosts = Array.isArray(parsed) ? parsed : []
    } catch (e) {
      root.hosts = []
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "🧒" // child emoji — plain Unicode, no dependency on the bar's icon font having a matching glyph
    active: root.anyOffline
    onPressed: function(b) {
      if (b === Qt.MiddleButton) root.openControlCenter()
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(300))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(420))

    Item {
      id: keyCatcher
      anchors.fill: parent
      focus: true
      Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
          root.close()
          event.accepted = true
        }
      }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(10)

        PanelSectionHeader {
          text: qsTr("OMARCHY KIDS")
          foreground: root.barForeground
        }

        Text {
          visible: root.hosts.length === 0
          width: parent.width
          text: qsTr("No children paired yet.")
          color: Qt.darker(root.barForeground, 1.4)
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          wrapMode: Text.WordWrap
        }

        Column {
          visible: root.hosts.length > 0
          width: parent.width
          spacing: Style.space(6)

          Repeater {
            model: root.hosts

            Row {
              required property var modelData
              width: column.width
              spacing: Style.space(8)

              Rectangle {
                width: Style.space(9)
                height: Style.space(9)
                radius: width / 2
                anchors.verticalCenter: parent.verticalCenter
                color: modelData.online ? Color.accent : Color.urgent
              }

              Text {
                text: String(modelData.name || "")
                color: root.barForeground
                font.family: Style.font.family
                font.pixelSize: Style.font.body
              }

              Text {
                text: modelData.online ? qsTr("online") : qsTr("offline")
                color: Qt.darker(root.barForeground, 1.4)
                font.family: Style.font.family
                font.pixelSize: Style.font.bodySmall
              }
            }
          }
        }

        PanelSeparator {
          foreground: root.barForeground
        }

        Text {
          width: parent.width
          text: root.hosts.length === 0 ? qsTr("Open Control Center to pair a child →") : qsTr("Open Control Center →")
          color: root.barForeground
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          font.bold: true

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              root.close()
              root.openControlCenter()
            }
          }
        }
      }
    }
  }
}
