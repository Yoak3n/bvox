package dev.yoa.bvox.ui.screens.me

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Logout
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

import dev.yoa.bvox.core.Snapshot
import dev.yoa.bvox.ui.components.GlassCard
import dev.yoa.bvox.ui.components.PillButton
import dev.yoa.bvox.ui.components.rememberRemoteImage
import dev.yoa.bvox.ui.theme.BvoxColors
import dev.yoa.bvox.ui.theme.BvoxRadius

/** Account card: avatar, login state, VIP badge and the login/logout action. */
@Composable
internal fun AccountSection(
    state: Snapshot,
    onLogin: () -> Unit,
    onLogout: () -> Unit,
) {
    val user = state.user
    val hasSession = user != null || state.hasCookie
    val avatar = rememberRemoteImage(state.userFace)

    GlassCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .border(
                        width = if (state.vip) 1.5.dp else 1.dp,
                        color = if (state.vip) {
                            BvoxColors.BrandPrimary
                        } else {
                            BvoxColors.BrandPrimary.copy(alpha = 0.45f)
                        },
                        shape = CircleShape,
                    )
                    .padding(3.dp)
                    .clip(CircleShape)
                    .background(BvoxColors.Accent),
                contentAlignment = Alignment.Center,
            ) {
                if (avatar != null) {
                    Image(
                        bitmap = avatar,
                        contentDescription = "头像",
                        modifier = Modifier.fillMaxSize(),
                        contentScale = ContentScale.Crop,
                    )
                } else if (user == null) {
                    Icon(
                        imageVector = Icons.Rounded.Person,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(28.dp),
                    )
                } else {
                    Text(
                        text = user.take(1).uppercase(),
                        style = MaterialTheme.typography.titleLarge,
                        color = Color.White,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 14.dp),
            ) {
                Text(
                    text = when {
                        user != null -> "@$user"
                        state.hasCookie -> "离线"
                        else -> "未登录"
                    },
                    style = MaterialTheme.typography.titleMedium,
                    color = BvoxColors.TextPrimary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (state.vip) {
                        VipBadge()
                        Spacer(modifier = Modifier.width(6.dp))
                    }
                    Text(
                        text = when {
                            user != null -> "已登录 B 站账号"
                            state.hasCookie -> "离线，已保存登录状态"
                            else -> "登录后可同步收藏夹、查看歌词"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = BvoxColors.TextTertiary,
                    )
                }
            }
            if (hasSession) {
                TextButton(onClick = onLogout) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Rounded.Logout,
                        contentDescription = null,
                        tint = BvoxColors.TextTertiary,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("退出", color = BvoxColors.TextTertiary)
                }
            } else {
                PillButton(text = "登录", onClick = onLogin)
            }
        }
    }
}

/** Small 大会员 tag; Bilibili's membership color is pink, not gold. */
@Composable
private fun VipBadge() {
    val shape = RoundedCornerShape(BvoxRadius.Pill)
    Text(
        text = "大会员",
        modifier = Modifier
            .clip(shape)
            .background(BvoxColors.BrandPrimarySoft)
            .border(1.dp, BvoxColors.BrandPrimary.copy(alpha = 0.4f), shape)
            .padding(horizontal = 8.dp, vertical = 2.dp),
        style = MaterialTheme.typography.labelSmall,
        color = BvoxColors.BrandPrimary,
        fontWeight = FontWeight.SemiBold,
    )
}
