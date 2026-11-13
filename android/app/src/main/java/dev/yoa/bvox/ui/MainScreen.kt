package dev.yoa.bvox.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.AppModel
import dev.yoa.bvox.CoreHolder
import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.ArtworkTile
import dev.yoa.bvox.ui.components.BvoxBottomBar
import dev.yoa.bvox.ui.components.BvoxNavItem
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.GradientBackground
import dev.yoa.bvox.ui.screens.DiscoverScreen
import dev.yoa.bvox.ui.screens.LibraryScreen
import dev.yoa.bvox.ui.screens.MeScreen
import dev.yoa.bvox.ui.screens.NowPlayingScreen
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius

private enum class RootTab(val label: String, val icon: ImageVector) {
    Library("歌单", Icons.Rounded.LibraryMusic),
    Discover("发现", Icons.Rounded.Search),
    Playing("播放", Icons.Rounded.GraphicEq),
    Me("我的", Icons.Rounded.Person),
}

/** App root: tabbed shell, mini player, core-driven dialogs. */
@Composable
fun BvoxApp(model: AppModel) {
    val snapshot by model.snapshot.collectAsState()
    val cover by model.cover.collectAsState()
    val state = snapshot
    val pendingImportTitle = state?.pendingImportTitle
    val pendingImportBvid = state?.pendingImportBvid
    val coverBitmap = cover?.takeIf { it.first == state?.coverKey }?.second

    var tab by rememberSaveable { mutableStateOf(RootTab.Library) }
    var showBvDialog by remember { mutableStateOf(false) }
    var showLogin by remember { mutableStateOf(false) }
    var showLogoutConfirm by remember { mutableStateOf(false) }

    var showCreatePlaylist by remember { mutableStateOf(false) }
    var renameTarget by remember { mutableStateOf<FavoriteFolder?>(null) }
    var deleteTarget by remember { mutableStateOf<FavoriteFolder?>(null) }
    var addTrackBvid by remember { mutableStateOf<String?>(null) }
    var removeTrack by remember { mutableStateOf<Pair<FavoriteFolder, String>?>(null) }

    // Close the login panel as soon as the core reports an account.
    LaunchedEffect(state?.user) {
        if (state?.user != null) {
            showLogin = false
        }
    }

    // A BV imported without an open playlist parks in the core; open the
    // add-to-playlist picker so the user chooses the destination folder.
    LaunchedEffect(pendingImportBvid) {
        if (pendingImportBvid != null) {
            addTrackBvid = pendingImportBvid
        }
    }

    // Inside a playlist, back returns to the folder list instead of exiting.
    val inPlaylistDetail = tab == RootTab.Library &&
        state != null &&
        !state.inLibrary &&
        state.currentPlaylist != null
    BackHandler(enabled = inPlaylistDetail) {
        CoreHolder.core.openLibrary()
    }

    val navItems = remember {
        RootTab.entries.map { BvoxNavItem(label = it.label, icon = it.icon) }
    }
    val pageOffset = with(LocalDensity.current) { 8.dp.roundToPx() }

    GradientBackground {
        Scaffold(
            containerColor = Color.Transparent,
            bottomBar = {
                Column {
                    if (state != null) {
                        AnimatedVisibility(
                            visible = state.hasMedia && tab != RootTab.Playing,
                            enter = fadeIn(tween(BvoxMotion.BottomNavMillis)) +
                                slideInVertically(tween(BvoxMotion.BottomNavMillis)) { it / 2 },
                            exit = fadeOut(tween(BvoxMotion.PageFadeOutMillis)) +
                                slideOutVertically(tween(BvoxMotion.PageFadeOutMillis)) { it / 2 },
                        ) {
                            MiniPlayer(
                                state = state,
                                cover = coverBitmap,
                                onOpen = { tab = RootTab.Playing },
                            )
                        }
                    }
                    BvoxBottomBar(
                        items = navItems,
                        selectedIndex = tab.ordinal,
                        onSelect = { index -> tab = RootTab.entries[index] },
                    )
                }
            },
        ) { padding ->
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
            ) {
                AnimatedContent(
                    targetState = tab,
                    transitionSpec = {
                        (fadeIn(
                            animationSpec = tween(
                                durationMillis = BvoxMotion.PageTransitionMillis,
                                easing = BvoxMotion.EaseOut,
                            )
                        ) + slideInVertically(
                            animationSpec = tween(
                                durationMillis = BvoxMotion.PageTransitionMillis,
                                easing = BvoxMotion.EaseOut,
                            ),
                            initialOffsetY = { pageOffset },
                        )).togetherWith(
                            fadeOut(animationSpec = tween(BvoxMotion.PageFadeOutMillis))
                        )
                    },
                    label = "root_tabs",
                ) { target ->
                    val current = state
                    if (current == null) {
                        Box(
                            modifier = Modifier.fillMaxSize(),
                            contentAlignment = Alignment.Center,
                        ) {
                            CircularProgressIndicator(color = BvoxColors.BrandPrimary)
                        }
                    } else {
                        when (target) {
                            RootTab.Library -> LibraryScreen(
                                state = current,
                                cover = coverBitmap,
                                onRefresh = { CoreHolder.core.refreshPlaylists() },
                                onCreate = {
                                    if (current.user == null) showLogin = true
                                    else showCreatePlaylist = true
                                },
                                onLogin = { showLogin = true },
                                onOpen = { folder -> CoreHolder.core.openPlaylist(folder.id) },
                                onRename = { folder -> renameTarget = folder },
                                onDelete = { folder -> deleteTarget = folder },
                                onToggleHidden = { folder, hidden ->
                                    CoreHolder.core.setPlaylistHidden(folder.id, hidden)
                                },
                                onToggleShowHidden = { CoreHolder.core.toggleShowHidden() },
                                onBack = { CoreHolder.core.openLibrary() },
                                onPlay = { index -> CoreHolder.core.play(index.toUInt()) },
                                onAddToPlaylist = { item ->
                                    if (current.playlists.isEmpty()) {
                                        CoreHolder.core.refreshPlaylists()
                                    }
                                    addTrackBvid = item.bvid
                                },
                                onRemove = { folder, item -> removeTrack = folder to item.bvid },
                                onOpenPlayer = { tab = RootTab.Playing },
                            )
                            RootTab.Discover -> DiscoverScreen(
                                state = current,
                                onSearch = { keyword -> CoreHolder.core.search(keyword) },
                                onLoadMore = { CoreHolder.core.loadMoreSearch() },
                                onImportBv = { showBvDialog = true },
                                onPlay = { index -> CoreHolder.core.play(index.toUInt()) },
                                onAddToPlaylist = { item ->
                                    if (current.playlists.isEmpty()) {
                                        CoreHolder.core.refreshPlaylists()
                                    }
                                    addTrackBvid = item.bvid
                                },
                            )
                            RootTab.Playing -> NowPlayingScreen(model)
                            RootTab.Me -> MeScreen(
                                state = current,
                                onLogin = { showLogin = true },
                                onLogout = { showLogoutConfirm = true },
                                onPlayCached = { bvid ->
                                    CoreHolder.core.playCached(bvid)
                                    tab = RootTab.Playing
                                },
                            )
                        }
                    }
                }
            }
        }

        if (showLogin) {
            LoginPanel(state?.login) { showLogin = false }
        }
        if (showLogoutConfirm) {
            AlertDialog(
                onDismissRequest = { showLogoutConfirm = false },
                title = { Text("退出登录") },
                text = { Text("确定要退出当前账号吗？退出后可登录其他账号。") },
                confirmButton = {
                    TextButton(onClick = {
                        CoreHolder.core.logout()
                        showLogoutConfirm = false
                    }) { Text("退出") }
                },
                dismissButton = {
                    TextButton(onClick = { showLogoutConfirm = false }) { Text("取消") }
                },
            )
        }
        if (showBvDialog) {
            BvImportDialog(onDismiss = { showBvDialog = false })
        }
        if (showCreatePlaylist) {
            PlaylistNameDialog(
                title = if (pendingImportTitle != null) "新建歌单并加入" else "新建歌单",
                initial = "",
                confirmLabel = "创建",
                hint = pendingImportTitle?.let { "「$it」会加入这个歌单" },
                onDismiss = {
                    if (pendingImportTitle != null) {
                        CoreHolder.core.cancelPendingImport()
                    }
                    showCreatePlaylist = false
                },
                onConfirm = { name ->
                    CoreHolder.core.createPlaylist(name)
                    showCreatePlaylist = false
                },
            )
        }
        renameTarget?.let { folder ->
            PlaylistNameDialog(
                title = "重命名歌单",
                initial = folder.title,
                confirmLabel = "保存",
                onDismiss = { renameTarget = null },
                onConfirm = { name ->
                    CoreHolder.core.renamePlaylist(folder.id, name)
                    renameTarget = null
                },
            )
        }
        deleteTarget?.let { folder ->
            AlertDialog(
                onDismissRequest = { deleteTarget = null },
                title = { Text("删除歌单") },
                text = { Text("确定删除「${folder.title}」吗？歌单里的视频不会被删除。") },
                confirmButton = {
                    TextButton(onClick = {
                        CoreHolder.core.deletePlaylist(folder.id)
                        deleteTarget = null
                    }) { Text("删除") }
                },
                dismissButton = {
                    TextButton(onClick = { deleteTarget = null }) { Text("取消") }
                },
            )
        }
        addTrackBvid?.let { bvid ->
            val item = state?.results?.firstOrNull { it.bvid == bvid }
            val hiddenIds = state?.hiddenPlaylistIds?.toSet().orEmpty()
            val folders = state?.playlists.orEmpty().filter {
                state?.showHidden == true || it.id !in hiddenIds
            }
            AddToPlaylistDialog(
                trackTitle = item?.title ?: bvid,
                folders = folders,
                onDismiss = {
                    if (pendingImportBvid == bvid) {
                        CoreHolder.core.cancelPendingImport()
                    }
                    addTrackBvid = null
                },
                onCreateNew = if (pendingImportBvid == bvid) {
                    {
                        addTrackBvid = null
                        showCreatePlaylist = true
                    }
                } else {
                    null
                },
                onPick = { folder ->
                    CoreHolder.core.addToPlaylist(folder.id, bvid)
                    addTrackBvid = null
                },
            )
        }
        removeTrack?.let { (folder, bvid) ->
            val item = state?.results?.firstOrNull { it.bvid == bvid }
            AlertDialog(
                onDismissRequest = { removeTrack = null },
                title = { Text("从歌单移除") },
                text = { Text("从「${folder.title}」移除「${item?.title ?: bvid}」？") },
                confirmButton = {
                    TextButton(onClick = {
                        CoreHolder.core.removeFromPlaylist(folder.id, bvid)
                        removeTrack = null
                    }) { Text("移除") }
                },
                dismissButton = {
                    TextButton(onClick = { removeTrack = null }) { Text("取消") }
                },
            )
        }
    }
}

