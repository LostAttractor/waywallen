pragma ComponentBehavior: Bound
pragma ValueTypeBehavior: Assertable
import QtQuick
import Qcm.Material as MD
import waywallen.ui as W

Item {
    id: root

    required property var model
    required property bool expanded
    readonly property real horizontalInset: 12
    readonly property real cardRadius: MD.Token.shape.corner.medium
    readonly property bool pointerHovered: carousel.hovered
    signal openRequested(string wallpaperId)

    implicitHeight: visible ? carousel.implicitHeight : 0
    height: implicitHeight

    MD.Carousel {
        id: carousel
        hoverEnabled: true
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(root.width, itemExtent + root.horizontalInset * 2)
        model: root.model
        layout: MD.Enum.CarouselUncontained
        itemExtent: root.expanded
                    ? Math.min(176, Math.max(0, root.width - root.horizontalInset * 2))
                    : Math.max(0, root.width - root.horizontalInset * 2)
        itemSpacing: MD.Token.spacing.small
        contentPadding: model?.count > 1 ? 0 : root.horizontalInset
        contentPaddingVertical: 0
        minimumViewportHeight: root.expanded ? 120 : 72
        wheelNavigationEnabled: false
        clipContainer: !root.expanded
        showPageIndicator: root.expanded && model?.count > 1
        header: qsTr("Current wallpapers")
        onClicked: index => {
            const item = model.item(index);
            if (item?.wallpaperId)
                root.openRequested(item.wallpaperId);
        }

        delegate: MD.CarouselItem {
            id: card
            required property var model
            required property var wallpaper
            required property string wallpaperId
            required property string targetSummary

            accessibilityTitle: wallpaper?.name || wallpaperId
            showBackground: false

            W.ThumbnailImage {
                id: thumbnail
                anchors.fill: parent
                source: card.wallpaper?.preview ?? ""
                resource: card.wallpaper?.resource ?? ""
                wpType: card.wallpaper?.wpType ?? ""
                fillMode: Image.PreserveAspectCrop
                radius: root.cardRadius
            }

            Rectangle {
                visible: root.expanded
                anchors.left: thumbnail.left
                anchors.right: thumbnail.right
                anchors.bottom: thumbnail.bottom
                height: Math.min(thumbnail.height, textColumn.implicitHeight + 28)
                radius: root.cardRadius
                gradient: Gradient {
                    GradientStop { position: 0; color: "transparent" }
                    GradientStop { position: 1; color: MD.Util.transparent(MD.Token.color.scrim, 0.78) }
                }
            }

            Column {
                id: textColumn
                visible: root.expanded
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                anchors.bottomMargin: 9
                spacing: 1

                MD.Text {
                    width: parent.width
                    text: card.wallpaper?.name || card.wallpaperId
                    typescale: MD.Token.typescale.label_large
                    color: MD.Token.color.on_primary
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }

                MD.Text {
                    width: parent.width
                    visible: text.length > 0
                    text: card.targetSummary
                    typescale: MD.Token.typescale.label_small
                    color: MD.Util.transparent(MD.Token.color.on_primary, 0.82)
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }
            }
        }
    }
}
