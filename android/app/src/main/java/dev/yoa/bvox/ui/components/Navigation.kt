package dev.yoa.bvox.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxDimens
import dev.yoa.bvox.ui.theme.BvoxMotion
import dev.yoa.bvox.ui.theme.BvoxRadius

internal data class BvoxNavItem(
    val label: String,
    val icon: ImageVector,
)

/** Semi-transparent attached bar with a rounded animated selection pill. */
@Composable
internal fun BvoxBottomBar(
    items: List<BvoxNavItem>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(BvoxColors.BottomBar)
            .navigationBarsPadding(),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(BvoxDimens.Hairline)
                .background(BvoxColors.Hairline),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    horizontal = BvoxDimens.BottomNavBarPaddingH,
                    vertical = BvoxDimens.BottomNavBarPaddingV,
                ),
            horizontalArrangement = Arrangement.spacedBy(BvoxDimens.BottomNavItemGap),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            items.forEachIndexed { index, item ->
                BottomBarItem(
                    item = item,
                    selected = selectedIndex == index,
                    modifier = Modifier.weight(1f),
                    onClick = { onSelect(index) },
                )
            }
        }
    }
}

@Composable
private fun BottomBarItem(
    item: BvoxNavItem,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val backgroundColor by animateColorAsState(
        targetValue = if (selected) BvoxColors.BrandPrimary.copy(alpha = 0.14f) else Color.Transparent,
        animationSpec = tween(BvoxMotion.BottomNavMillis),
        label = "nav_bg",
    )
    val contentColor by animateColorAsState(
        targetValue = if (selected) BvoxColors.BrandPrimary else BvoxColors.TextSecondary,
        animationSpec = tween(BvoxMotion.BottomNavMillis),
        label = "nav_color",
    )
    val iconScale by animateFloatAsState(
        targetValue = if (selected) BvoxDimens.BottomNavIconSelectedScale else 1f,
        animationSpec = tween(BvoxMotion.BottomNavMillis),
        label = "nav_scale",
    )
    val shape = RoundedCornerShape(BvoxRadius.Md)

    Box(
        modifier = modifier
            .height(BvoxDimens.BottomNavItemHeight)
            .clip(shape)
            .background(backgroundColor)
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                onClick = onClick,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Icon(
                imageVector = item.icon,
                contentDescription = item.label,
                tint = contentColor,
                modifier = Modifier
                    .size(BvoxDimens.BottomNavIconSize)
                    .graphicsLayer {
                        scaleX = iconScale
                        scaleY = iconScale
                    },
            )
            Text(
                text = item.label,
                color = contentColor,
                style = MaterialTheme.typography.labelSmall,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                maxLines = 1,
            )
        }
    }
}
