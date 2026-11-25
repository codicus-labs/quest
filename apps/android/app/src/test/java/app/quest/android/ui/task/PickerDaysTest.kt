package app.quest.android.ui.task

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PickerDaysTest {
    @Test
    fun isoDaysRoundTripThroughThePickersUtcMidnights() {
        val millis = PickerDays.toPickerMillis("2026-11-24")

        assertEquals(1_795_478_400_000L, millis)
        assertEquals("2026-11-24", PickerDays.toIso(millis!!))
    }

    @Test
    fun anythingButAnIsoDayHasNoPickerDay() {
        assertNull(PickerDays.toPickerMillis("Nov 24, 2026"))
        assertNull(PickerDays.toPickerMillis(""))
        assertNull(PickerDays.toPickerMillis(null))
    }
}
