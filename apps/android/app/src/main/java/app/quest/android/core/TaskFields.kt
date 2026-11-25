package app.quest.android.core

import uniffi.quest_mobile.CalendarStep
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.QuickFields
import uniffi.quest_mobile.shiftCalendarDate

internal fun CoreHolder.readCalendar(cursor: String?, fields: QuickFields, target: DateTarget) =
    (read { core, now -> core.taskCalendar(cursor, fields, target, now) } as? Outcome.Done)?.value

internal fun CoreHolder.shiftTaskMonth(cursor: String, forward: Boolean) =
    (read { _, now -> shiftCalendarDate(cursor, CalendarStep.MONTH, forward, now) } as? Outcome.Done)?.value?.iso
