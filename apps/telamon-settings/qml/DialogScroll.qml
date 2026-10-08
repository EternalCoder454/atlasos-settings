import QtQuick
import org.kde.kirigami as Kirigami

// Put one inside a TelamonDialog whose body can be taller than the window.
// The dialog scrolls its body in a bare Flickable, which Qt moves 28 to 38
// pixels a wheel notch, unevenly; the pages scroll in a ScrollView of the
// desktop style, whose Kirigami.WheelHandler moves 60 pixels a notch, evenly,
// smooths a touchpad's pixel deltas and takes PageUp, PageDown, Home and End.
// This gives the dialog's Flickable the same handler. (Telamon.Ui's own
// TelamonDialog should do this; drop DialogScroll when it does.)
//
//   TelamonDialog {
//       DialogScroll {}
//       Repeater { ... }
//   }
Item {
    id: root

    // The Flickable that holds the dialog's body, found through the parents.
    readonly property Flickable flickable: {
        for (let p = root.parent; p; p = p.parent) {
            const f = p as Flickable;
            if (f)
                return f;
        }
        return null;
    }

    visible: false

    Kirigami.WheelHandler {
        target: root.flickable
    }
}
