package app.quest.android.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.ui.theme.QuestTheme

/** A quest's share of the weekly goal as a ring in its colour, over a faint track. Decorative. */
@Composable
fun HoursRing(fraction: Float, color: Color, modifier: Modifier = Modifier, size: Dp = 22.dp, stroke: Dp = 3.dp) {
    val track = QuestTheme.colors.barTrack
    Canvas(modifier.size(size)) {
        val width = stroke.toPx()
        val inset = width / 2
        val arcSize = Size(this.size.width - width, this.size.height - width)
        val style = Stroke(width, cap = StrokeCap.Round)
        drawArc(track, 0f, 360f, useCenter = false, topLeft = Offset(inset, inset), size = arcSize, style = Stroke(width))
        if (fraction > 0f) {
            drawArc(color, -90f, 360f * fraction.coerceIn(0f, 1f), useCenter = false, topLeft = Offset(inset, inset), size = arcSize, style = style)
        }
    }
}
