/// Ouvre le share sheet Android natif pour partager du texte.
///
/// Sur Android : utilise JNI pour créer un Intent ACTION_SEND et lancer le chooser.
/// Sur les autres plateformes : retourne une erreur (le frontend doit gérer le fallback).

#[cfg(target_os = "android")]
#[tauri::command]
pub fn share_text(text: String) -> Result<(), String> {
    use jni::objects::{JObject, JValue};
    use ndk_context::android_context;

    let ctx = android_context();

    // Récupère la JavaVM depuis le contexte NDK
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| format!("JavaVM error: {e}"))?;

    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("JNI attach error: {e}"))?;

    // Intent intent = new Intent(Intent.ACTION_SEND)
    let intent_class = env
        .find_class("android/content/Intent")
        .map_err(|e| format!("find Intent class: {e}"))?;

    let action_send = env
        .new_string("android.intent.action.SEND")
        .map_err(|e| format!("new_string ACTION_SEND: {e}"))?;

    let intent = env
        .new_object(
            &intent_class,
            "(Ljava/lang/String;)V",
            &[JValue::Object(&action_send)],
        )
        .map_err(|e| format!("new Intent: {e}"))?;

    // intent.setType("text/plain")
    let mime = env
        .new_string("text/plain")
        .map_err(|e| format!("new_string mime: {e}"))?;

    env.call_method(
        &intent,
        "setType",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&mime)],
    )
    .map_err(|e| format!("setType: {e}"))?;

    // intent.putExtra(Intent.EXTRA_TEXT, text)
    let extra_key = env
        .new_string("android.intent.extra.TEXT")
        .map_err(|e| format!("new_string EXTRA_TEXT key: {e}"))?;

    let extra_val = env
        .new_string(&text)
        .map_err(|e| format!("new_string text: {e}"))?;

    env.call_method(
        &intent,
        "putExtra",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&extra_key), JValue::Object(&extra_val)],
    )
    .map_err(|e| format!("putExtra: {e}"))?;

    // Intent chooser = Intent.createChooser(intent, "Partager la clé publique")
    let title = env
        .new_string("Partager la clé publique")
        .map_err(|e| format!("new_string title: {e}"))?;

    let chooser = env
        .call_static_method(
            &intent_class,
            "createChooser",
            "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
            &[JValue::Object(&intent), JValue::Object(&title)],
        )
        .map_err(|e| format!("createChooser: {e}"))?
        .l()
        .map_err(|e| format!("chooser to object: {e}"))?;

    // activity.startActivity(chooser)
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    env.call_method(
        &activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&chooser)],
    )
    .map_err(|e| format!("startActivity: {e}"))?;

    Ok(())
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn share_text(_text: String) -> Result<(), String> {
    Err("share_not_supported".to_string())
}
