package dev.yoa.bvox.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxDimens

@Composable
internal fun StatusLine(
    status: String,
    loading: Boolean,
    error: Boolean,
    modifier: Modifier = Modifier,
) {
    // The core keeps its last status forever; these confirmations duplicate
    // what the screen already shows (folder count / opened folder title) or
    // belong to another tab (search result counts).
    val idle = status.matches(Regex("^歌单 \\d+ 个$")) ||
        status.startsWith("已打开") ||
        status.startsWith("找到 ") ||
        status == "没有搜索到结果"
    val text = if (idle) "" else status
    AnimatedVisibility(
        visible = loading || text.isNotBlank(),
        enter = fadeIn(),
        exit = fadeOut(),
        modifier = modifier,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (loading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(12.dp),
                    strokeWidth = 1.5.dp,
                    color = BvoxColors.BrandPrimary,
                )
            }
            Text(
                text = text,
                style = MaterialTheme.typography.bodySmall,
                color = if (error) BvoxColors.StateError else BvoxColors.TextTertiary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
internal fun EmptyState(
    icon: ImageVector,
    title: String,
    message: String,
    modifier: Modifier = Modifier,
    action: (@Composable () -> Unit)? = null,
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            modifier = Modifier
                .size(76.dp)
                .clip(CircleShape)
                .background(BvoxColors.Card)
                .border(BvoxDimens.Hairline, BvoxColors.Hairline, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = BvoxColors.BrandPrimary,
                modifier = Modifier.size(34.dp),
            )
        }
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = BvoxColors.TextPrimary,
            modifier = Modifier.padding(top = 18.dp),
        )
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = BvoxColors.TextTertiary,
            modifier = Modifier.padding(top = 6.dp, start = 24.dp, end = 24.dp),
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        if (action != null) {
            Spacer(modifier = Modifier.height(16.dp))
            action()
        }
    }
}
