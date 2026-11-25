package app.quest.android.ui.theme

import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.quest.android.R

/** Inter, as the desktop app (Rune embeds it). */
val Inter = FontFamily(
    Font(R.font.inter_regular, FontWeight.Normal),
    Font(R.font.inter_medium, FontWeight.Medium),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
)

/** Letter spacing in px at a font size, as `theme.rs` gives it, in em for Compose. */
private fun tracking(px: Float, size: Float) = (px / size).em

private fun style(size: Float, weight: Int, lineHeight: Float, trackingPx: Float = 0f, features: String? = null) =
    TextStyle(
        fontFamily = Inter,
        fontSize = size.sp,
        fontWeight = FontWeight(weight),
        lineHeight = lineHeight.sp,
        letterSpacing = tracking(trackingPx, size),
        fontFeatureSettings = features,
    )

/** The type scale from `crates/quest-app/src/theme.rs` (`mod ty`). */
object QuestType {
    val body = style(14f, 400, 20f)
    val bodyMedium = style(14f, 500, 20f)
    val small = style(12f, 400, 16f)
    val smallMedium = style(12f, 500, 16f)
    val caps = style(11f, 500, 14f, trackingPx = 1.4f)
    val nav = style(14f, 500, 20f)
    val task = style(15f, 400, 21f)
    val heading = style(30f, 500, 38f, trackingPx = -0.4f)
    val stat = style(20f, 500, 26f)
    val bigStat = style(30f, 500, 36f)
    val focusTitle = style(26f, 400, 32f)
    val timer = style(120f, 500, 130f, trackingPx = -2f, features = "tnum")
    val monoTime = style(14f, 500, 20f, features = "tnum")
}

/** Spacing and radii used by the desktop app (`crates/quest-app/src/ui`), named by role. */
object QuestSpacing {
    val hairline = 1.dp
    val tight = 6.dp
    val small = 8.dp
    /** Between cards in a list (desktop `LIST_GAP`). */
    val listGap = 10.dp
    val medium = 12.dp
    /** Inside a card (desktop task card `py(14)`). */
    val cardPadding = 14.dp
    val large = 16.dp
    /** Screen edges on a phone. */
    val screen = 20.dp
    /** Between sections (desktop "Other" header `mt(28)`). */
    val section = 28.dp

    val cardRadius = 10.dp
    val fieldRadius = 6.dp
    val chipHeight = 26.dp
    val checkSize = 24.dp
    /** Minimum touch target (Material accessibility guideline). */
    val touchTarget = 48.dp
    /** Controls on a task card: drawn smaller to keep cards short; their touch target stays [touchTarget]. */
    val cardControl = 36.dp
}
