package dev.yoa.bvox.ui.screens.discover

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.core.VideoItem
import dev.yoa.bvox.ui.components.CircleIconButton
import dev.yoa.bvox.ui.components.EmptyState
import dev.yoa.bvox.ui.components.PillButton
import dev.yoa.bvox.ui.components.ScreenHeader
import dev.yoa.bvox.ui.components.StatusLine
import dev.yoa.bvox.ui.components.TrackRow
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxDimens
import dev.yoa.bvox.ui.theme.BvoxRadius

/** Search + results. The core uses one queue list, so results are the queue. */
@Composable
fun DiscoverScreen(
    state: Snapshot,
    onSearch: (String) -> Unit,
    onLoadMore: () -> Unit,
    onImportBv: () -> Unit,
    onPlay: (Int) -> Unit,
    onAddToPlaylist: (VideoItem) -> Unit,
) {
    var query by rememberSaveable { mutableStateOf("") }
    val focusManager = LocalFocusManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    val showResults = state.currentPlaylist == null && state.results.isNotEmpty()

    fun submit() {
        val keyword = query.trim()
        if (keyword.isEmpty()) return
        keyboard?.hide()
        focusManager.clearFocus()
        onSearch(keyword)
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            ScreenHeader(
                kicker = "DISCOVER",
                title = "发现音乐",
                subtitle = "搜索 B 站音乐、UP 主或 BV 号",
            )
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .height(48.dp)
                        .clip(RoundedCornerShape(BvoxRadius.Pill))
                        .background(BvoxColors.Card)
                        .border(
                            BvoxDimens.Hairline,
                            BvoxColors.Hairline,
                            RoundedCornerShape(BvoxRadius.Pill),
                        )
                        .padding(horizontal = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Search,
                        contentDescription = null,
                        tint = BvoxColors.TextTertiary,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(modifier = Modifier.width(10.dp))
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyLarge.copy(color = BvoxColors.TextPrimary),
                        cursorBrush = SolidColor(BvoxColors.BrandPrimary),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        keyboardActions = KeyboardActions(onSearch = { submit() }),
                        decorationBox = { innerTextField ->
                            Box(contentAlignment = Alignment.CenterStart) {
                                if (query.isEmpty()) {
                                    Text(
                                        text = "搜索歌曲 / UP 主 / BV 号",
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = BvoxColors.TextTertiary,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                                innerTextField()
                            }
                        },
                    )
                    if (query.isNotEmpty()) {
                        Box(
                            modifier = Modifier
                                .size(30.dp)
                                .clip(CircleShape)
                                .clickable { query = "" },
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.Clear,
                                contentDescription = "清空",
                                tint = BvoxColors.TextTertiary,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
                CircleIconButton(
                    icon = Icons.Rounded.Add,
                    contentDescription = "导入 BV 号/链接",
                    onClick = onImportBv,
                    size = 48.dp,
                    tint = BvoxColors.BrandPrimary,
                )
            }
        }

        item {
            StatusLine(status = state.status, loading = state.loading, error = state.error)
        }

        if (showResults) {
            item {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "搜索结果",
                            style = MaterialTheme.typography.titleMedium,
                            color = BvoxColors.TextPrimary,
                        )
                        Text(
                            text = when {
                                state.hasMoreResults && state.searchTotal > 0uL ->
                                    "已加载 ${state.results.size} / ${state.searchTotal} 首 · 上滑加载更多"
                                state.hasMoreResults ->
                                    "已加载 ${state.results.size} 首 · 上滑加载更多"
                                else -> "${state.results.size} 首 · 点击即播放"
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = BvoxColors.TextTertiary,
                        )
                    }
                    PillButton(text = "播放全部", onClick = { onPlay(0) })
                }
            }
            itemsIndexed(
                items = state.results,
                key = { index, item -> "${item.bvid}-$index" },
            ) { index, item ->
                TrackRow(
                    number = "${index + 1}",
                    title = item.title,
                    subtitle = "${item.author} · ${item.duration}",
                    current = state.current?.toInt() == index,
                    cached = state.cachedBvids.contains(item.bvid),
                    artworkUrl = item.pic,
                    onClick = { onPlay(index) },
                    trailing = {
                        CircleIconButton(
                            icon = Icons.Rounded.Add,
                            contentDescription = "加入歌单",
                            onClick = { onAddToPlaylist(item) },
                            size = 32.dp,
                            tint = BvoxColors.TextTertiary,
                        )
                    },
                )
            }
            if (state.hasMoreResults) {
                item(key = "load-more") {
                    // Composed only when the footer scrolls into view, which
                    // makes this a lazy "load next page" trigger.
                    LaunchedEffect(state.results.size) { onLoadMore() }
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 14.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (state.loadingMore) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(22.dp),
                                color = BvoxColors.BrandPrimary,
                                strokeWidth = 2.dp,
                            )
                        } else {
                            // Visible after a failed fetch too, so tapping it
                            // retries without having to scroll away and back.
                            PillButton(text = "加载更多", onClick = onLoadMore)
                        }
                    }
                }
            }
        } else {
            item {
                EmptyState(
                    icon = Icons.Rounded.Search,
                    title = "找点好听的",
                    message = "输入关键词后回车，或点右侧按钮导入 BV 号 / 链接",
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 40.dp),
                )
            }
        }
    }
}
