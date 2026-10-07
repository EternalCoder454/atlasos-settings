pragma ComponentBehavior: Bound

import QtQuick
import Telamon.Ui

// A person's round picture. Telamon.Ui's TelamonAvatar rounds a picture with
// a GPU mask, which this app's default CPU drawing (the software backend,
// cpp/main.cpp) cannot do: there the picture would be a square. So the
// picture is cropped to a circle on a Canvas, which draws the same on every
// backend; with no picture (or one that fails to load) TelamonAvatar's
// initials show. Drop this for TelamonAvatar when the framework rounds
// pictures on the software backend too (docs/DESIGN.md, "Accounts").
Item {
    id: root

    property url source
    property string name
    property real size: 32

    readonly property bool _picture: probe.status === Image.Ready

    implicitWidth: size
    implicitHeight: size
    Accessible.role: Accessible.Graphic
    Accessible.name: name.trim().length > 0 ? name : qsTr("Profile picture")

    TelamonAvatar {
        anchors.fill: parent
        size: root.size
        name: root.name
        visible: !root._picture
        Accessible.ignored: true
    }

    // Loads the picture, decoded at about the drawn size.
    Image {
        id: probe
        source: root.source
        visible: false
        asynchronous: true
        cache: true
        sourceSize: Qt.size(Math.ceil(root.size * 2), Math.ceil(root.size * 2))
        onStatusChanged: canvas.requestPaint()
    }

    Canvas {
        id: canvas
        anchors.fill: parent
        visible: root._picture
        renderTarget: Canvas.Image
        canvasSize: Qt.size(Math.ceil(root.size * Screen.devicePixelRatio), Math.ceil(root.size * Screen.devicePixelRatio))
        onCanvasSizeChanged: requestPaint()
        onAvailableChanged: requestPaint()
        // The canvas loads the file itself: it can't draw an Image item
        // that isn't shown, or on every backend.
        readonly property url file: root.source
        onFileChanged: if (file.toString() !== "")
            loadImage(file)
        Component.onCompleted: if (file.toString() !== "")
            loadImage(file)
        onImageLoaded: requestPaint()
        Accessible.ignored: true

        onPaint: {
            const ctx = getContext("2d");
            const w = canvas.canvasSize.width;
            const h = canvas.canvasSize.height;
            ctx.clearRect(0, 0, w, h);
            if (probe.status !== Image.Ready || probe.implicitWidth <= 0 || probe.implicitHeight <= 0 || !canvas.isImageLoaded(canvas.file))
                return;
            // The picture, cropped to fill the square.
            const scale = Math.max(w / probe.implicitWidth, h / probe.implicitHeight);
            const dw = probe.implicitWidth * scale;
            const dh = probe.implicitHeight * scale;
            // Only the circle is drawn into.
            ctx.save();
            ctx.beginPath();
            ctx.arc(w / 2, h / 2, Math.min(w, h) / 2, 0, 2 * Math.PI);
            ctx.closePath();
            ctx.clip();
            ctx.drawImage(canvas.file, (w - dw) / 2, (h - dh) / 2, dw, dh);
            ctx.restore();
        }
    }
}
