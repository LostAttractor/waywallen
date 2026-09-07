module;
#include "waywallen/model/wallpaper_select_storage.moc.h"

module waywallen;
import qextra;
import :model.wallpaper_select_storage;

namespace waywallen::model
{

WallpaperSelectStorage::WallpaperSelectStorage(QObject* parent): SelectStorage(parent) {
    connect(this,
            &SelectStorage::selectedCountChanged,
            this,
            &WallpaperSelectStorage::selectedRemovableCountChanged);
    connect(this,
            &SelectStorage::modelChanged,
            this,
            &WallpaperSelectStorage::selectedRemovableCountChanged);
}

WallpaperSelectStorage::~WallpaperSelectStorage() = default;

auto WallpaperSelectStorage::selectedWallpaperIds() const -> QVariantList {
    QVariantList out;
    const auto   keys = selectedKeys();
    out.reserve(keys.size());
    for (const auto& key : keys) out.append(key);
    return out;
}

auto WallpaperSelectStorage::removableSelectedWallpaperIds() const -> QStringList {
    QStringList out;
    const auto  items = selectedItems();
    out.reserve(items.size());
    for (const auto& item : items) {
        if (! item.canConvert<model::Wallpaper>()) continue;
        const auto wallpaper = item.value<model::Wallpaper>();
        const auto id        = wallpaper.id_proto();
        if (wallpaper.supportsItemRemove() && ! id.isEmpty()) out.append(id);
    }
    return out;
}

auto WallpaperSelectStorage::removableSelectedCount() const -> qint32 {
    return static_cast<qint32>(removableSelectedWallpaperIds().size());
}

PlaylistItemSelectStorage::PlaylistItemSelectStorage(QObject* parent)
    : WallpaperSelectStorage(parent) {
    connect(this,
            &SelectStorage::selectedCountChanged,
            this,
            &PlaylistItemSelectStorage::syncNewEntryOrder);
}

PlaylistItemSelectStorage::~PlaylistItemSelectStorage() = default;

auto PlaylistItemSelectStorage::playlistId() const -> qint64 { return m_playlist_id; }
auto PlaylistItemSelectStorage::revision() const -> qint64 { return m_revision; }
auto PlaylistItemSelectStorage::initialEntryIds() const -> const QStringList& {
    return m_initial_entry_ids;
}

void PlaylistItemSelectStorage::beginPlaylistItems(qint64 playlistId, qint64 revision,
                                                   const QStringList& entryIds) {
    SelectStorage::clear();
    m_playlist_id = playlistId;
    m_revision    = revision;
    m_initial_entry_ids.clear();
    m_new_entry_ids.clear();
    QSet<QString> seen;
    seen.reserve(entryIds.size());
    for (const auto& entry_id : entryIds) {
        if (entry_id.isEmpty() || seen.contains(entry_id)) continue;
        seen.insert(entry_id);
        m_initial_entry_ids.append(entry_id);
    }
    Q_EMIT playlistChanged();
    setSelectedKeys(m_initial_entry_ids);
    setSelectionMode(true);
    setAnchorIndex(-1);
    notifyActiveChanged();
}

auto PlaylistItemSelectStorage::orderedWallpaperIds() const -> QVariantList {
    const auto    selected_keys = selectedKeys();
    QSet<QString> selected;
    selected.reserve(selected_keys.size());
    for (const auto& entry_id : selected_keys) selected.insert(entry_id);

    QVariantList result;
    result.reserve(selected.size());
    QSet<QString> emitted;
    emitted.reserve(selected.size());
    for (const auto& entry_id : m_initial_entry_ids) {
        if (! selected.contains(entry_id) || emitted.contains(entry_id)) continue;
        result.append(entry_id);
        emitted.insert(entry_id);
    }
    for (const auto& entry_id : m_new_entry_ids) {
        if (entry_id.isEmpty() || emitted.contains(entry_id)) continue;
        if (! selected.contains(entry_id)) continue;
        result.append(entry_id);
        emitted.insert(entry_id);
    }
    for (const auto& entry_id : selected_keys) {
        if (entry_id.isEmpty() || emitted.contains(entry_id)) continue;
        result.append(entry_id);
        emitted.insert(entry_id);
    }
    return result;
}

void PlaylistItemSelectStorage::syncNewEntryOrder() {
    const auto    selected_keys = selectedKeys();
    QSet<QString> selected;
    selected.reserve(selected_keys.size());
    for (const auto& entry_id : selected_keys) selected.insert(entry_id);

    m_new_entry_ids.removeIf([&selected](const QString& entry_id) {
        return ! selected.contains(entry_id);
    });

    QSet<QString> known;
    known.reserve(m_initial_entry_ids.size() + m_new_entry_ids.size());
    for (const auto& entry_id : m_initial_entry_ids) known.insert(entry_id);
    for (const auto& entry_id : m_new_entry_ids) known.insert(entry_id);
    for (const auto& entry_id : selected_keys) {
        if (entry_id.isEmpty() || known.contains(entry_id)) continue;
        m_new_entry_ids.append(entry_id);
        known.insert(entry_id);
    }
}

void PlaylistItemSelectStorage::clear() {
    const bool had_playlist =
        m_playlist_id > 0 || m_revision > 0 || ! m_initial_entry_ids.isEmpty();
    m_playlist_id = 0;
    m_revision    = 0;
    m_initial_entry_ids.clear();
    m_new_entry_ids.clear();
    if (had_playlist) Q_EMIT playlistChanged();
    SelectStorage::clear();
}

auto PlaylistItemSelectStorage::keepActiveWithoutSelection() const -> bool {
    return m_playlist_id > 0;
}

} // namespace waywallen::model

#include "waywallen/model/wallpaper_select_storage.moc.cpp"
