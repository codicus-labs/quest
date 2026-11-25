package app.quest.android.auth

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import app.quest.android.MainActivity
import app.quest.android.QuestApplication

/**
 * Receives `app.quest.android:/oauth2redirect` (RFC 8252 private-use scheme, in every build): the answer to a
 * sign-in, or the return from signing out. Hands the address to the app and brings [MainActivity] back to the front, which closes
 * the Custom Tab above it. Shows nothing itself. Exported, as the browser must reach it; the core checks that an
 * answer belongs to the sign-in it started (state, PKCE), so a forged one is refused.
 */
class AuthRedirectActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // A restored instance already handed its address over.
        if (savedInstanceState == null) intent?.data?.let { (application as QuestApplication).authRedirects.deliver(it.toString()) }
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP))
        finish()
    }
}
