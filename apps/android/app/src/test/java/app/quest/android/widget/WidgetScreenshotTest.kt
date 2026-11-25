package app.quest.android.widget

import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.glance.appwidget.compose
import app.quest.android.Samples
import com.github.takahirom.roborazzi.captureRoboImage
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.Robolectric
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The widget in its three sizes, light and dark: Glance composes RemoteViews, which are applied as
 * on a launcher and captured (`src/test/screenshots/widget-<size>-<appearance>.png`).
 */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-xhdpi")
class WidgetScreenshotTest(private val sizeName: String, widthDp: Int, heightDp: Int, private val dark: Boolean) {
    private val size = DpSize(widthDp.dp, heightDp.dp)

    @Test
    fun capture() {
        if (dark) RuntimeEnvironment.setQualifiers("+night")
        val activity = Robolectric.buildActivity(ComponentActivity::class.java).setup().get()
        val views = runBlocking { QuestWidget(WidgetData.Ready(Samples.home)).compose(activity, size = size) }
        val density = activity.resources.displayMetrics.density
        val host = FrameLayout(activity)
        activity.setContentView(host, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        val widget = views.apply(activity, host)
        host.addView(widget, FrameLayout.LayoutParams((size.width.value * density).toInt(), (size.height.value * density).toInt()))
        widget.captureRoboImage("src/test/screenshots/widget-$sizeName-${if (dark) "dark" else "light"}.png")
    }

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0} dark={3}")
        fun parameters(): List<Array<Any>> = listOf(
            Triple("small", 150, 80),
            Triple("medium", 300, 200),
            Triple("large", 300, 300),
        ).flatMap { (name, width, height) -> listOf(false, true).map { arrayOf(name, width, height, it) } }
    }
}
