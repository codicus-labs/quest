package app.quest.android.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow

/** About as wide as any week range ("Sep 28 – Oct 4") or month ("September 2026") gets. */
const val WIDEST_WEEK = "Nov 30 – Dec 6"
const val WIDEST_MONTH = "September 2026"

/**
 * The text between a period's back and forward arrows, as wide as [widest] at every font size, so the arrows
 * stay put while one steps through periods with shorter or longer labels.
 */
@Composable
fun PeriodLabel(
    text: String,
    widest: String,
    style: TextStyle,
    color: Color,
    modifier: Modifier = Modifier,
    textModifier: Modifier = Modifier,
    alignment: Alignment = Alignment.Center,
) {
    // Never narrower than [widest]; a longer label still shows whole if there's room.
    Box(modifier, contentAlignment = alignment) {
        Text(widest, style = style, maxLines = 1, modifier = Modifier.alpha(0f).clearAndSetSemantics {})
        Text(
            text,
            style = style,
            color = color,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = textModifier,
        )
    }
}
