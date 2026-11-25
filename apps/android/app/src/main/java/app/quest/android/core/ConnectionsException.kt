package app.quest.android.core

/**
 * Listing or revoking connected AI clients failed; thrown by `QuestCoreApi.aiConnections` and `revokeAiConnection`.
 * [offline]: the Quest server was not reachable; [gone]: the client was no longer connected (revoked elsewhere), so
 * revoking it has nothing left to do.
 */
class ConnectionsException(message: String, val offline: Boolean = false, val gone: Boolean = false) : Exception(message)
