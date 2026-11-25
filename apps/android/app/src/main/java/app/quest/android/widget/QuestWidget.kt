package app.quest.android.widget

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.LocalContext
import androidx.glance.LocalSize
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.CheckBox
import androidx.glance.appwidget.CheckboxDefaults
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.appWidgetBackground
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.color.ColorProvider
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextDecoration
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import androidx.compose.runtime.collectAsState
import app.quest.android.INTERNAL_ENTRY
import app.quest.android.MainActivity
import app.quest.android.R
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.DarkQuestColors
import app.quest.android.ui.theme.LightQuestColors
import app.quest.android.ui.theme.QuestColors
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeTask

/** The least room for the tasks; below it, only the hours (today, this week). */
private val Medium = DpSize(250.dp, 110.dp)

/** Vertical padding (both edges), day label and the hours row: what [LocalSize] height leaves no rows (measured on a device). */
private val TasksTop = 60.dp

/** One task row: the 48 dp check. */
private val TaskRowHeight = 48.dp

/** The "+N more" line. */
private val MoreLine = 18.dp

/** Glance drops a container's children past ten (RemoteViews limit). */
private const val MAX_CHILDREN = 10

/**
 * The home-screen widget: today's Quest Hours and tasks (Quest Work first), check off in place, tap
 * a task for its sheet, anywhere else for the app. Data comes from [WidgetUpdater]; the colours are
 * Quest's own tokens (light and dark follow the system), not dynamic colour, so it looks like the app.
 * Exact size mode: the task rows that fit follow the widget's real height, not a size bucket.
 * [fixed] draws given data instead (screenshots).
 */
class QuestWidget(private val fixed: WidgetData? = null) : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        if (fixed != null) return provideContent { WidgetContent(fixed) }
        val widgets = (context.applicationContext as WidgetHost).widgets
        // A redraw from refresh() has fresh data already; a cold start or a launcher update has not.
        if (widgets.data.value !is WidgetData.Ready) widgets.reload()
        provideContent {
            val data by widgets.data.collectAsState()
            WidgetContent(data)
        }
    }
}

/** Light and dark token pairs as Glance colours. */
private object WidgetColors {
    private fun pick(token: (QuestColors) -> Color) = ColorProvider(day = token(LightQuestColors), night = token(DarkQuestColors))

    val bg = pick { it.bg }
    val text = pick { it.text }
    val muted = pick { it.textMuted }
    val faint = pick { it.textFaint }
    val accent = pick { it.accent }
}

@Composable
fun WidgetContent(data: WidgetData) {
    val context = LocalContext.current
    Box(
        GlanceModifier
            .fillMaxSize()
            .appWidgetBackground()
            .cornerRadius(16.dp)
            .background(WidgetColors.bg)
            .clickable(actionStartActivity<MainActivity>())
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        when (data) {
            WidgetData.Opening -> Note(context.getString(R.string.widget_opening))
            is WidgetData.Failed -> Note(data.message)
            is WidgetData.Ready -> {
                val size = LocalSize.current
                if (size.width < Medium.width || size.height < Medium.height) SmallContent(data.home) else TasksContent(data.home, size.height)
            }
        }
    }
}

@Composable
private fun Note(text: String) {
    Text(text, style = TextStyle(color = WidgetColors.muted, fontSize = 13.sp))
}

/** Today and this week stacked; nothing else fits. */
@Composable
private fun SmallContent(home: HomeData) {
    val context = LocalContext.current
    Column(GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        Stat(context.getString(R.string.widget_today), home.todayHours)
        Spacer(GlanceModifier.height(4.dp))
        Stat(context.getString(R.string.widget_week), context.getString(R.string.widget_week_of_target, home.weekHours, home.weeklyTarget))
    }
}

@Composable
private fun Stat(label: String, value: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(label.uppercase(), style = TextStyle(color = WidgetColors.muted, fontSize = 11.sp, fontWeight = FontWeight.Medium))
        Spacer(GlanceModifier.width(6.dp))
        Text(value, style = TextStyle(color = WidgetColors.text, fontSize = 15.sp, fontWeight = FontWeight.Medium))
    }
}

