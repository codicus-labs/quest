package app.quest.android.auth

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import java.io.File
import java.io.IOException
import java.nio.ByteBuffer
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.json.JSONException
import org.json.JSONObject
import uniffi.quest_mobile.SavedTokens
import uniffi.quest_mobile.TokenStore
import uniffi.quest_mobile.TokenStoreException

/** An account key from the core: 32 hex characters, so it is safe as a file name. */
private val ACCOUNT_KEY = Regex("[0-9a-f]{32}")

/** File format version; bump when the layout changes. */
private const val FORMAT: Byte = 1
private const val IV_BYTES = 12
private const val TAG_BITS = 128

/**
 * The accounts' tokens for the core ([TokenStore]): one file per account in [dir], encrypted with AES-GCM under
 * [key] (the Android Keystore's in the app; the account key is the associated data, so a file cannot be moved to
 * another account). The core calls this from its background threads only.
 */
class EncryptedTokenStore(private val dir: File, private val key: () -> SecretKey) : TokenStore {
    private val lock = Any()

    override fun load(key: String): SavedTokens? = guarded(key) {
        val file = file(key)
        if (!file.exists()) return@guarded null
        decode(decrypt(AtomicFile(file).readFully(), key))
    }

    override fun save(key: String, tokens: SavedTokens) = guarded(key) {
        dir.mkdirs()
        val atomic = AtomicFile(file(key))
        val out = atomic.startWrite()
        try {
            out.write(encrypt(encode(tokens), key))
            atomic.finishWrite(out)
        } catch (e: IOException) {
            atomic.failWrite(out)
            throw e
        }
    }

    override fun delete(key: String) = guarded(key) {
        AtomicFile(file(key)).delete()
    }

    private fun file(key: String) = File(dir, "$key.tokens")

    private fun encrypt(plain: ByteArray, account: String): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(account.toByteArray())
        val sealed = cipher.doFinal(plain)
        val iv = cipher.iv
        return ByteBuffer.allocate(1 + iv.size + sealed.size).put(FORMAT).put(iv).put(sealed).array()
    }

    private fun decrypt(bytes: ByteArray, account: String): ByteArray {
        if (bytes.size <= 1 + IV_BYTES || bytes[0] != FORMAT) throw GeneralSecurityException("unknown token file format")
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, bytes, 1, IV_BYTES))
        cipher.updateAAD(account.toByteArray())
        return cipher.doFinal(bytes, 1 + IV_BYTES, bytes.size - 1 - IV_BYTES)
    }

    /** Serialized, with the core's error for anything that goes wrong; messages never contain a token. */
    private fun <T> guarded(key: String, call: () -> T): T = synchronized(lock) {
        if (!ACCOUNT_KEY.matches(key)) throw TokenStoreException.Failed("the account key is malformed")
        try {
            call()
        } catch (e: GeneralSecurityException) {
            throw TokenStoreException.Failed("the stored sign-in could not be decrypted (${e::class.simpleName}); sign in again")
        } catch (e: IOException) {
            throw TokenStoreException.Failed("the stored sign-in could not be read or written (${e.message})")
        } catch (e: JSONException) {
            throw TokenStoreException.Failed("the stored sign-in is damaged; sign in again")
        }
    }
}

private fun encode(tokens: SavedTokens): ByteArray = JSONObject()
    .put("access_token", tokens.accessToken)
    .put("refresh_token", tokens.refreshToken ?: JSONObject.NULL)
    .put("access_expires_at_ms", tokens.accessExpiresAtMs.toLong())
    .toString()
    .toByteArray()

private fun decode(bytes: ByteArray): SavedTokens {
    val json = JSONObject(String(bytes))
    return SavedTokens(
        accessToken = json.getString("access_token"),
        refreshToken = if (json.isNull("refresh_token")) null else json.getString("refresh_token"),
        accessExpiresAtMs = json.getLong("access_expires_at_ms").toULong(),
    )
}

private const val KEYSTORE = "AndroidKeyStore"
private const val KEY_ALIAS = "quest-tokens"

/**
 * The AES-256 key for [EncryptedTokenStore] in the Android Keystore: created on first use, never leaves the
 * Keystore. If it is lost (e.g. the user removed the screen lock on some devices), the stored sign-ins can no
 * longer be read and the core asks to sign in again; the data on the server is unaffected.
 */
object KeystoreKey {
    @Synchronized
    fun get(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val spec = KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE).apply { init(spec) }.generateKey()
    }
}
