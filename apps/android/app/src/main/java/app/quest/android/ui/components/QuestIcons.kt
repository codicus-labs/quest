package app.quest.android.ui.components

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/** Outline icon from the desktop's 16-unit SVG paths (`crates/quest-app/src/icons.rs`); tinted by `Icon`. */
private fun outline(name: String, vararg paths: String, width: Float = 1.2f): ImageVector {
    val builder = ImageVector.Builder(name, 24.dp, 24.dp, 16f, 16f)
    paths.forEach { path ->
        builder.addPath(
            pathData = addPathNodes(path),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = width,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        )
    }
    return builder.build()
}

/** The desktop's icons that Material's core set lacks. */
object QuestIcons {
    val Pause = outline("pause", "M4 3h2.6v10H4Z", "M9.4 3h2.6v10H9.4Z", width = 1.1f)
    val Filter = outline("filter", "M2.5 3h11L9.2 8.2V13L6.8 11.8V8.2Z", width = 1.1f)
    val ToTop = outline("to-top", "M3.5 2.5h9M8 13.5V5.5M4.8 8.5L8 5.3L11.2 8.5")
    val ToBottom = outline("to-bottom", "M3.5 13.5h9M8 2.5v8M4.8 7.5L8 10.7L11.2 7.5")
    val Eye = outline(
        "eye",
        "M1.5 8C3 5 5.3 3.5 8 3.5S13 5 14.5 8C13 11 10.7 12.5 8 12.5S3 11 1.5 8Z",
        "M10 8A2 2 0 1 1 6 8A2 2 0 1 1 10 8Z",
        width = 1.1f,
    )
    val EyeOff = outline(
        "eye-off",
        "M6.3 3.7A6.9 6.9 0 0 1 8 3.5C10.7 3.5 13 5 14.5 8a9.6 9.6 0 0 1-1.6 2.3M10.1 12.1A6.4 6.4 0 0 1 8 12.5C5.3 12.5 3 11 1.5 8a9.4 9.4 0 0 1 2.4-3M6.6 6.6a2 2 0 0 0 2.8 2.8M2.5 2.5l11 11",
        width = 1.1f,
    )
    val Trash = outline("trash", "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.3c0 .4.4.7.8.7h4.2c.4 0 .8-.3.8-.7l.6-8.3", width = 1.1f)
    val Focus = outline("focus", "M13.5 9A5.5 5.5 0 1 1 2.5 9A5.5 5.5 0 1 1 13.5 9Z", "M8 9V6.2", "M6.5 1.8h3", width = 1.1f)
    /** A task without a quest: a circle with a plus (no desktop icon; the desktop shows "No quest"). */
    val AssignQuest = outline("assign-quest", "M13 8A5 5 0 1 1 3 8A5 5 0 1 1 13 8Z", "M8 5.6v4.8M5.6 8h4.8", width = 1.1f)
    val Archive = outline("archive", "M2.5 3h11v3h-11Z", "M3.5 6v7h9V6", "M6.5 8.5h3", width = 1.1f)
    val Trophy = outline("trophy", "M5 2.5h6v3.5a3 3 0 0 1-6 0Z", "M5 4H3c0 1.8 1 2.8 2.2 2.9M11 4h2c0 1.8-1 2.8-2.2 2.9", "M8 9v2.5M5.5 13.5h5M6.5 11.5h3", width = 1.1f)
    val Restore = outline("restore", "M3 8a5 5 0 1 0 1.5-3.5", "M2.8 2.5v2.5h2.5", width = 1.1f)
    /** Dictation (no desktop icon). */
    val Mic = outline("mic", "M6 3.5a2 2 0 0 1 4 0v4a2 2 0 0 1-4 0Z", "M3.8 7.5a4.2 4.2 0 0 0 8.4 0", "M8 11.7v2.3M6 14h4", width = 1.1f)
    /** AI capture (no desktop icon): two sparks. */
    val Sparkle = outline("sparkle", "M6.5 2.5l1.2 3.3L11 7l-3.3 1.2L6.5 11.5L5.3 8.2L2 7l3.3-1.2Z", "M12 10l.5 1.5L14 12l-1.5.5L12 14l-.5-1.5L10 12l1.5-.5Z", width = 1.1f)
    /** An unsure guess of the AI (no desktop icon): a question mark in a circle. */
    val Unsure = outline("unsure", "M13.5 8A5.5 5.5 0 1 1 2.5 8A5.5 5.5 0 1 1 13.5 8Z", "M6.4 6.3a1.7 1.7 0 1 1 2.4 1.5c-.5.3-.8.6-.8 1.2", "M8 11h.01", width = 1.1f)
    val Open = outline("open", "M6 3.5H4a.5.5 0 0 0-.5.5v8c0 .3.2.5.5.5h8a.5.5 0 0 0 .5-.5v-2M9 3h4v4M13 3L7.5 8.5", width = 1.1f)
    val Layers = outline("layers", "M8 2.5L14 5.5L8 8.5L2 5.5Z", "M2 8.25L8 11.25L14 8.25M2 10.75L8 13.75L14 10.75", width = 1.1f)
    val ChevronDown = outline("chevron-down", "M4.5 6.5L8 10L11.5 6.5", width = 1.1f)
    val Inbox = outline("inbox", "M2.5 9.5h3l1 1.5h3l1-1.5h3", "M4 3.5h8l1.5 6v3a.5.5 0 0 1-.5.5H3a.5.5 0 0 1-.5-.5v-3Z", width = 1.1f)
    val Today = outline("today", "M10.8 8a2.8 2.8 0 1 1-5.6 0 2.8 2.8 0 1 1 5.6 0Z", "M8 1.5v1.8M8 12.7v1.8M1.5 8h1.8M12.7 8h1.8M3.4 3.4l1.3 1.3M11.3 11.3l1.3 1.3M3.4 12.6l1.3-1.3M11.3 4.7l1.3-1.3", width = 1.1f)
    val Flag = outline("flag", "M3.3 14V2.7h7.4L9.3 5.3 10.7 8H3.3", width = 1.1f)
    val Logbook = outline("logbook", "M4.5 2.5h7a1 1 0 0 1 1 1v10h-8a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1Z", "M6 6.5l1.3 1.3L10 5", width = 1.1f)
    val Stop = ImageVector.Builder("stop", 24.dp, 24.dp, 16f, 16f)
        .addPath(pathData = addPathNodes("M4 4h8v8H4Z"), fill = SolidColor(Color.Black))
        .build()
    val Repeat = outline("repeat", "M3 7V6.2C3 5 4 4 5.2 4H13M11 2l2 2-2 2M13 9v.8C13 11 12 12 10.8 12H3M5 14l-2-2 2-2", width = 1.1f)
}
