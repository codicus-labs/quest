package app.quest.android.quickadd

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.quest.android.QuestApplication
import app.quest.android.capture.CaptureActivity
import app.quest.android.core.RustDayLabels
import app.quest.android.settings.Appearance
import app.quest.android.ui.theme.QuestTheme
import java.time.LocalDate

/**
 * A slim, transparent activity with the quick-add sheet: the share target for text, the "New task"
 * shortcut and the Quick Settings tile. It shares the process's core; adding closes it.
 */
class QuickAddActivity : ComponentActivity() {
    private val app get() = application as QuestApplication

    private val quickAdd: QuickAddViewModel by viewModels {
        viewModelFactory { initializer { QuickAddViewModel(app.core, createSavedStateHandle(), RustDayLabels, sharedText(intent), planDay(intent)) } }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val appearance = app.appearance.appearance.collectAsStateWithLifecycle(null).value ?: return@setContent
            val dark = when (appearance) {
                Appearance.System -> isSystemInDarkTheme()
                Appearance.Light -> false
                Appearance.Dark -> true
            }
            val state by quickAdd.state.collectAsStateWithLifecycle()
            val captureShown by app.core.aiFeatures.collectAsStateWithLifecycle(false)
            LaunchedEffect(state) { if (state == QuickAddUiState.Added) finish() }
            QuestTheme(dark) {
                QuickAddSheet(
                    state,
                    QuickAddActions(
                        close = ::finish,
                        editTitle = quickAdd::editTitle,
                        editDescription = quickAdd::editDescription,
                        setQuest = quickAdd::setQuest,
                        setDay = quickAdd::setDay,
                        add = quickAdd::add,
                        dictate = if (captureShown) ::dictate else null,
                    ),
                )
            }
        }
    }

    /** Hands what is typed to AI capture, which starts listening; quick add closes. */
    private fun dictate() {
        val ready = quickAdd.state.value as? QuickAddUiState.Ready
        val typed = ready?.inputs?.let { listOf(it.title, it.description).filter(String::isNotBlank).joinToString("\n") }
        startActivity(CaptureActivity.intent(this, typed, dictate = true))
        finish()
    }

    companion object {
        private const val PLAN_DAY = "app.quest.android.extra.PLAN_DAY"

        fun forDay(context: Context, date: String) =
            Intent(context, QuickAddActivity::class.java).putExtra(PLAN_DAY, date)

        private fun planDay(intent: Intent): String? = intent.getStringExtra(PLAN_DAY)?.let {
            runCatching { LocalDate.parse(it).toString() }.getOrNull()
        }

        /** Shared text with its subject (a page title, say) as the first line, so the core makes it the title. */
        fun sharedText(intent: Intent?): String? {
            if (intent?.action != Intent.ACTION_SEND) return null
            val text = intent.getStringExtra(Intent.EXTRA_TEXT)
            val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)
            return listOfNotNull(subject, text).filter { it.isNotBlank() }.joinToString("\n").ifEmpty { null }
        }
    }
}
