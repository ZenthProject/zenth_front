package com.zenth_project.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.PermissionRequest
import android.webkit.WebChromeClient
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {

  private var pendingPermissionRequest: PermissionRequest? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    window.setFlags(
      WindowManager.LayoutParams.FLAG_SECURE,
      WindowManager.LayoutParams.FLAG_SECURE
    )

    WindowCompat.setDecorFitsSystemWindows(window, false)
    val controller = WindowInsetsControllerCompat(window, window.decorView)
    controller.hide(WindowInsetsCompat.Type.systemBars())
    controller.systemBarsBehavior =
      WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE

    window.decorView.post { disableAutofillRecursive(window.decorView) }

    requestPermissionsIfNeeded()
  }

  // Point d'entrée Tauri pour configurer le WebView avant qu'il soit affiché
  override fun onWebViewCreate(webView: WebView) {
    // Désactive toute forme de suggestion/complétion de mot de passe
    @Suppress("DEPRECATION")
    webView.settings.savePassword = false
    @Suppress("DEPRECATION")
    webView.settings.saveFormData = false
    webView.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS

    webView.webChromeClient = object : WebChromeClient() {
      override fun onPermissionRequest(request: PermissionRequest) {
        val audioGranted = ContextCompat.checkSelfPermission(
          this@MainActivity, Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED

        if (audioGranted) {
          request.grant(request.resources)
        } else {
          pendingPermissionRequest = request
          ActivityCompat.requestPermissions(
            this@MainActivity,
            arrayOf(Manifest.permission.RECORD_AUDIO, Manifest.permission.MODIFY_AUDIO_SETTINGS),
            1002
          )
        }
      }
    }
    super.onWebViewCreate(webView)
  }

  override fun onRequestPermissionsResult(
    requestCode: Int,
    permissions: Array<String>,
    grantResults: IntArray
  ) {
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)

    if (requestCode == 1002) {
      val granted = grantResults.isNotEmpty() &&
        grantResults[0] == PackageManager.PERMISSION_GRANTED
      if (granted) {
        pendingPermissionRequest?.grant(pendingPermissionRequest?.resources ?: arrayOf())
      } else {
        pendingPermissionRequest?.deny()
      }
      pendingPermissionRequest = null
    }
  }

  private fun requestPermissionsIfNeeded() {
    val permissions = mutableListOf<String>()

    val photoPermission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      Manifest.permission.READ_MEDIA_IMAGES
    } else {
      Manifest.permission.READ_EXTERNAL_STORAGE
    }
    if (ContextCompat.checkSelfPermission(this, photoPermission) != PackageManager.PERMISSION_GRANTED) {
      permissions.add(photoPermission)
    }
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
      permissions.add(Manifest.permission.RECORD_AUDIO)
    }
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.MODIFY_AUDIO_SETTINGS) != PackageManager.PERMISSION_GRANTED) {
      permissions.add(Manifest.permission.MODIFY_AUDIO_SETTINGS)
    }
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
      permissions.add(Manifest.permission.CAMERA)
    }

    if (permissions.isNotEmpty()) {
      ActivityCompat.requestPermissions(this, permissions.toTypedArray(), 1001)
    }
  }

  override fun onResume() {
    super.onResume()
    val controller = WindowInsetsControllerCompat(window, window.decorView)
    controller.hide(WindowInsetsCompat.Type.systemBars())
    controller.systemBarsBehavior =
      WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    window.decorView.post { disableAutofillRecursive(window.decorView) }
  }

  private fun disableAutofillRecursive(view: View) {
    view.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
    if (view is WebView) {
      @Suppress("DEPRECATION")
      view.settings.saveFormData = false
    }
    if (view is ViewGroup) {
      for (i in 0 until view.childCount) {
        disableAutofillRecursive(view.getChildAt(i))
      }
    }
  }
}
