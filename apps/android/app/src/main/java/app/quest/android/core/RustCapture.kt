package app.quest.android.core

import app.quest.android.capture.CaptureChip
import app.quest.android.capture.CaptureDraft
import app.quest.android.capture.CaptureException
import app.quest.android.capture.CaptureFailure
import app.quest.android.capture.CaptureView
import app.quest.android.capture.DraftBlock
import app.quest.android.capture.DraftDay
import app.quest.android.capture.DraftMilestone
import app.quest.android.capture.DraftQuest
import app.quest.android.capture.DraftQuestOption
import app.quest.android.capture.DraftQuestRef
import app.quest.android.capture.DraftQuestView
import app.quest.android.capture.DraftTask
import app.quest.android.capture.DraftTaskView
import app.quest.android.capture.DraftWork
import app.quest.android.capture.DraftWorkTask
import app.quest.android.capture.DraftWorkView
import app.quest.android.ui.task.RepeatForm
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.QuestChip
import uniffi.quest_mobile.CaptureDraft as FfiDraft
import uniffi.quest_mobile.CaptureView as FfiView
import uniffi.quest_mobile.DraftBlock as FfiBlock
import uniffi.quest_mobile.DraftMilestone as FfiMilestone
import uniffi.quest_mobile.DraftQuest as FfiQuest
import uniffi.quest_mobile.DraftQuestRef as FfiQuestRef
import uniffi.quest_mobile.DraftTask as FfiTask
import uniffi.quest_mobile.DraftWork as FfiWork
import uniffi.quest_mobile.DraftWorkTask as FfiWorkTask

/*
 * The AI capture's records between UniFFI and the app's own types (capture/CaptureModel.kt), both ways for the
 * draft, one way for the view; and the core's capture errors as [CaptureException].
 */

/** Runs a capture call; the AI's error variants become [CaptureException]s, other errors pass through. */
internal inline fun <T> captureCall(call: () -> T): T = try {
    call()
} catch (e: MobileException) {
    throw captureFailure(e)?.let(::CaptureException) ?: e
}

internal fun captureFailure(e: MobileException): CaptureFailure? = when (e) {
    is MobileException.AiSignedOut -> CaptureFailure.SignedOut
    is MobileException.AiNotConfigured -> CaptureFailure.NotConfigured
    is MobileException.ServerUnreachable -> CaptureFailure.Offline(e.message.orEmpty())
    is MobileException.AiUnavailable -> CaptureFailure.Unavailable
    is MobileException.AiRateLimited -> CaptureFailure.RateLimited
    is MobileException.AiInvalidResponse -> CaptureFailure.InvalidResponse
    is MobileException.CaptureTextTooLong -> CaptureFailure.TextTooLong(e.message.orEmpty())
    is MobileException.AiDisabled, is MobileException.SessionExpired -> CaptureFailure.Other(e.message.orEmpty())
    else -> null
}

internal fun FfiDraft.toApp() = CaptureDraft(
    quests = quests.map { q -> DraftQuest(q.key, q.selected, q.title, q.description, q.milestones.map { DraftMilestone(it.title, it.day.toApp()) }) },
    tasks = tasks.map { t ->
        DraftTask(
            key = t.key,
            selected = t.selected,
            title = t.title,
            description = t.description,
            quest = t.quest?.toApp(),
            questUncertain = t.questUncertain,
            day = t.day?.toApp(),
            block = t.block?.let { DraftBlock(it.startMinutes.toInt(), it.endMinutes.toInt()) },
            repeat = t.repeat?.let(RepeatForm::of),
        )
    },
    work = work.map { w -> DraftWork(w.key, w.selected, w.task.toApp(), w.uncertain, w.day.toApp(), w.startMinutes.toInt(), w.endMinutes.toInt()) },
    dropped = ArrayList(dropped),
)

internal fun CaptureDraft.toFfi() = FfiDraft(
    quests = quests.map { q -> FfiQuest(q.key, q.selected, q.title, q.description, q.milestones.map { FfiMilestone(it.title, it.day.toFfi()) }) },
    tasks = tasks.map { t ->
        FfiTask(
            key = t.key,
            selected = t.selected,
            title = t.title,
            description = t.description,
            quest = t.quest?.toFfi(),
            questUncertain = t.questUncertain,
            day = t.day?.toFfi(),
            block = t.block?.let { FfiBlock(it.startMinutes.toUInt(), it.endMinutes.toUInt()) },
            repeat = t.repeat?.toDraft(),
        )
    },
    work = work.map { w -> FfiWork(w.key, w.selected, w.task.toFfi(), w.uncertain, w.day.toFfi(), w.startMinutes.toUInt(), w.endMinutes.toUInt()) },
    dropped = dropped,
)

internal fun FfiView.toApp() = CaptureView(
    quests = quests.map { DraftQuestView(it.key, it.colorHex, it.detail, it.error) },
    tasks = tasks.map { DraftTaskView(it.key, it.quest?.toApp(), it.`when`, it.blockStartLabel, it.blockEndLabel, it.repeatSummary, it.error) },
    work = work.map { DraftWorkView(it.key, it.taskTitle, it.quest?.toApp(), it.`when`, it.duration, it.startLabel, it.endLabel, it.error) },
    questOptions = questOptions.map { DraftQuestOption(it.quest.toApp(), it.chip.toApp()) },
    selectedCount = selectedCount.toInt(),
    canApply = canApply,
    applyLabel = applyLabel,
)

private fun DayValue.toApp() = DraftDay(iso, label)

private fun DraftDay.toFfi() = DayValue(iso, label)

private fun QuestChip.toApp() = CaptureChip(name, colorHex)

private fun FfiQuestRef.toApp(): DraftQuestRef = when (this) {
    is FfiQuestRef.Existing -> DraftQuestRef.Existing(id)
    is FfiQuestRef.New -> DraftQuestRef.New(key)
}

private fun DraftQuestRef.toFfi(): FfiQuestRef = when (this) {
    is DraftQuestRef.Existing -> FfiQuestRef.Existing(id)
    is DraftQuestRef.New -> FfiQuestRef.New(key)
}

private fun FfiWorkTask.toApp(): DraftWorkTask = when (this) {
    is FfiWorkTask.Existing -> DraftWorkTask.Existing(id, title)
    is FfiWorkTask.New -> DraftWorkTask.New(title, quest?.toApp())
}

private fun DraftWorkTask.toFfi(): FfiWorkTask = when (this) {
    is DraftWorkTask.Existing -> FfiWorkTask.Existing(id, title)
    is DraftWorkTask.New -> FfiWorkTask.New(title, quest?.toFfi())
}
