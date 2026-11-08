// Не показывать консоль на Windows в релизной сборке
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod catalog;
mod game;
mod instances;
mod loaders;

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const ASSETS_URL: &str = "https://resources.download.minecraft.net";

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

/// Папка лаунчера: %APPDATA%\.minelauncher, ~/.local/share/.minelauncher и т.д.
fn root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".minelauncher")
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn progress(app: &AppHandle, stage: &str, pct: f64) {
    let _ = app.emit("progress", json!({ "stage": stage, "pct": pct }));
}

/// Проверка правил Mojang (rules) для библиотек и аргументов
fn rules_allow(rules: Option<&Value>) -> bool {
    let Some(rules) = rules.and_then(|r| r.as_array()) else {
        return true;
    };
    let mut allowed = false;
    for r in rules {
        let is_allow = r["action"].as_str() == Some("allow");
        let mut matches = true;
        if let Some(os) = r.get("os") {
            if let Some(n) = os["name"].as_str() {
                if n != os_name() {
                    matches = false;
                }
            }
            if os["arch"].as_str() == Some("x86") && cfg!(target_pointer_width = "64") {
                matches = false;
            }
        }
        // demo-режим, пользовательское разрешение и т.п. — не включаем
        if r.get("features").is_some() {
            matches = false;
        }
        if matches {
            allowed = is_allow;
        }
    }
    allowed
}

async fn download(c: &reqwest::Client, url: &str, dest: &Path) -> Result<(), String> {
    if let Ok(m) = std::fs::metadata(dest) {
        if m.len() > 0 {
            return Ok(());
        }
    }
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(e)?;
    }
    let bytes = c
        .get(url)
        .send()
        .await
        .map_err(e)?
        .error_for_status()
        .map_err(e)?
        .bytes()
        .await
        .map_err(e)?;
    let tmp = dest.with_extension("part");
    std::fs::write(&tmp, &bytes).map_err(e)?;
    std::fs::rename(tmp, dest).map_err(e)
}

fn extract_natives(jar: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(e)?;
    let f = std::fs::File::open(jar).map_err(e)?;
    let mut z = zip::ZipArchive::new(f).map_err(e)?;
    for i in 0..z.len() {
        let mut file = z.by_index(i).map_err(e)?;
        let name = file.name().to_string();
        if file.is_dir() || name.starts_with("META-INF") {
            continue;
        }
        // берём только имя файла — защита от path traversal
        let Some(fname) = Path::new(&name).file_name() else {
            continue;
        };
        let mut out = std::fs::File::create(dir.join(fname)).map_err(e)?;
        std::io::copy(&mut file, &mut out).map_err(e)?;
    }
    Ok(())
}

fn subst(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{}}}", k), v);
    }
    out
}

