//! Puente JNI entre Rust y las clases Kotlin de Android.

use android_activity::AndroidApp;
use jni::objects::{GlobalRef, JByteArray, JByteBuffer, JObject, JString};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jint, jlong, jstring};
use jni::{JNIEnv, JavaVM};
use puente_core::wire::{CallInfo, MediaInfo, NotificationInfo, VideoPacket};
use puente_ui::PhoneActions;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::{
    report, BATTERY_UNRESTRICTED, FONT_SCALE, INPUT_ENABLED, NOTIFICATIONS_ENABLED, SAFE_BOTTOM, SAFE_TOP, ensure_engine, request_repaint,
    running_engine,
};

pub struct ActivityActions {
    vm: JavaVM,
    activity: GlobalRef,
}

impl ActivityActions {
    pub fn new(app: &AndroidApp) -> Option<Self> {
        // SAFETY: android-activity entrega punteros válidos a la VM y a la actividad
        // durante toda la vida de `android_main`.
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
        let activity = {
            let env = vm.attach_current_thread().ok()?;
            let object = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
            env.new_global_ref(object).ok()?
        };
        Some(Self { vm, activity })
    }

    fn call(&self, method: &str) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            log::error!("no se pudo acceder a la VM de Java");
            return;
        };
        if let Err(error) = env.call_method(&self.activity, method, "()V", &[]) {
            log::error!("falló la llamada a {method}: {error}");
        }
    }
}

impl PhoneActions for ActivityActions {
    fn start_capture(&self) {
        self.call("requestCapture");
    }

    fn stop_capture(&self) {
        self.call("stopCapture");
    }

    fn open_accessibility_settings(&self) {
        self.call("openAccessibilitySettings");
    }

    fn open_notification_settings(&self) {
        self.call("openNotificationSettings");
    }

    fn open_battery_settings(&self) {
        self.call("openBatterySettings");
    }

    fn scan_code(&self) {
        self.call("scanCode");
    }

    fn battery_unrestricted(&self) -> bool {
        BATTERY_UNRESTRICTED.load(Ordering::SeqCst)
    }

    fn safe_area_pixels(&self) -> (f32, f32) {
        (
            SAFE_TOP.load(Ordering::SeqCst) as f32,
            SAFE_BOTTOM.load(Ordering::SeqCst) as f32,
        )
    }

    fn font_scale(&self) -> f32 {
        f32::from_bits(FONT_SCALE.load(Ordering::SeqCst))
    }
}

