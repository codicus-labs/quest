package app.quest.android.auth

import java.io.File
import javax.crypto.KeyGenerator
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.SavedTokens
import uniffi.quest_mobile.TokenStoreException

/** With a plain AES key instead of the Keystore's (Robolectric has no AndroidKeyStore). */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class EncryptedTokenStoreTest {
    @get:Rule val folder = TemporaryFolder()

    private val key = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
    private val anna = "0123456789abcdef0123456789abcdef"
    private val ben = "fedcba9876543210fedcba9876543210"
    private val tokens = SavedTokens(accessToken = "access-secret", refreshToken = "refresh-secret", accessExpiresAtMs = 1_800_000_000_000u)

    private fun store(dir: File = folder.root) = EncryptedTokenStore(dir) { key }

    @Test
    fun savedTokensComeBackAndAreNotStoredInPlainText() {
        store().save(anna, tokens)

        assertEquals(tokens, store().load(anna))
        val bytes = File(folder.root, "$anna.tokens").readBytes()
        assertFalse(String(bytes, Charsets.ISO_8859_1).contains("secret"))
    }

    @Test
    fun withoutARefreshTokenAndUnknownAccounts() {
        val noRefresh = tokens.copy(refreshToken = null)
        store().save(anna, noRefresh)
        assertEquals(noRefresh, store().load(anna))
        assertNull(store().load(ben))
    }

    @Test
    fun deleteRemovesTheAccountsFileOnly() {
        store().save(anna, tokens)
        store().save(ben, tokens)

        store().delete(anna)

        assertNull(store().load(anna))
        assertEquals(tokens, store().load(ben))
    }

    @Test
    fun aFileMovedToAnotherAccountIsRefused() {
        store().save(anna, tokens)
        File(folder.root, "$anna.tokens").copyTo(File(folder.root, "$ben.tokens"))

        assertThrows(TokenStoreException.Failed::class.java) { store().load(ben) }
    }

    @Test
    fun aMalformedAccountKeyIsRefused() {
        assertThrows(TokenStoreException.Failed::class.java) { store().save("../escape", tokens) }
        assertThrows(TokenStoreException.Failed::class.java) { store().load("ABC") }
    }
}