fn collect_args(arr: &Value, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut res = vec![];
    let Some(items) = arr.as_array() else {
        return res;
    };
    for item in items {
        match item {
            Value::String(s) => res.push(subst(s, vars)),
            Value::Object(_) => {
                if !rules_allow(item.get("rules")) {
                    continue;
                }
                match &item["value"] {
                    Value::String(s) => res.push(subst(s, vars)),
                    Value::Array(a) => {
                        for x in a {
                            if let Some(s) = x.as_str() {
                                res.push(subst(s, vars));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    res
}

/// UUID офлайн-игрока — точно так же, как считает сам Minecraft
fn offline_uuid_of(name: &str) -> String {
    let mut h = md5::compute(format!("OfflinePlayer:{}", name)).0;
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    let hex: String = h.iter().map(|b| format!("{:02x}", b)).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[derive(Serialize)]
struct VersionItem {
    id: String,
    kind: String,
}

#[tauri::command]
fn offline_uuid(name: String) -> String {
    offline_uuid_of(name.trim())
}

#[tauri::command]
async fn get_versions(snapshots: bool) -> Result<Vec<VersionItem>, String> {
    let m: Value = reqwest::get(MANIFEST)
        .await
        .map_err(e)?
        .json()
        .await
        .map_err(e)?;
    let list = m["versions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| {
            let kind = v["type"].as_str()?.to_string();
            if kind == "release" || (snapshots && kind == "snapshot") {
                Some(VersionItem {
                    id: v["id"].as_str()?.to_string(),
                    kind,
                })
            } else {
                None
            }
        })
        .collect();
    Ok(list)
}

// ---------------------------------------------------------------------------
//  JAVA: автоматическая загрузка нужной версии от Mojang
// ---------------------------------------------------------------------------

const RUNTIME_INDEX: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

fn runtime_platform() -> &'static str {
    if cfg!(all(windows, target_arch = "aarch64")) {
        "windows-arm64"
    } else if cfg!(windows) {
        "windows-x64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "mac-os-arm64"
    } else if cfg!(target_os = "macos") {
        "mac-os"
    } else {
        "linux"
    }
}

fn java_rel_path() -> &'static str {
    if cfg!(windows) {
        "bin/java.exe"
    } else if cfg!(target_os = "macos") {
        "jre.bundle/Contents/Home/bin/java"
    } else {
        "bin/java"
    }
}

/// Скачивает Java (component: java-runtime-delta, jre-legacy и т.д.), если её ещё нет
async fn ensure_java(
    app: &AppHandle,
    c: &reqwest::Client,
    base: &Path,
    component: &str,
) -> Result<PathBuf, String> {
    let dir = base.join("runtime").join(component);
    let java_bin = dir.join(java_rel_path());
    let marker = dir.join(".ok");
    if marker.exists() && java_bin.exists() {
        return Ok(java_bin);
    }

    progress(app, "Скачиваю Java (один раз)…", 0.05);
    let index: Value = c
        .get(RUNTIME_INDEX)
        .send()
        .await
        .map_err(e)?
        .json()
        .await
        .map_err(e)?;
    let manifest_url = index[runtime_platform()][component][0]["manifest"]["url"]
        .as_str()
        .ok_or_else(|| format!("Для вашей системы нет Java «{}»", component))?
        .to_string();
    let manifest: Value = c
        .get(&manifest_url)
        .send()
        .await
        .map_err(e)?
        .json()
        .await
        .map_err(e)?;
    let files = manifest["files"]
        .as_object()
        .ok_or("Пустой список файлов Java")?;

    let mut jobs: Vec<(String, PathBuf)> = vec![];
    let mut execs: Vec<PathBuf> = vec![];
    let mut links: Vec<(PathBuf, String)> = vec![];
    for (rel, f) in files {
        // защита от выхода за пределы папки
        if rel.split('/').any(|p| p == "..") || rel.starts_with('/') {
            continue;
        }
        let dest = dir.join(rel);
        match f["type"].as_str() {
            Some("directory") => std::fs::create_dir_all(&dest).map_err(e)?,
            Some("file") => {
                if let Some(url) = f["downloads"]["raw"]["url"].as_str() {
                    jobs.push((url.to_string(), dest.clone()));
                    if f["executable"].as_bool() == Some(true) {
                        execs.push(dest);
                    }
                }
            }
            Some("link") => {
                if let Some(t) = f["target"].as_str() {
                    links.push((dest, t.to_string()));
                }
            }
            _ => {}
        }
    }

    let total = jobs.len();
    let done = Arc::new(AtomicUsize::new(0));
    let results: Vec<Result<(), String>> = futures_util::stream::iter(jobs)
        .map(|(url, path)| {
            let c = c.clone();
            let app = app.clone();
            let done = done.clone();
            async move {
                let r = download(&c, &url, &path).await;
                let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                if d % 10 == 0 || d == total {
                    progress(&app, &format!("Скачиваю Java: {} / {}", d, total), 0.05);
                }
                r
            }
        })
        .buffer_unordered(16)
        .collect()
        .await;
    if let Some(Err(err)) = results.into_iter().find(|r| r.is_err()) {
        return Err(format!("Ошибка загрузки Java: {}", err));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in &execs {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
        }
        for (dest, target) in &links {
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::os::unix::fs::symlink(target, dest);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (&execs, &links);
    }

    if !java_bin.exists() {
        return Err("Java скачалась, но файл запуска не найден".into());
    }
    std::fs::write(&marker, "ok").map_err(e)?;
    Ok(java_bin)
}

/// Открывает файл или ссылку средствами системы (только доверенные адреса)
fn open_target(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .creation_flags(0x08000000) // без чёрного окна
            .spawn()
            .map_err(e)?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(e)?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(e)?;
    }
    Ok(())
}

#[tauri::command]
fn open_site(page: String) -> Result<(), String> {
    let url = match page.as_str() {
        "ely_register" => "https://account.ely.by/register",
        "ely_skins" => "https://ely.by/skins",
        "ms_link" => "https://www.microsoft.com/link",
        "cf_console" => "https://console.curseforge.com/",
        "ms_azure" => {
            "https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade"
        }
        _ => return Err("Неизвестная ссылка".into()),
    };
    open_target(url)
}

/// Открывает папку данных лаунчера
#[tauri::command]
fn open_folder() -> Result<(), String> {
    let r = root();
    std::fs::create_dir_all(&r).map_err(e)?;
    open_target(&r.to_string_lossy())
}

// ---------------------------------------------------------------------------
//  Автообновление (через GitHub Releases, свой сервер не нужен)
// ---------------------------------------------------------------------------

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Спрашивает у GitHub, есть ли новая версия
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Value, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app
        .updater()
        .map_err(|_| "Обновления ещё не настроены (см. README)".to_string())?;
    match updater.check().await {
        Ok(Some(u)) => Ok(json!({
            "available": true,
            "version": u.version,
            "current": u.current_version,
            "notes": u.body.clone().unwrap_or_default()
        })),
        Ok(None) => Ok(json!({ "available": false })),
        Err(_) => Err("Не удалось проверить обновления. Проверьте интернет.".to_string()),
    }
}

/// Скачивает и ставит обновление, затем перезапускает лаунчер
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app
        .updater()
        .map_err(|_| "Обновления ещё не настроены".to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|_| "Не удалось проверить обновления".to_string())?
        .ok_or_else(|| "Новых версий нет".to_string())?;
    let app2 = app.clone();
    let mut got: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                got += chunk as u64;
                let pct = total.map(|t| got as f64 / t as f64).unwrap_or(0.5);
                progress(&app2, "Скачиваю обновление…", pct.min(0.99));
            },
            || {},
        )
        .await
        .map_err(|_| "Не удалось установить обновление".to_string())?;
    app.restart()
}

// ---------------------------------------------------------------------------
//  Окно, шрифты, картинки, выход
// ---------------------------------------------------------------------------

/// Масштаб интерфейса (1.0 = 100%)
#[tauri::command]
fn set_zoom(window: tauri::WebviewWindow, factor: f64) -> Result<(), String> {
    window.set_zoom(factor.clamp(0.5, 2.5)).map_err(e)
}

/// Размер окна лаунчера; 0 x 0 — развернуть на весь экран
#[tauri::command]
fn set_window_size(window: tauri::WebviewWindow, width: u32, height: u32) -> Result<(), String> {
    if width == 0 {
        let _ = window.unmaximize();
        return window.maximize().map_err(e);
    }
    let _ = window.unmaximize();
    window
        .set_size(tauri::LogicalSize::new(width as f64, height as f64))
        .map_err(e)?;
    window.center().map_err(e)
}

#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
}

