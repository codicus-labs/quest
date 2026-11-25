package app.quest.android

import app.quest.android.core.WorkspacesApi
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.TimerElsewhere
import uniffi.quest_mobile.WorkspaceRow
import uniffi.quest_mobile.WorkspacesData

const val DEFAULT_WORKSPACE = "00000000-0000-0000-0000-000000000000"

object WorkspaceSamples {
    private val default = WorkspaceRow(DEFAULT_WORKSPACE, "Default", "Current · 4 quests · 22 tasks · 19.8h tracked", current = true, canDelete = true)
    private val work = WorkspaceRow("w2", "Work", "Empty", current = false, canDelete = true)

    /** A new account: only the default workspace, which stays. */
    val one = WorkspacesData(
        active = default.copy(canDelete = false),
        live = listOf(default.copy(canDelete = false)),
        deleted = emptyList(),
        timerElsewhere = null,
    )

    /** Default (shown) and Work, and a deleted Side project. */
    val two = WorkspacesData(
        active = default,
        live = listOf(default, work),
        deleted = listOf(WorkspaceRow("w3", "Side project", "Empty", current = false, canDelete = false)),
        timerElsewhere = null,
    )

    /** Work shown while the timer runs in Default. */
    val timerInDefault = WorkspacesData(
        active = work.copy(current = true, summary = "Current · Empty"),
        live = listOf(default.copy(current = false, summary = "4 quests · 22 tasks · 19.8h tracked"), work.copy(current = true, summary = "Current · Empty")),
        deleted = emptyList(),
        timerElsewhere = TimerElsewhere(DEFAULT_WORKSPACE, "Default", "Design the next chapter", "36:00"),
    )
}

/**
 * The workspaces part of [FakeCore]: canned [data], every call in [calls]; a switch shows the asked
 * workspace. [failure] makes the next write fail with that message (as `InvalidForm`).
 */
class FakeWorkspaces(var data: WorkspacesData = WorkspaceSamples.one) : WorkspacesApi {
    val calls = mutableListOf<String>()
    var failure: String? = null
    var question: String = "Delete “Work”? It is empty. You can restore it under Deleted."

    private fun fail() {
        failure?.let {
            failure = null
            throw MobileException.InvalidForm(it)
        }
    }

    override fun workspaces(now: Moment): WorkspacesData = data

    override fun switchWorkspace(workspaceId: String) {
        calls += "switch $workspaceId"
        fail()
        val live = data.live.map { it.copy(current = it.id == workspaceId) }
        data = data.copy(active = live.first { it.current }, live = live, timerElsewhere = data.timerElsewhere?.takeIf { it.workspaceId != workspaceId })
    }

    override fun createWorkspace(name: String): String {
        calls += "create $name"
        fail()
        val row = WorkspaceRow("new", name.trim(), "Current · Empty", current = true, canDelete = true)
        data = data.copy(active = row, live = data.live.map { it.copy(current = false, canDelete = true) } + row)
        return row.id
    }

    override fun renameWorkspace(workspaceId: String, name: String) {
        calls += "rename $workspaceId $name"
        fail()
    }

    override fun deleteWorkspaceQuestion(workspaceId: String): String {
        calls += "ask delete $workspaceId"
        fail()
        return question
    }

    override fun deleteWorkspace(workspaceId: String, now: Moment) {
        calls += "delete $workspaceId"
        fail()
    }

    override fun restoreWorkspace(workspaceId: String) {
        calls += "restore $workspaceId"
        fail()
    }
}
