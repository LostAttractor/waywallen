module;
#include "QExtra/macro_qt.hpp"

#ifdef Q_MOC_RUN
#    include "waywallen/model/wallpaper_select_storage.moc"
#endif

export module waywallen:model.wallpaper_select_storage;
export import :msg.backend_msg;
import qextra;

namespace waywallen::model
{

export class WallpaperSelectStorage : public SelectStorage {
    Q_OBJECT
    QML_ELEMENT

    Q_PROPERTY(qint32 removableSelectedCount READ removableSelectedCount NOTIFY
                   selectedRemovableCountChanged FINAL)

public:
    WallpaperSelectStorage(QObject* parent = nullptr);
    ~WallpaperSelectStorage() override;

    Q_INVOKABLE QVariantList selectedWallpaperIds() const;
    Q_INVOKABLE QStringList  removableSelectedWallpaperIds() const;

    Q_SIGNAL void selectedRemovableCountChanged();

    auto removableSelectedCount() const -> qint32;
};

export class PlaylistItemSelectStorage : public WallpaperSelectStorage {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(qint64 playlistId READ playlistId NOTIFY playlistChanged FINAL)
    Q_PROPERTY(qint64 revision READ revision NOTIFY playlistChanged FINAL)
    Q_PROPERTY(QStringList initialEntryIds READ initialEntryIds NOTIFY playlistChanged FINAL)

public:
    PlaylistItemSelectStorage(QObject* parent = nullptr);
    ~PlaylistItemSelectStorage() override;

    auto playlistId() const -> qint64;
    auto revision() const -> qint64;
    auto initialEntryIds() const -> const QStringList&;

    Q_INVOKABLE void         beginPlaylistItems(qint64 playlistId, qint64 revision,
                                                const QStringList& entryIds);
    Q_INVOKABLE QVariantList orderedWallpaperIds() const;
    Q_INVOKABLE void         clear() override;

    Q_SIGNAL void playlistChanged();

protected:
    auto keepActiveWithoutSelection() const -> bool override;

private:
    void syncNewEntryOrder();

    qint64      m_playlist_id { 0 };
    qint64      m_revision { 0 };
    QStringList m_initial_entry_ids;
    QStringList m_new_entry_ids;
};

} // namespace waywallen::model
