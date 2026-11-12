package dev.yoa.bilimusic

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {

    private val notificationPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val model = CoreHolder.init(applicationContext)
        CoreHolder.connectController(applicationContext)
        startStateReporter()

        requestNotificationPermissionIfNeeded()

        setContent {
            MaterialTheme {
                SkeletonApp(model)
            }
        }
    }

    /**
     * Media3 fires callbacks only on state changes; poll the controller twice
     * a second (mirroring the desktop mpv poller) and push snapshots into the
     * core, which drives progress rendering and EOF auto-advance.
     */
    private fun startStateReporter() {
        lifecycleScope.launch(Dispatchers.Default) {
            while (isActive) {
                CoreHolder.controllerFlow.value?.let { controller ->
                    val ended = controller.playbackState == androidx.media3.common.Player.STATE_ENDED
                    val idle = controller.playbackState == androidx.media3.common.Player.STATE_IDLE
                    val duration = controller.duration
                    CoreHolder.core.reportPlayerState(
                        dev.yoa.bilimusic.core.PlayerState(
                            position = controller.currentPosition / 1000.0,
                            duration = if (duration != androidx.media3.common.C.TIME_UNSET && duration > 0) duration / 1000.0 else 0.0,
                            paused = !controller.playWhenReady,
                            volume = controller.volume * 100.0,
                            eof = ended,
                            idle = idle,
                        )
                    )
                }
                delay(500)
            }
        }
    }

    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED
            ) {
                notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }
}

/** Phase-3 skeleton UI: proves the pipeline; the full screens land in Phase 4. */
@Composable
fun SkeletonApp(model: AppModel) {
    val snapshot by model.snapshot.collectAsState()

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = snapshot?.status ?: "正在初始化核心…",
                style = MaterialTheme.typography.bodyMedium,
                color = if (snapshot?.error == true) MaterialTheme.colorScheme.error
                else MaterialTheme.colorScheme.onSurface,
            )
            snapshot?.user?.let { user ->
                Text("已登录：$user", style = MaterialTheme.typography.bodySmall)
            }

            Button(onClick = { CoreHolder.core.startLogin() }) {
                Text("扫码登录")
            }

            SearchBar()

            snapshot?.let { state ->
                Text(state.queueTitle, style = MaterialTheme.typography.titleSmall)
                LazyColumn(state = rememberLazyListState(), modifier = Modifier.fillMaxWidth()) {
                    items(state.results.size) { index ->
                        val item = state.results[index]
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 4.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(item.title, style = MaterialTheme.typography.bodyMedium)
                                Text(
                                    "${item.author} · ${item.duration}",
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                            Button(onClick = { CoreHolder.core.play(index.toUInt()) }) {
                                Text("播放")
                            }
                        }
                    }
                }
                if (state.hasMedia) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = { CoreHolder.core.togglePause() }) {
                            Text(if (state.paused) "播放" else "暂停")
                        }
                        Button(onClick = { CoreHolder.core.nextTrack() }) {
                            Text("下一首")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SearchBar() {
    var keyword by remember { mutableStateOf("") }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        OutlinedTextField(
            value = keyword,
            onValueChange = { keyword = it },
            modifier = Modifier.weight(1f),
            singleLine = true,
            label = { Text("搜索") },
        )
        Button(
            onClick = {
                if (keyword.isNotBlank()) {
                    CoreHolder.core.search(keyword.trim())
                }
            },
        ) {
            Text("搜索")
        }
    }
}
