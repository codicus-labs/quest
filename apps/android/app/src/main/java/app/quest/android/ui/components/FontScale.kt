package app.quest.android.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.sp

/**
 * Text in [content] grows with the system font size up to [max] times, no further. Only for labels in fixed
 * geometry (navigation bar, the grid's hour column), where larger text would break words or overlap; the
 * labels stay readable through TalkBack and the screen's own large text.
 */
@Composable
fun CappedFontScale(max: Float, content: @Composable () -> Unit) {
    val density = LocalDensity.current
    if (density.fontScale <= max) return content()
    CompositionLocalProvider(LocalDensity provides Density(density.density, max), content = content)
}

/**
 * The least width of one figure in a strip of figures (Quests, a quest's page): six times body-size text, so it
 * grows with the text size (from a small size: Android scales large sizes less than small ones).
 */
@Composable
fun figureMinWidth(): Dp = with(LocalDensity.current) { 14.sp.toDp() * 6 }
