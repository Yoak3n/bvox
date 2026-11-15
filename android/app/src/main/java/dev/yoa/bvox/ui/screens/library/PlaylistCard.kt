package dev.yoa.bvox.ui.screens.library

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.FavoriteFolder
import dev.yoa.bvox.ui.components.ArtworkTile
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.theme.BvoxColors

@Composable
internal fun PlaylistCard(
    folder: FavoriteFolder,
    hidden: Boolean,
    onOpen: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    onToggleHidden: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    GlassCard(
        onClick = onOpen,
        contentPadding = PaddingValues(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            ArtworkTile(size = 52.dp, icon = Icons.AutoMirrored.Rounded.QueueMusic)
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 12.dp),
            ) {
                Text(
                    text = folder.title,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (hidden) BvoxColors.TextTertiary else BvoxColors.TextPrimary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = buildString {
                        append("${folder.count} 首")
                        if (hidden) append(" · 已隐藏")
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = BvoxColors.TextTertiary,
                )
            }
            Box {
                IconButton(onClick = { menuOpen = true }) {
                    Icon(
                        imageVector = Icons.Rounded.MoreVert,
                        contentDescription = "歌单操作",
                        tint = BvoxColors.TextTertiary,
                    )
                }
                DropdownMenu(
                    expanded = menuOpen,
                    onDismissRequest = { menuOpen = false },
                ) {
                    DropdownMenuItem(
                        text = { Text(if (hidden) "取消隐藏" else "隐藏") },
                        onClick = {
                            menuOpen = false
                            onToggleHidden()
                        },
                    )
                    DropdownMenuItem(
                        text = { Text("重命名") },
                        onClick = {
                            menuOpen = false
                            onRename()
                        },
                    )
                    DropdownMenuItem(
                        text = { Text("删除歌单") },
                        onClick = {
                            menuOpen = false
                            onDelete()
                        },
                    )
                }
            }
        }
    }
}
