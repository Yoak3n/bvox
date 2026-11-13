package dev.yoa.bilimusic.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.yoa.bilimusic.AppModel
import dev.yoa.bilimusic.CoreHolder
import dev.yoa.bilimusic.core.Snapshot

/** App root: main list + mini player + core-driven dialogs + player screen. */
@Composable
fun BiliMusicApp(model: AppModel) {
    val snapshot by model.snapshot.collectAsState()
    val state = snapshot

    var showPlayer by remember { mutableStateOf(false) }
    var showBvDialog by remember { mutableStateOf(false) }
    var showLogin by remember { mutableStateOf(false) }

    // Close the login panel as soon as the core reports an account.
    LaunchedEffect(state?.user) {
        if (state?.user != null) {
            showLogin = false
        }
    }

    val notLoggedIn = state?.user == null

    Box(modifier = Modifier.fillMaxSize()) {
        Scaffold { padding ->
            Column(modifier = Modifier.padding(padding)) {
                Header(state)
                Toolbar(
                    notLoggedIn = notLoggedIn,
                    onLogin = { showLogin = true },
                    onImportBv = { showBvDialog = true },
                    onFavorites = {
                        if (notLoggedIn) showLogin = true
                        else CoreHolder.core.openFavorites()
                    },
                    onSync = {
                        if (notLoggedIn) showLogin = true
                        else CoreHolder.core.syncPull()
                    },
                )
                ResultsList(state, modifier = Modifier.weight(1f))
            }
        }

        // Mini player above the bottom edge
        state?.takeIf { it.hasMedia }?.let { playing ->
            Surface(
                shadowElevation = 8.dp,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth(),
            ) {
                MiniPlayer(
                    state = playing,
                    onOpen = { showPlayer = true },
                )
            }
        }

        // Core-driven modals
        if (showLogin) {
            LoginPanel(state?.login) { showLogin = false }
        }
        state?.favorites?.let { modal -> FavoritesDialog(modal) }
        if (showBvDialog) {
            BvImportDialog(onDismiss = { showBvDialog = false })
        }

        // Full player screen overlay
        if (showPlayer) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.surface),
            ) {
                NowPlayingScreen(model) { showPlayer = false }
            }
        }
    }
}

@Composable
private fun Header(state: Snapshot?) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                "♪ Bili Music",
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.primary,
            )
            state?.user?.let { user ->
                Text(
                    "  @$user",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.tertiary,
                )
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (state?.loading == true) {
                CircularProgressIndicator(modifier = Modifier.size(14.dp), strokeWidth = 2.dp)
            }
            Text(
                state?.status ?: "正在初始化核心…",
                style = MaterialTheme.typography.bodySmall,
                color = if (state?.error == true) MaterialTheme.colorScheme.error
                else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun Toolbar(
    notLoggedIn: Boolean,
    onLogin: () -> Unit,
    onImportBv: () -> Unit,
    onFavorites: () -> Unit,
    onSync: () -> Unit,
) {
    var keyword by remember { mutableStateOf("") }
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = keyword,
                onValueChange = { keyword = it },
                modifier = Modifier.weight(1f),
                singleLine = true,
                label = { Text("搜索") },
                trailingIcon = {
                    IconButton(onClick = {
                        if (keyword.isNotBlank()) CoreHolder.core.search(keyword.trim())
                    }) {
                        Icon(Icons.Filled.Search, contentDescription = "搜索")
                    }
                },
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 8.dp)) {
            Button(onClick = {
                if (keyword.isNotBlank()) {
                    CoreHolder.core.search(keyword.trim())
                }
            }) {
                Text("搜索")
            }
            IconButton(onClick = onImportBv) {
                Icon(Icons.Filled.Add, contentDescription = "导入 BV 号/链接")
            }
            if (notLoggedIn) {
                IconButton(onClick = onLogin) {
                    Icon(Icons.Filled.Person, contentDescription = "登录", tint = MaterialTheme.colorScheme.primary)
                }
            }
            IconButton(onClick = onFavorites) {
                Icon(Icons.Filled.FavoriteBorder, contentDescription = "收藏夹导入")
            }
            IconButton(onClick = onSync) {
                Icon(Icons.Filled.Sync, contentDescription = "云同步载入")
            }
        }
    }
}

@Composable
private fun ResultsList(state: Snapshot?, modifier: Modifier = Modifier) {
    if (state == null) {
        Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        return
    }
    LazyColumn(modifier = modifier.fillMaxWidth(), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 96.dp)) {
        itemsIndexed(state.results, key = { index, item -> "${item.bvid}-$index" }) { index, item ->
            val isCurrent = state.current?.toInt() == index
            val isSelected = state.selected?.toInt() == index
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(
                        if (isSelected) MaterialTheme.colorScheme.surfaceVariant
                        else MaterialTheme.colorScheme.surface
                    )
                    .clickable { CoreHolder.core.play(index.toUInt()) }
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        if (isCurrent) "▶" else "${index + 1}",
                        style = MaterialTheme.typography.labelLarge,
                        color = if (isCurrent) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            item.title,
                            style = MaterialTheme.typography.bodyLarge,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            color = if (isCurrent) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.onSurface,
                        )
                        Text(
                            "${item.author} · ${item.duration}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun MiniPlayer(state: Snapshot, onOpen: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 8.dp),
    ) {
        if (state.duration > 0) {
            LinearProgressIndicator(
                progress = { (state.position / state.duration).toFloat().coerceIn(0f, 1f) },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(3.dp),
            )
        }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    state.nowPlaying?.title ?: "",
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    state.nowPlaying?.author ?: "",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
            IconButton(onClick = { CoreHolder.core.togglePause() }) {
                Icon(
                    if (state.paused) Icons.Filled.PlayArrow else Icons.Filled.Pause,
                    contentDescription = if (state.paused) "播放" else "暂停",
                )
            }
            IconButton(onClick = { CoreHolder.core.nextTrack() }) {
                Icon(Icons.Filled.SkipNext, contentDescription = "下一首")
            }
        }
    }
}
