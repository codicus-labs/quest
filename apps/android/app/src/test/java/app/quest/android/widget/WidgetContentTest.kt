package app.quest.android.widget

import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.glance.appwidget.testing.unit.hasStartActivityClickAction
import androidx.glance.appwidget.testing.unit.isChecked
import androidx.glance.appwidget.testing.unit.runGlanceAppWidgetUnitTest
import androidx.glance.testing.unit.hasClickAction
import androidx.glance.testing.unit.hasContentDescriptionEqualTo
import androidx.glance.testing.unit.hasText
import androidx.glance.testing.unit.hasTextEqualTo
import app.quest.android.Samples
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/** The widget's content per size, over sample Home data (Quest Work: 3 tasks, Other: 1). */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class WidgetContentTest {
    private val context = RuntimeEnvironment.getApplication()
    private val ready = WidgetData.Ready(Samples.home)

    @Test
    fun openTasksComeFirstThenQuestWorkBeforeOther() {
        // t2 is done Quest Work: it goes after the open Other task t3.
        assertEquals(listOf("t1", "t4", "t3", "t2"), widgetTasks(Samples.home).map { it.id })
    }

    @Test
    fun theMoreLineTakesARowOnlyWhenTasksAreLeftOut() {
        assertEquals(3, visibleTasks(3, 220.dp))
        assertEquals(2, visibleTasks(4, 220.dp))
        assertEquals(0, visibleTasks(2, 72.dp))
        // A 3×2 widget on a phone (224 dp): three rows and "+N more".
        assertEquals(3, visibleTasks(7, 224.dp))
        // Glance keeps ten children per container: ten rows, or nine and "+N more".
        assertEquals(10, visibleTasks(10, 600.dp))
        assertEquals(9, visibleTasks(12, 600.dp))
    }

    @Test
    fun smallShowsOnlyTheHours() = runGlanceAppWidgetUnitTest {
        setAppWidgetSize(DpSize(120.dp, 60.dp))
        setContext(context)
        provideComposable { WidgetContent(ready) }

        onNode(hasTextEqualTo("1.5h")).assertExists()
        onNode(hasTextEqualTo("6.2h / 20h")).assertExists()
        onNode(hasText("Outline chapter 3")).assertDoesNotExist()
    }

    @Test
    fun mediumShowsTheTasksThatFitAndHowManyMore() = runGlanceAppWidgetUnitTest {
        setAppWidgetSize(DpSize(280.dp, 240.dp))
        setContext(context)
        provideComposable { WidgetContent(ready) }

        onNode(hasTextEqualTo("Monday, November 23")).assertExists()
        onNode(hasTextEqualTo("Outline chapter 3")).assertExists()
        // Open Other work stays in view; the done Quest Work task is the one left out.
        onNode(hasTextEqualTo("Call the bank")).assertExists()
        onNode(hasTextEqualTo("Morning run")).assertDoesNotExist()
        onNode(hasTextEqualTo("+1 more")).assertExists()
    }

    @Test
    fun aCheckTogglesItsTaskAndATitleOpensIt() = runGlanceAppWidgetUnitTest {
        setAppWidgetSize(DpSize(280.dp, 320.dp))
        setContext(context)
        provideComposable { WidgetContent(ready) }

        // Glance wraps a check's action in its own compound-button action, so only its presence is visible here.
        onNode(hasContentDescriptionEqualTo("Complete Outline chapter 3")).assert(hasClickAction())
        onNode(hasContentDescriptionEqualTo("Reopen Morning run")).assert(isChecked())
        onNode(hasTextEqualTo("Edit chapter 2")).assert(hasStartActivityClickAction(taskIntent(context, "t4")))
    }

    @Test
    fun anEmptyDaySaysSo() = runGlanceAppWidgetUnitTest {
        setAppWidgetSize(DpSize(280.dp, 140.dp))
        setContext(context)
        provideComposable { WidgetContent(WidgetData.Ready(Samples.emptyHome)) }

        onNode(hasTextEqualTo("Nothing planned for today.")).assertExists()
    }

    @Test
    fun aFailedCoreShowsWhy() = runGlanceAppWidgetUnitTest {
        setAppWidgetSize(DpSize(280.dp, 140.dp))
        setContext(context)
        provideComposable { WidgetContent(WidgetData.Failed("Could not open Quest's data.")) }

        onNode(hasTextEqualTo("Could not open Quest's data.")).assertExists()
    }
}