@Composable
private fun MiniPlayer(
    state: Snapshot,
    cover: ImageBitmap?,
    onOpen: () -> Unit,
) {
    val item = state.nowPlaying ?: return
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, bottom = 6.dp),
    ) {
        GlassCard(
            onClick = onOpen,
            shape = RoundedCornerShape(20.dp),
            contentPadding = PaddingValues(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (cover != null) {
                    Image(
                        bitmap = cover,
                        contentDescription = "封面",
                        modifier = Modifier
                            .size(44.dp)
                            .clip(RoundedCornerShape(BvoxRadius.Sm)),
                        contentScale = ContentScale.Crop,
                    )
                } else {
                    ArtworkTile(size = 44.dp, shape = RoundedCornerShape(BvoxRadius.Sm))
                }
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(horizontal = 10.dp),
                ) {
                    Text(
                        text = item.title,
                        style = MaterialTheme.typography.titleSmall,
                        color = BvoxColors.TextPrimary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = item.author,
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                IconButton(onClick = { CoreHolder.core.togglePause() }) {
                    Icon(
                        imageVector = if (state.paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause,
                        contentDescription = if (state.paused) "播放" else "暂停",
                        tint = BvoxColors.TextPrimary,
                    )
                }
                IconButton(onClick = { CoreHolder.core.nextTrack() }) {
                    Icon(
                        imageVector = Icons.Rounded.SkipNext,
                        contentDescription = "下一首",
                        tint = BvoxColors.TextPrimary,
                    )
                }
            }
            if (state.duration > 0) {
                LinearProgressIndicator(
                    progress = { (state.position / state.duration).toFloat().coerceIn(0f, 1f) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 8.dp)
                        .height(2.dp)
                        .clip(RoundedCornerShape(1.dp)),
                    color = BvoxColors.BrandPrimary,
                    trackColor = BvoxColors.Hairline,
                )
            }
        }
    }
}
