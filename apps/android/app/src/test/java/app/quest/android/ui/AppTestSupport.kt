package app.quest.android.ui

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCapturePreferences
import app.quest.android.FakeClock
import app.quest.android.FakeFocusPreferences
import app.quest.android.FakeHomePreferences
import app.quest.android.FakePlanRules
import app.quest.android.FakeRepeatRules
import app.quest.android.core.CoreHolder
import app.quest.android.core.ServerUrlPolicy
import app.quest.android.settings.Appearance
import app.quest.android.settings.AppearanceRepository
import app.quest.android.ui.account.AccountViewModel
import app.quest.android.ui.archive.ArchiveViewModel
import app.quest.android.ui.focus.FocusViewModel
import app.quest.android.ui.home.HomeViewModel
import app.quest.android.ui.logwork.LogWorkViewModel
import app.quest.android.ui.plan.PlanViewModel
import app.quest.android.ui.quests.QuestDetailViewModel
import app.quest.android.ui.quests.QuestEditorViewModel
import app.quest.android.ui.quests.QuestsViewModel
import app.quest.android.ui.review.ReviewViewModel
import app.quest.android.ui.search.SearchViewModel
import app.quest.android.ui.settings.SettingsViewModel
import app.quest.android.ui.task.TaskSheetViewModel
import app.quest.android.ui.workspaces.WorkspacesViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.emptyFlow

/** An appearance that only remembers what was set. */
class FixedAppearance(initial: Appearance = Appearance.Light) : AppearanceRepository {
    override val appearance = MutableStateFlow(initial)

    override suspend fun set(appearance: Appearance) {
        this.appearance.value = appearance
    }
}

/** Every screen's ViewModel over [holder], with fakes for preferences and rules. */
fun testAppViewModels(holder: CoreHolder, appearance: AppearanceRepository = FixedAppearance()) = AppViewModels(
    home = HomeViewModel(holder, SavedStateHandle(), FakeHomePreferences()),
    taskSheet = TaskSheetViewModel(holder, SavedStateHandle(), FakeRepeatRules(), FakeClock()),
    account = AccountViewModel(holder, ServerUrlPolicy { null }, SavedStateHandle(), emptyFlow()),
    settings = SettingsViewModel(holder, appearance, FakeFocusPreferences(), FakeCapturePreferences()),
    focus = FocusViewModel(holder, SavedStateHandle()),
    plan = PlanViewModel(holder, SavedStateHandle(), FakeHomePreferences(), FakePlanRules(), FakeClock()),
    logWork = LogWorkViewModel(holder, SavedStateHandle()),
    quests = QuestsViewModel(holder, SavedStateHandle()),
    questEditor = QuestEditorViewModel(holder, SavedStateHandle()),
    questDetail = QuestDetailViewModel(holder, SavedStateHandle(), FakeHomePreferences()),
    review = ReviewViewModel(holder, SavedStateHandle()),
    archive = ArchiveViewModel(holder),
    search = SearchViewModel(holder, SavedStateHandle()),
    workspaces = WorkspacesViewModel(holder, SavedStateHandle()),
)
