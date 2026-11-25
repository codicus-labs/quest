package app.quest.android.auth

import android.content.ActivityNotFoundException
import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent

/**
 * Opens [url] in a Custom Tab (the user's browser, with its saved sign-ins; RFC 8252: never a WebView). False when
 * there is no browser to open it in.
 */
fun openInBrowser(context: Context, url: String): Boolean = try {
    CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(context, Uri.parse(url))
    true
} catch (_: ActivityNotFoundException) {
    false
}
