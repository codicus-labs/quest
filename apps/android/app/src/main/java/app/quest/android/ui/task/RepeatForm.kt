package app.quest.android.ui.task

import java.io.Serializable
import uniffi.quest_mobile.RepeatDraft
import uniffi.quest_mobile.RepeatFrequency
import uniffi.quest_mobile.RepeatPinChoice
import uniffi.quest_mobile.RepeatWeekday

/**
 * The repeat dialog's fields as the user edits them: [RepeatDraft], but storable in a
 * `SavedStateHandle`. Only a carrier; the core checks and interprets it.
 */
data class RepeatForm(
    val every: String,
    val frequency: RepeatFrequency,
    val pin: RepeatPinChoice,
    val weekdays: List<RepeatWeekday>,
    val lastDay: Boolean,
    val monthDay: String,
    val starts: String,
    val endsOnDate: Boolean,
    val ends: String,
    val timeZone: String,
) : Serializable {
    fun toDraft() = RepeatDraft(
        every = every,
        frequency = frequency,
        pin = pin,
        weekdays = weekdays,
        lastDay = lastDay,
        monthDay = monthDay,
        starts = starts,
        endsOnDate = endsOnDate,
        ends = ends,
        timeZone = timeZone,
    )

    companion object {
        fun of(draft: RepeatDraft) = RepeatForm(
            every = draft.every,
            frequency = draft.frequency,
            pin = draft.pin,
            weekdays = ArrayList(draft.weekdays),
            lastDay = draft.lastDay,
            monthDay = draft.monthDay,
            starts = draft.starts,
            endsOnDate = draft.endsOnDate,
            ends = draft.ends,
            timeZone = draft.timeZone,
        )
    }
}
