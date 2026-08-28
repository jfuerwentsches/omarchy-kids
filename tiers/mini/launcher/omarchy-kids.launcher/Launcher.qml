import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import qs.Commons
import qs.Ui

// Fullscreen kiosk icon grid for the mini age tier (5-7). Replaces the normal
// Omarchy menu (SUPER + SPACE) for this tier — see
// tiers/mini/hypr/hyprland.lua. No search box, no text dependency: a
// pre-reader picks an app by its icon/color alone.
Item {
  id: root

  property var shell: null
  property var manifest: null

  property bool opened: false
  property int selectedIndex: 0
  property var tiles: []

  property color background: Color.menu.background
  property color foreground: Color.menu.text

  // Fixed pixel sizes on purpose: this is a dedicated fullscreen kiosk
  // layout, not shell chrome, so it deliberately doesn't ride the theme's
  // [spacing]/[font] density scale (Style.space()) — a tall base-size +
  // spacing scale, like this tier's own theme sets for readability
  // elsewhere, would otherwise blow these tiles up past the viewport.
  property int tileSize: 260
  property int tileSpacing: 40
  readonly property int cornerRadius: Style.cornerRadius

  function open(payloadJson) {
    root.opened = true
    root.selectedIndex = 0
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "omarchy-kids.launcher")
  }

  function toggle() {
    if (root.opened) root.dismiss()
    else root.open("{}")
  }

  // Pre-warning (issue #8): agentd calls this over Quickshell IPC
  // (`omarchy-shell omarchyKidsLauncher preWarn <app> <secondsLeft>`) when an
  // active session is within its tier's lead time of a budget/window
  // cutoff. Shown regardless of whether the launcher grid itself is open —
  // the child is presumably inside the app being warned about, not looking
  // at this overlay — so it lives in its own always-present PanelWindow
  // below, not gated on `opened`. Non-readers (5-7) get an acoustic cue
  // alongside the visual banner, per "Omarchy Kids - Parental Controls und
  // Bildschirmzeit".
  property bool warningVisible: false
  property int warningSecondsLeft: 0

  function preWarn(app, secondsLeft) {
    root.warningSecondsLeft = Number(secondsLeft) || 0
    root.warningVisible = true
    warningHideTimer.restart()
    Quickshell.execDetached(["canberra-gtk-play", "-i", "alarm-clock-elapsed"])
  }

  Timer {
    id: warningHideTimer
    interval: 4000
    onTriggered: root.warningVisible = false
  }

  IpcHandler {
    target: "omarchyKidsLauncher"
    function preWarn(app: string, secondsLeft: string): void {
      root.preWarn(app, secondsLeft)
    }
  }

  function loadTiles(raw) {
    try {
      root.tiles = JSON.parse(raw) || []
    } catch (e) {
      root.tiles = []
    }
    tileModel.clear()
    for (var i = 0; i < root.tiles.length; i++) tileModel.append(root.tiles[i])
  }

  function launchIndex(index) {
    if (index < 0 || index >= tileModel.count) return
    var row = tileModel.get(index)
    root.dismiss()
    // Every unlocked app runs through the wrapper instead of gtk-launch
    // directly, so agentd sees start/stop and can cut a session short on a
    // time-budget/window cutoff — see agent/wrapper and issue #5.
    Quickshell.execDetached(["uwsm-app", "--", "omarchy-kids-run", row.desktopId])
  }

  ListModel { id: tileModel }

  FileView {
    path: Quickshell.env("HOME") + "/.config/omarchy-kids/launcher-apps.json"
    watchChanges: true
    onLoaded: root.loadTiles(text())
    onFileChanged: root.loadTiles(text())
  }

  PanelWindow {
    id: panel
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: root.background
    WlrLayershell.namespace: "omarchy-kids-launcher"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    Item {
      id: keyCatcher
      anchors.fill: parent
      focus: true

      Keys.priority: Keys.BeforeItem
      Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
          root.dismiss()
          event.accepted = true
        } else if (event.key === Qt.Key_Left) {
          root.selectedIndex = Math.max(0, root.selectedIndex - 1)
          event.accepted = true
        } else if (event.key === Qt.Key_Right) {
          root.selectedIndex = Math.min(tileModel.count - 1, root.selectedIndex + 1)
          event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
          root.launchIndex(root.selectedIndex)
          event.accepted = true
        }
      }

      Row {
        anchors.centerIn: parent
        spacing: root.tileSpacing

        Repeater {
          model: tileModel

          delegate: Rectangle {
            id: tile
            required property int index
            required property string desktopId
            required property string label
            required property string icon
            required property string swatch

            readonly property bool hasCursor: index === root.selectedIndex

            width: root.tileSize
            height: root.tileSize
            radius: root.cornerRadius
            color: tile.swatch
            border.width: hasCursor ? 6 : 0
            border.color: root.foreground
            scale: hasCursor ? 1.04 : 1.0

            Behavior on scale { NumberAnimation { duration: 120 } }

            Column {
              anchors.centerIn: parent
              spacing: 20

              Image {
                anchors.horizontalCenter: parent.horizontalCenter
                source: Quickshell.iconPath(tile.icon, true)
                width: 130
                height: 130
                fillMode: Image.PreserveAspectFit
              }

              Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: tile.label
                color: root.foreground
                font.family: Style.font.menuFamily
                font.pixelSize: 22
                font.bold: true
              }
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onContainsMouseChanged: if (containsMouse) root.selectedIndex = tile.index
              onClicked: root.launchIndex(tile.index)
            }
          }
        }
      }
    }
  }

  PanelWindow {
    id: warningPanel
    visible: root.warningVisible
    anchors { top: true }
    margins.top: 40
    implicitWidth: 360
    implicitHeight: 100
    color: "transparent"
    WlrLayershell.namespace: "omarchy-kids-prewarning"
    WlrLayershell.layer: WlrLayer.Overlay
    exclusionMode: ExclusionMode.Ignore

    Rectangle {
      anchors.centerIn: parent
      width: 340
      height: 100
      radius: Style.cornerRadius
      color: "#F59E0B"
      border.width: 4
      border.color: "#FFFFFF"

      Row {
        anchors.centerIn: parent
        spacing: 20

        Text {
          text: "⏰"
          font.pixelSize: 48
        }

        Text {
          text: Math.max(0, Math.round(root.warningSecondsLeft / 60)) + " min"
          color: "#FFFFFF"
          font.family: Style.font.menuFamily
          font.pixelSize: 36
          font.bold: true
        }
      }
    }
  }
}
