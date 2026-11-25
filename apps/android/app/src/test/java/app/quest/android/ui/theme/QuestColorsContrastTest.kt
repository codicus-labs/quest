package app.quest.android.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.luminance
import app.quest.android.ui.components.OnAccentFill
import app.quest.android.ui.tracked.TIMELINE_TINT
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * WCAG AA contrast of the colour tokens (`theme.json`, shared with the desktop) and of the Material
 * colour scheme built from them. Android's Accessibility Test Framework does not run its contrast
 * check under Robolectric (it needs a device screenshot), so the pairs are computed here.
 *
 * Text needs 4.5 : 1, UI components (lines, dots, icons, borders that carry meaning) 3 : 1. Not
 * checked: `textFaint` (done tasks, placeholders, past days: inactive content, exempt), `border`
 * and `borderStrong` (decoration; fields also have a label). Values: `docs/mobile.md`, „Phase 12: Kontraste“.
 */
class QuestColorsContrastTest {
    private fun contrast(foreground: Color, background: Color): Double {
        val (light, dark) = listOf(foreground.luminance(), background.luminance()).sortedDescending()
        return (light + 0.05) / (dark + 0.05)
    }

    private fun surfaces(c: QuestColors) = listOf(
        "bg" to c.bg,
        "card" to c.card,
        "cardRaised" to c.cardRaised,
        "bar" to c.bar,
        "chip" to c.chip,
        "selected" to c.selected,
        "hover" to c.hover,
    )

    /** Every pair that carries text, with its contrast. */
    private fun textPairs(c: QuestColors, dark: Boolean) = buildMap {
        for ((surface, background) in surfaces(c)) {
            for ((name, ink) in listOf("text" to c.text, "textMuted" to c.textMuted, "accent" to c.accent, "error" to c.error)) {
                put("$name on $surface", contrast(ink, background))
            }
        }
        for ((name, background) in listOf("bg" to c.bg, "card" to c.card)) {
            put("inline code on $name", contrast(c.text, c.textMuted.copy(alpha = 0.12f).compositeOver(background)))
        }
        put("Focus Done", contrast(OnAccentFill, c.accentFill))
        put("Organizing Stop", contrast(c.text, c.barTrack))
        val darkestTimeline = Color.Black.copy(alpha = TIMELINE_TINT).compositeOver(c.bg)
        val lightestTimeline = Color.White.copy(alpha = TIMELINE_TINT).compositeOver(c.bg)
        put("timeline text", contrast(c.text, if (dark) lightestTimeline else darkestTimeline))
        val scheme = colorScheme(c, dark)
        put("onPrimary on primary", contrast(scheme.onPrimary, scheme.primary))
        // Quests' FABs and filled buttons put `card` on the accent.
        put("card on accent (FABs, filled buttons)", contrast(c.card, c.accent))
        put("onError on error", contrast(scheme.onError, scheme.error))
        put("inverseOnSurface on inverseSurface (snackbar)", contrast(scheme.inverseOnSurface, scheme.inverseSurface))
        put("inversePrimary on inverseSurface (snackbar action)", contrast(scheme.inversePrimary, scheme.inverseSurface))
        // The swipe background: label in `text` on the action colour at 18 % over `bg`.
        for ((name, fill) in listOf("accent" to c.accent, "error" to c.error)) {
            put("text on $name tint", contrast(c.text, fill.copy(alpha = SWIPE_TINT).compositeOver(c.bg)))
        }
    }

    /** UI components: the swipe icons on their tint (the accent on surfaces is covered as text). */
    private fun componentPairs(c: QuestColors) = buildMap {
        for ((name, fill) in listOf("accent" to c.accent, "error" to c.error)) {
            put("$name icon on its tint", contrast(fill, fill.copy(alpha = SWIPE_TINT).compositeOver(c.bg)))
        }
        val darkestTimeline = Color.Black.copy(alpha = TIMELINE_TINT).compositeOver(c.bg)
        val lightestTimeline = Color.White.copy(alpha = TIMELINE_TINT).compositeOver(c.bg)
        put("outside-focus hatch", minOf(contrast(c.textMuted, darkestTimeline), contrast(c.textMuted, lightestTimeline)))
        put("review day bars on track", contrast(c.accent, c.barTrack))
        put("checklist drop outline", contrast(c.borderStrong, c.textMuted.copy(alpha = 0.08f).compositeOver(c.card)))
    }

    private fun belowAa(c: QuestColors, dark: Boolean) =
        textPairs(c, dark).filterValues { it < AA_TEXT }.keys + componentPairs(c).filterValues { it < AA_COMPONENT }.keys

    @Test
    fun lightMeetsAa() = assertEquals(emptySet<String>(), belowAa(LightQuestColors, dark = false))

    @Test
    fun darkMeetsAa() = assertEquals(emptySet<String>(), belowAa(DarkQuestColors, dark = true))

    private companion object {
        const val AA_TEXT = 4.5
        const val AA_COMPONENT = 3.0

        /** Alpha of the swipe background in `SwipeTaskCard`. */
        const val SWIPE_TINT = 0.18f
    }
}
