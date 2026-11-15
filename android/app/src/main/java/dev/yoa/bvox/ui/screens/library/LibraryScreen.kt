package dev.yoa.bvox.ui.screens.library

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.ImageBitmap

import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem

@Composable
internal fun LibraryScreen(
    state: Snapshot,
    cover: ImageBitmap?,
    onRefresh: () -> Unit,
    onCreate: () -> Unit,
    onLogin: () -> Unit,
    onOpen: (FavoriteFolder) -> Unit,
    onRename: (FavoriteFolder) -> Unit,
    onDelete: (FavoriteFolder) -> Unit,
    onToggleHidden: (FavoriteFolder, Boolean) -> Unit,
    onToggleShowHidden: () -> Unit,
    onBack: () -> Unit,
    onPlay: (Int) -> Unit,
    onAddToPlaylist: (VideoItem) -> Unit,
    onRemove: (FavoriteFolder, VideoItem) -> Unit,
    onOpenPlayer: () -> Unit,
) {
    val folder = state.currentPlaylist
    if (!state.inLibrary && folder != null) {
        PlaylistDetail(
            state = state,
            folder = folder,
            onBack = onBack,
            onPlay = onPlay,
            onAddToPlaylist = onAddToPlaylist,
            onRemove = onRemove,
        )
    } else {
        LibraryHome(
            state = state,
            cover = cover,
            onRefresh = onRefresh,
            onCreate = onCreate,
            onLogin = onLogin,
            onOpen = onOpen,
            onRename = onRename,
            onDelete = onDelete,
            onToggleHidden = onToggleHidden,
            onToggleShowHidden = onToggleShowHidden,
            onOpenPlayer = onOpenPlayer,
        )
    }
}
