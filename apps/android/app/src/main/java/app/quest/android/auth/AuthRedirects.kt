package app.quest.android.auth

import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow

/**
 * Redirects from the browser on their way to the Account screen's ViewModel: buffered, so one that arrives
 * before the activity is back (or while it is recreated) waits; each is handled once.
 */
class AuthRedirects {
    private val channel = Channel<String>(Channel.BUFFERED)

    val redirects: Flow<String> = channel.receiveAsFlow()

    fun deliver(url: String) {
        channel.trySend(url)
    }
}
