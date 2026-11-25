package app.quest.android.quickadd

import android.content.Intent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SharedTextTest {
    private fun send(text: String?, subject: String? = null) =
        Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text).putExtra(Intent.EXTRA_SUBJECT, subject)

    @Test
    fun theSubjectBecomesTheFirstLine() {
        assertEquals("A page\nhttps://example.com", QuickAddActivity.sharedText(send("https://example.com", "A page")))
    }

    @Test
    fun textAlone() {
        assertEquals("Call Anna", QuickAddActivity.sharedText(send("Call Anna")))
    }

    @Test
    fun noShareNoText() {
        assertNull(QuickAddActivity.sharedText(Intent(QuickAddActivity::class.java.name)))
        assertNull(QuickAddActivity.sharedText(send("  ")))
    }
}
