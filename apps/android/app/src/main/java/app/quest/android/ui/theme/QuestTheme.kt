package app.quest.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

private val LocalQuestColors = staticCompositionLocalOf { LightQuestColors }

/** Sync status colours. `live` and `offline` match the quest accents in `theme.rs` (dots only); `failed` is also text. */
object StatusColors {
    val live = Color(0xFF62B87A)
    val offline = Color(0xFFD98A4E)

    /** The `error` token, so failures read as text in both appearances (WCAG AA). */
    val failed: Color
        @Composable @ReadOnlyComposable
        get() = LocalQuestColors.current.error
}

/**
 * Quest's look on Material 3: colour tokens from `theme.json` (generated [QuestColors]), type
 * scale and spacing from `crates/quest-app/src/theme.rs`. Material components pick them up through
 * the colour scheme and typography; Quest's own components read [QuestTheme.colors].
 */
@Composable
fun QuestTheme(dark: Boolean, content: @Composable () -> Unit) {
    val colors = if (dark) DarkQuestColors else LightQuestColors
    CompositionLocalProvider(LocalQuestColors provides colors) {
        MaterialTheme(colorScheme = colorScheme(colors, dark), typography = materialTypography, content = content)
    }
}

object QuestTheme {
    val colors: QuestColors
        @Composable @ReadOnlyComposable
        get() = LocalQuestColors.current

    /** Whether the dark palette is in use (for "Toggle theme"). */
    val isDark: Boolean
        @Composable @ReadOnlyComposable
        get() = LocalQuestColors.current === DarkQuestColors
}

internal fun colorScheme(c: QuestColors, dark: Boolean): ColorScheme {
    val base = if (dark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = c.accent,
        onPrimary = if (dark) c.bar else Color.White,
        primaryContainer = c.selected,
        onPrimaryContainer = c.text,
        secondaryContainer = c.selected,
        onSecondaryContainer = c.text,
        background = c.bg,
        onBackground = c.text,
        surface = c.bg,
        onSurface = c.text,
        surfaceVariant = c.chip,
        onSurfaceVariant = c.textMuted,
        surfaceContainerLowest = c.card,
        surfaceContainerLow = c.card,
        surfaceContainer = c.bar,
        surfaceContainerHigh = c.cardRaised,
        surfaceContainerHighest = c.chip,
        inverseSurface = c.text,
        inverseOnSurface = c.bg,
        // Accent on the inverse surface (snackbar actions): the other appearance's accent keeps AA.
        inversePrimary = if (dark) LightQuestColors.accent else DarkQuestColors.accent,
        outline = c.borderStrong,
        outlineVariant = c.border,
        scrim = c.scrim,
        error = c.error,
        onError = if (dark) c.bar else Color.White,
    )
}

private val materialTypography = Typography(
    headlineMedium = QuestType.heading,
    headlineSmall = QuestType.focusTitle,
    titleLarge = QuestType.stat,
    titleMedium = QuestType.task,
    titleSmall = QuestType.bodyMedium,
    bodyLarge = QuestType.task,
    bodyMedium = QuestType.body,
    bodySmall = QuestType.small,
    labelLarge = QuestType.nav,
    labelMedium = QuestType.smallMedium,
    labelSmall = QuestType.caps,
)