/** The header, then as many of today's tasks as fit in [height]; the rest as "+N more" (a tap opens the app). */
@Composable
private fun TasksContent(home: HomeData, height: Dp) {
    val context = LocalContext.current
    val tasks = widgetTasks(home)
    val limit = visibleTasks(tasks.size, height)
    Column(GlanceModifier.fillMaxSize()) {
        Text(home.dayLabel, style = TextStyle(color = WidgetColors.text, fontSize = 14.sp, fontWeight = FontWeight.Medium), maxLines = 1)
        Row(GlanceModifier.padding(top = 2.dp, bottom = 2.dp)) {
            Stat(context.getString(R.string.widget_today), home.todayHours)
            Spacer(GlanceModifier.width(12.dp))
            Stat(context.getString(R.string.widget_week), context.getString(R.string.widget_week_of_target, home.weekHours, home.weeklyTarget))
        }
        when {
            tasks.isEmpty() -> Note(context.getString(R.string.widget_empty))
            // Its own column: the header does not count against the rows' ten children.
            else -> Column(GlanceModifier.fillMaxWidth()) {
                tasks.take(limit).forEach { TaskRow(it) }
                val more = tasks.size - limit
                if (more > 0) Note(context.resources.getQuantityString(R.plurals.widget_more, more, more))
            }
        }
    }
}

/** Rows that fit below the header (ten at most); when not all do, one line is kept for "+N more". */
fun visibleTasks(count: Int, height: Dp): Int {
    val rows = ((height - TasksTop) / TaskRowHeight).toInt().coerceIn(0, MAX_CHILDREN)
    if (count <= rows) return count
    return ((height - TasksTop - MoreLine) / TaskRowHeight).toInt().coerceIn(0, MAX_CHILDREN - 1)
}

/**
 * Open tasks before done ones, so done Quest Work never pushes open Other work out of the few rows;
 * within each, Quest Work first, then Other, each in Home's order.
 */
fun widgetTasks(home: HomeData): List<HomeTask> = (home.questWork + home.other).sortedBy { it.done }

/** Check (48 dp), quest dot (none without a quest) and title; the title opens the task's sheet. */
@Composable
private fun TaskRow(task: HomeTask) {
    val context = LocalContext.current
    val toggleLabel = context.getString(if (task.done) R.string.widget_reopen_task else R.string.widget_complete_task, task.title)
    Row(GlanceModifier.fillMaxWidth().height(TaskRowHeight), verticalAlignment = Alignment.CenterVertically) {
        val toggle = actionRunCallback<ToggleTaskAction>(actionParametersOf(ToggleTaskAction.TaskIdKey to task.id))
        // The box is the 48 dp touch target and centres the check, which draws at its own size.
        Box(GlanceModifier.size(TaskRowHeight).clickable(toggle), contentAlignment = Alignment.Center) {
            CheckBox(
                checked = task.done,
                onCheckedChange = toggle,
                colors = CheckboxDefaults.colors(checkedColor = WidgetColors.accent, uncheckedColor = WidgetColors.faint),
                modifier = GlanceModifier.semantics { contentDescription = toggleLabel },
            )
        }
        // Without a quest no dot (a blank of the same width keeps the titles aligned).
        val quest = task.quest
        if (quest == null) {
            Spacer(GlanceModifier.size(8.dp))
        } else {
            Box(GlanceModifier.size(8.dp).cornerRadius(4.dp).background(ColorProvider(questColor(quest.colorHex)))) {}
        }
        Text(
            task.title,
            style = TextStyle(
                color = if (task.done) WidgetColors.faint else WidgetColors.text,
                fontSize = 14.sp,
                textDecoration = if (task.done) TextDecoration.LineThrough else TextDecoration.None,
            ),
            maxLines = 1,
            modifier = GlanceModifier
                .defaultWeight()
                .padding(start = 8.dp)
                .clickable(actionStartActivity(taskIntent(context, task.id))),
        )
    }
}

/** quest://task/<id> to the app: it opens the task's sheet. */
fun taskIntent(context: Context, taskId: String): Intent =
    Intent(Intent.ACTION_VIEW, Uri.parse("quest://task/${Uri.encode(taskId)}")).setClassName(context, INTERNAL_ENTRY)
