package com.zenth_project.app

import android.app.Activity
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.spec.GCMParameterSpec

@TauriPlugin
class KeystorePlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        private const val KEY_ALIAS  = "zenth_persist_key"
        private const val PREFS_NAME = "zenth_keystore"
        private const val ENC_BLOB   = "enc_blob"
        private const val ENC_IV     = "enc_iv"
    }

    private fun getOrCreateKey(): javax.crypto.SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").also { it.load(null) }
        if (!ks.containsAlias(KEY_ALIAS)) {
            KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
                init(
                    KeyGenParameterSpec.Builder(
                        KEY_ALIAS,
                        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
                    )
                        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                        .setKeySize(256)
                        .build()
                )
                generateKey()
            }
        }
        return (ks.getEntry(KEY_ALIAS, null) as KeyStore.SecretKeyEntry).secretKey
    }

    @Command
    fun store(invoke: Invoke) {
        try {
            val args     = invoke.getArgs()
            val username = args.getString("username", null) ?: return invoke.reject("Missing username")
            val password = args.getString("password", null) ?: return invoke.reject("Missing password")

            // Store as JSON so any character is safe
            val blob      = JSONObject()
            blob.put("u", username)
            blob.put("p", password)
            val plaintext = blob.toString()

            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
            val encrypted = cipher.doFinal(plaintext.toByteArray(Charsets.UTF_8))

            activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE).edit()
                .putString(ENC_BLOB, Base64.encodeToString(encrypted, Base64.NO_WRAP))
                .putString(ENC_IV,   Base64.encodeToString(cipher.iv,  Base64.NO_WRAP))
                .apply()

            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }

    @Command
    fun retrieve(invoke: Invoke) {
        try {
            val prefs   = activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val encBlob = prefs.getString(ENC_BLOB, null)
            val encIv   = prefs.getString(ENC_IV,   null)

            if (encBlob == null || encIv == null) {
                return invoke.resolve(JSObject())  // empty → Rust gets None
            }

            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            val iv     = Base64.decode(encIv, Base64.NO_WRAP)
            cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(128, iv))

            val plaintext = String(
                cipher.doFinal(Base64.decode(encBlob, Base64.NO_WRAP)),
                Charsets.UTF_8
            )

            val parsed = JSONObject(plaintext)
            val result = JSObject()
            result.put("username", parsed.getString("u"))
            result.put("password", parsed.getString("p"))
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }

    @Command
    fun delete(invoke: Invoke) {
        try {
            activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE).edit()
                .remove(ENC_BLOB)
                .remove(ENC_IV)
                .apply()

            val ks = KeyStore.getInstance("AndroidKeyStore").also { it.load(null) }
            if (ks.containsAlias(KEY_ALIAS)) ks.deleteEntry(KEY_ALIAS)

            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }
}