fn copy_buffer(env: &JNIEnv, buffer: &JByteBuffer, offset: jint, size: jint) -> Option<Vec<u8>> {
    let offset = usize::try_from(offset).ok()?;
    let size = usize::try_from(size).ok()?;
    let capacity = env.get_direct_buffer_capacity(buffer).ok()?;
    if offset.checked_add(size)? > capacity {
        return None;
    }
    let address = env.get_direct_buffer_address(buffer).ok()?;
    // SAFETY: el rango se comprobó contra la capacidad del búfer directo, y Kotlin
    // no lo libera hasta que esta llamada termina.
    let bytes = unsafe { std::slice::from_raw_parts(address.add(offset), size) };
    Some(bytes.to_vec())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onVideoFrame(
    env: JNIEnv,
    _this: JObject,
    buffer: JByteBuffer,
    offset: jint,
    size: jint,
    flags: jint,
) -> jboolean {
    let Some(engine) = running_engine() else {
        return JNI_FALSE;
    };
    let Some(data) = copy_buffer(&env, &buffer, offset, size) else {
        return JNI_FALSE;
    };
    let packet = VideoPacket {
        flags: flags as u8 & (VideoPacket::KEY | VideoPacket::CONFIG),
        data,
    };
    if engine.send_video(packet) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onMirrorState(
    _env: JNIEnv,
    _this: JObject,
    active: jboolean,
) {
    if let Some(engine) = running_engine() {
        engine.set_mirroring(active != JNI_FALSE);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onInputState(
    _env: JNIEnv,
    _this: JObject,
    enabled: jboolean,
) {
    let enabled = enabled != JNI_FALSE;
    INPUT_ENABLED.store(enabled, Ordering::SeqCst);
    if let Some(engine) = running_engine() {
        engine.set_input_enabled(enabled);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_waitInput(
    env: JNIEnv,
    _this: JObject,
    timeout_milliseconds: jint,
) -> jstring {
    let wait = Duration::from_millis(u64::try_from(timeout_milliseconds).unwrap_or(0));
    let Some(engine) = running_engine() else {
        std::thread::sleep(wait);
        return std::ptr::null_mut();
    };
    engine
        .next_input(wait)
        .and_then(|input| env.new_string(input).ok())
        .map_or(std::ptr::null_mut(), |text| text.into_raw())
}

fn text(env: &mut JNIEnv, value: &JString) -> String {
    if value.is_null() {
        return String::new();
    }
    env.get_string(value).map(String::from).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_ensureEngine(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
) {
    let data_dir = PathBuf::from(text(&mut env, &data_dir));
    if let Err(error) = ensure_engine(data_dir) {
        log::error!("no se pudo iniciar el motor: {error}");
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_setSystemLanguage(mut env: JNIEnv, _this: JObject, tag: JString) {
    puente_ui::i18n::set_system_language(&text(&mut env, &tag));
    if let Some(engine) = running_engine() {
        puente_ui::i18n::apply(engine.view().language);
    }
}

/// Los textos de Kotlin siguen la misma elección que la interfaz en Rust.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_language(env: JNIEnv, _this: JObject) -> jstring {
    let language = match running_engine() {
        Some(engine) => puente_ui::i18n::resolve(engine.view().language),
        None => puente_ui::i18n::system_language(),
    };
    env.new_string(language.code())
        .map_or(std::ptr::null_mut(), |code| code.into_raw())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_waitTask(
    env: JNIEnv,
    _this: JObject,
    timeout_milliseconds: jint,
) -> jstring {
    let wait = Duration::from_millis(u64::try_from(timeout_milliseconds).unwrap_or(0));
    let Some(engine) = running_engine() else {
        std::thread::sleep(wait);
        return std::ptr::null_mut();
    };
    engine
        .next_task(wait)
        .and_then(|task| env.new_string(task).ok())
        .map_or(std::ptr::null_mut(), |task| task.into_raw())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onNotificationAccess(
    _env: JNIEnv,
    _this: JObject,
    enabled: jboolean,
) {
    let enabled = enabled != JNI_FALSE;
    NOTIFICATIONS_ENABLED.store(enabled, Ordering::SeqCst);
    if let Some(engine) = running_engine() {
        engine.set_notifications_enabled(enabled);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onNotification(
    mut env: JNIEnv,
    _this: JObject,
    key: JString,
    app: JString,
    title: JString,
    body: JString,
    can_reply: jboolean,
) {
    let info = NotificationInfo {
        key: text(&mut env, &key),
        app: text(&mut env, &app),
        title: text(&mut env, &title),
        text: text(&mut env, &body),
        can_reply: can_reply != JNI_FALSE,
    };
    if let Some(engine) = running_engine() {
        engine.post_notification(info);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onNotificationRemoved(
    mut env: JNIEnv,
    _this: JObject,
    key: JString,
) {
    let key = text(&mut env, &key);
    if let Some(engine) = running_engine() {
        engine.remove_notification(key);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onBattery(
    _env: JNIEnv,
    _this: JObject,
    percent: jint,
    charging: jboolean,
) {
    if let Some(engine) = running_engine() {
        engine.set_battery(percent.clamp(0, 100) as u8, charging != JNI_FALSE);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onMedia(
    mut env: JNIEnv,
    _this: JObject,
    title: JString,
    artist: JString,
    playing: jboolean,
) {
    let info = (!title.is_null()).then(|| MediaInfo {
        title: text(&mut env, &title),
        artist: text(&mut env, &artist),
        playing: playing != JNI_FALSE,
    });
    if let Some(engine) = running_engine() {
        engine.set_media(info);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onClipboard(
    mut env: JNIEnv,
    _this: JObject,
    value: JString,
) {
    let value = text(&mut env, &value);
    if let Some(engine) = running_engine() {
        engine.send_clipboard(value);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_beginFile(
    mut env: JNIEnv,
    _this: JObject,
    name: JString,
    size: jlong,
) -> jint {
    let name = text(&mut env, &name);
    let Ok(size) = u64::try_from(size) else {
        return -1;
    };
    running_engine()
        .and_then(|engine| engine.begin_file(&name, size))
        .and_then(|id| jint::try_from(id).ok())
        .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_fileChunk(
    env: JNIEnv,
    _this: JObject,
    id: jint,
    data: JByteArray,
) -> jboolean {
    let (Some(engine), Ok(id), Ok(data)) = (
        running_engine(),
        u32::try_from(id),
        env.convert_byte_array(&data),
    ) else {
        return JNI_FALSE;
    };
    if engine.send_file_chunk(id, data) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_endFile(
    _env: JNIEnv,
    _this: JObject,
    id: jint,
    success: jboolean,
) {
    if let (Some(engine), Ok(id)) = (running_engine(), u32::try_from(id)) {
        engine.end_file(id, success != JNI_FALSE);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onAudio(
    env: JNIEnv,
    _this: JObject,
    buffer: JByteBuffer,
    size: jint,
) {
    if let (Some(engine), Some(samples)) = (running_engine(), copy_buffer(&env, &buffer, 0, size)) {
        engine.send_audio(samples);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onDisplay(
    _env: JNIEnv,
    _this: JObject,
    top: jint,
    bottom: jint,
    font_scale: jni::sys::jfloat,
) {
    SAFE_TOP.store(u32::try_from(top).unwrap_or(0), Ordering::SeqCst);
    SAFE_BOTTOM.store(u32::try_from(bottom).unwrap_or(0), Ordering::SeqCst);
    FONT_SCALE.store(font_scale.to_bits(), Ordering::SeqCst);
    request_repaint();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onBatteryUnrestricted(
    _env: JNIEnv,
    _this: JObject,
    unrestricted: jboolean,
) {
    BATTERY_UNRESTRICTED.store(unrestricted != JNI_FALSE, Ordering::SeqCst);
    request_repaint();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_isLinked(_env: JNIEnv, _this: JObject) -> jboolean {
    if running_engine().is_some_and(|engine| engine.is_connected()) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_log(mut env: JNIEnv, _this: JObject, value: JString) {
    report(text(&mut env, &value));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onCall(
    mut env: JNIEnv,
    _this: JObject,
    key: JString,
    app: JString,
    caller: JString,
    can_answer: jboolean,
    can_decline: jboolean,
) {
    let call = CallInfo {
        key: text(&mut env, &key),
        app: text(&mut env, &app),
        caller: text(&mut env, &caller),
        can_answer: can_answer != JNI_FALSE,
        can_decline: can_decline != JNI_FALSE,
    };
    if let Some(engine) = running_engine() {
        engine.report_call(call);
    }
}

/// El texto de un código escaneado. Solo se atiende si es un enlace de Puente.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onScannedCode(mut env: JNIEnv, _this: JObject, value: JString) {
    let scanned = text(&mut env, &value);
    let Some(engine) = running_engine() else {
        return;
    };
    match puente_core::link::parse_connect_link(&scanned) {
        Some(ip) => {
            engine.set_manual_ip(Some(ip));
            request_repaint();
        }
        None => report("el código escaneado no es de Puente".to_string()),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onCallEnded(mut env: JNIEnv, _this: JObject, key: JString) {
    let key = text(&mut env, &key);
    if let Some(engine) = running_engine() {
        engine.end_call(key);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_puente_app_Native_onReplyResult(
    mut env: JNIEnv,
    _this: JObject,
    key: JString,
    delivered: jboolean,
) {
    let key = text(&mut env, &key);
    if let Some(engine) = running_engine() {
        engine.report_reply(key, delivered != JNI_FALSE);
    }
}
