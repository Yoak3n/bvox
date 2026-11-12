package dev.yoa.bilimusic.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color

/**
 * Renders the QR login matrix (`List<List<Boolean>>` from the core) as a
 * square canvas, sized to the smaller of the incoming width/height.
 */
@Composable
fun QrCodeView(matrix: List<List<Boolean>>, modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier) {
        val side = minOf(maxWidth, maxHeight)
        Canvas(modifier = Modifier.size(side)) {
            val n = matrix.size
            if (n == 0) return@Canvas
            val cell = size.width / n
            drawRect(Color.White)
            for (y in matrix.indices) {
                val row = matrix[y]
                for (x in row.indices) {
                    if (row[x]) {
                        drawRect(
                            Color.Black,
                            topLeft = Offset(x * cell, y * cell),
                            size = Size(cell, cell),
                        )
                    }
                }
            }
        }
    }
}