#[tauri::command]
fn minimize_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(e)
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

/// Копирует файл в папку лаунчера (fonts / art) и возвращает его как data-URL
fn import_asset(folder: &str, path: &str, allowed: &[&str], max: u64) -> Result<Value, String> {
    use base64::Engine;
    let src = PathBuf::from(path.trim().trim_matches('"'));
    let ext = src
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !allowed.contains(&ext.as_str()) {
        return Err(format!("Подходят файлы: {}", allowed.join(", ")));
    }
    let meta = std::fs::metadata(&src).map_err(|_| "Файл не найден".to_string())?;
    if meta.len() > max {
        return Err(format!(
            "Файл слишком большой (максимум {} МБ)",
            max / 1048576
        ));
    }
    let name = src
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("file")
        .to_string();
    let dir = root().join(folder);
    std::fs::create_dir_all(&dir).map_err(e)?;
    let dest = dir.join(&name);
    if dest != src {
        std::fs::copy(&src, &dest).map_err(e)?;
    }
    let bytes = std::fs::read(&dest).map_err(e)?;
    Ok(json!({
        "name": name,
        "data": format!("data:{};base64,{}", mime_of(&dest), base64::engine::general_purpose::STANDARD.encode(bytes))
    }))
}

#[tauri::command]
fn install_font(path: String) -> Result<Value, String> {
    import_asset(
        "fonts",
        &path,
        &["ttf", "otf", "woff", "woff2"],
        15 * 1048576,
    )
}

#[tauri::command]
fn install_art(path: String) -> Result<Value, String> {
    import_asset(
        "art",
        &path,
        &["png", "jpg", "jpeg", "webp", "gif"],
        25 * 1048576,
    )
}

/// Возвращает ранее сохранённый файл из fonts / art
#[tauri::command]
fn load_asset(folder: String, name: String) -> Result<String, String> {
    use base64::Engine;
    if !(folder == "fonts" || folder == "art")
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return Err("Недопустимый путь".into());
    }
    let p = root().join(&folder).join(&name);
    let bytes = std::fs::read(&p).map_err(|_| "Файл не найден".to_string())?;
    Ok(format!(
        "data:{};base64,{}",
        mime_of(&p),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_versions,
            offline_uuid,
            open_site,
            app_version,
            check_update,
            install_update,
            set_zoom,
            set_window_size,
            open_devtools,
            minimize_window,
            exit_app,
            install_font,
            install_art,
            load_asset,
            open_folder,
            game::launch,
            instances::list_instances,
            instances::create_instance,
            instances::update_instance,
            instances::delete_instance,
            instances::duplicate_instance,
            instances::open_instance_folder,
            instances::instance_content,
            instances::content_toggle,
            instances::content_delete,
            instances::list_worlds,
            loaders::loader_versions,
            catalog::catalog_search,
            catalog::catalog_install,
            catalog::modpack_install,
            catalog::import_pack,
            auth::get_account,
            auth::logout,
            auth::ely_login,
            auth::ms_login,
            auth::skin_preview,
            auth::skin_upload_ms,
            auth::detect_java,
            auth::check_java
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения");
}
