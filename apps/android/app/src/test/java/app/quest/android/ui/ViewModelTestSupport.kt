package app.quest.android.ui

import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.core.CoreHolder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.rules.TestWatcher
import org.junit.runner.Description

/** Runs `viewModelScope` on a test dispatcher that executes eagerly. */
@OptIn(ExperimentalCoroutinesApi::class)
class MainDispatcherRule(val dispatcher: TestDispatcher = UnconfinedTestDispatcher()) : TestWatcher() {
    override fun starting(description: Description) = Dispatchers.setMain(dispatcher)

    override fun finished(description: Description) = Dispatchers.resetMain()
}

/** A holder over [core] that opens right away on [dispatcher]. */
fun openedHolder(core: FakeCore, scope: CoroutineScope, dispatcher: TestDispatcher): CoreHolder =
    CoreHolder(scope, dispatcher, FakeClock()) { onChange -> core.also { it.onChange = onChange } }
