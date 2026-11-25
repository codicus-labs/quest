package app.quest.android.capture

import android.content.ComponentName
import android.content.Intent
import android.text.SpannableString
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class CaptureStartTextTest {
    private val alias = ComponentName("app.quest.android", SHARE_ALIAS)

    @Test
    fun aShareGivesItsSubjectAndText() {
        val share = Intent(Intent.ACTION_SEND).setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, "Notes").putExtra(Intent.EXTRA_TEXT, SpannableString("Call Anna"))
        assertEquals("Notes\nCall Anna", CaptureActivity.startText(share.setComponent(alias)))
    }

    @Test
    fun theAppsOwnStartTextCounts() {
        val intent = Intent(CaptureActivity.ACTION_DICTATE).putExtra(CaptureActivity.EXTRA_START_TEXT, "Typed")
        assertEquals("Typed", CaptureActivity.startText(intent))
    }

    @Test
    fun throughTheAliasOnlyAShareCounts() {
        val named = Intent(CaptureActivity.ACTION_DICTATE).setComponent(alias).putExtra(CaptureActivity.EXTRA_START_TEXT, "Injected")
        assertNull(CaptureActivity.startText(named))
    }
}
