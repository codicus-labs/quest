package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.ui.theme.QuestTheme
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.captureScreenRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private const val SUBJECT = "screenshot-subject"

/** Robolectric's SDK 37 image draws some Compose frames blank; 36 renders reliably. */
private const val ROBOLECTRIC_SDK = 36

/**
 * Captures one scene in one appearance. Reference images live in the repo
 * (`src/test/screenshots/<scene>-<form>-<appearance>.png`); `recordRoborazziDebug` updates them.
 */
abstract class ScreenshotTest(private val scene: Scene, private val dark: Boolean) {
    @get:Rule val compose = createComposeRule()

    protected abstract val form: String

    /** How wide components are drawn on this form factor. */
    protected abstract val componentWidth: Dp

    @Test
    fun capture() {
        compose.setContent {
            QuestTheme(dark) {
                when (scene.kind) {
                    SceneKind.Screen, SceneKind.Window -> scene.content()
                    SceneKind.Component -> Column(
                        Modifier
                            .testTag(SUBJECT)
                            .width(componentWidth)
                            .background(QuestTheme.colors.bg)
                            .padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) { scene.content() }
                }
            }
        }
        val path = "src/test/screenshots/${scene.name}-$form-${if (dark) "dark" else "light"}.png"
        when (scene.kind) {
            SceneKind.Screen -> compose.onRoot().captureRoboImage(path)
            SceneKind.Component -> compose.onNodeWithTag(SUBJECT).captureRoboImage(path)
            SceneKind.Window -> {
                compose.waitForIdle()
                captureScreenRoboImage(path)
            }
        }
    }

    companion object {
        fun parameters(): List<Array<Any>> = scenes.flatMap { scene -> listOf(false, true).map { arrayOf(scene, it) } }

        /** The main screens at 200 % text size, light only: nothing may be cut off. */
        fun largeFontParameters(): List<Array<Any>> = scenes.filter { it.name in largeFontScenes }.map { arrayOf(it, false) }
    }
}

/** One scene per main screen and sheet, for the 200 % text size runs. */
private val largeFontScenes = setOf(
    "home", "focus-running", "focus-pick", "task-sheet", "plan", "plan-move-sheet", "calendar", "log-work", "entry-editor",
    "quests", "quest-detail", "workspaces-manage", "quest-editor", "review", "archive", "search", "account-signed-out", "account-signed-in",
    "settings", "quick-add-shared", "capture-dictate", "capture-listening", "capture-offline", "capture-proposal",
    "capture-proposal-task", "capture-proposal-work", "capture-applied", "settings-connections", "settings-connections-offline",
    "account-import-guest", "account-sign-out-warning", "settings-ai-privacy",
)

/** Compact width: bottom bar. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [ROBOLECTRIC_SDK], qualifiers = "w411dp-h914dp-port-mdpi")
class PhoneScreenshotTest(scene: Scene, dark: Boolean) : ScreenshotTest(scene, dark) {
    override val form = "phone"
    override val componentWidth = 411.dp

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0} dark={1}")
        fun parameters() = ScreenshotTest.parameters()
    }
}

/** Expanded width: navigation rail. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [ROBOLECTRIC_SDK], qualifiers = "w1280dp-h800dp-land-mdpi")
class TabletScreenshotTest(scene: Scene, dark: Boolean) : ScreenshotTest(scene, dark) {
    override val form = "tablet"
    override val componentWidth = 720.dp

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0} dark={1}")
        fun parameters() = ScreenshotTest.parameters()
    }
}

/** Compact width at 200 % text size (Settings → Display → Font size, largest). */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [ROBOLECTRIC_SDK], qualifiers = "w411dp-h914dp-port-mdpi", fontScale = 2f)
class PhoneLargeFontScreenshotTest(scene: Scene, dark: Boolean) : ScreenshotTest(scene, dark) {
    override val form = "phone-font200"
    override val componentWidth = 411.dp

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}")
        fun parameters() = ScreenshotTest.largeFontParameters()
    }
}

/** Expanded width at 200 % text size. */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [ROBOLECTRIC_SDK], qualifiers = "w1280dp-h800dp-land-mdpi", fontScale = 2f)
class TabletLargeFontScreenshotTest(scene: Scene, dark: Boolean) : ScreenshotTest(scene, dark) {
    override val form = "tablet-font200"
    override val componentWidth = 720.dp

    companion object {
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}")
        fun parameters() = ScreenshotTest.largeFontParameters()
    }
}
