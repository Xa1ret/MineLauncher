//! Дополнительные возможности: конфиги, логи и краши, статистика, ссылки, скины

use crate::instances::{dir_of, safe_id};
use crate::{e, root};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
//  Открыть сайт в браузере (только разрешённые адреса)
// ---------------------------------------------------------------------------

const SITES: [&str; 4] = [
    "https://chatgpt.com/",
    "https://claude.ai/new",
    "https://www.kimi.com/",
    "https://www.google.com/",
];

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !SITES.contains(&url.as_str()) {
        return Err("Этот адрес открыть нельзя".to_string());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(e)
}

// ---------------------------------------------------------------------------
//  Редактор конфигов
// ---------------------------------------------------------------------------

const CFG_EXT: [&str; 11] = [
    "json",
    "json5",
    "toml",
    "cfg",
    "conf",
    "properties",
    "txt",
    "yml",
    "yaml",
    "ini",
    "snbt",
];
const CFG_MAX: u64 = 1024 * 1024;

fn cfg_dir(id: &str) -> Result<PathBuf, String> {
    safe_id(id)?;
    Ok(dir_of(id).join("config"))
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<Value>, depth: usize) {
    if depth > 4 || out.len() > 800 {
        return;
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for en in rd.flatten() {
            let p = en.path();
            if p.is_dir() {
                walk(&p, base, out, depth + 1);
            } else if let Ok(m) = en.metadata() {
                let ext = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if CFG_EXT.contains(&ext.as_str()) && m.len() <= CFG_MAX {
                    if let Ok(rel) = p.strip_prefix(base) {
                        out.push(json!({
                            "path": rel.to_string_lossy().replace('\\', "/"),
                            "size": m.len()
                        }));
                    }
                }
            }
        }
    }
}

#[tauri::command]
pub fn config_list(instance_id: String) -> Result<Vec<Value>, String> {
    let base = cfg_dir(&instance_id)?;
    let mut out = vec![];
    // options.txt лежит рядом с папкой config
    let opt = dir_of(&instance_id).join("options.txt");
    if opt.is_file() {
        out.push(json!({ "path": "../options.txt", "size": opt.metadata().map(|m| m.len()).unwrap_or(0) }));
    }
    walk(&base, &base, &mut out, 0);
    out.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b["path"].as_str().unwrap_or("").to_lowercase())
    });
    Ok(out)
}

fn cfg_path(id: &str, rel: &str) -> Result<PathBuf, String> {
    let base = cfg_dir(id)?;
    if rel == "../options.txt" {
        return Ok(dir_of(id).join("options.txt"));
    }
    if rel.is_empty() || rel.contains("..") || rel.starts_with('/') || rel.contains(':') {
        return Err("Некорректный путь".to_string());
    }
    let p = base.join(rel);
    let ext = p
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !CFG_EXT.contains(&ext.as_str()) {
        return Err("Этот тип файла править нельзя".to_string());
    }
    Ok(p)
}

#[tauri::command]
pub fn config_read(instance_id: String, path: String) -> Result<String, String> {
    let p = cfg_path(&instance_id, &path)?;
    if p.metadata().map(|m| m.len()).unwrap_or(0) > CFG_MAX {
        return Err("Файл слишком большой".to_string());
    }
    std::fs::read_to_string(&p).map_err(|_| "Не удалось прочитать файл (это не текст?)".to_string())
}

#[tauri::command]
pub fn config_write(instance_id: String, path: String, content: String) -> Result<(), String> {
    let p = cfg_path(&instance_id, &path)?;
    if !p.is_file() {
        return Err("Файл не найден".to_string());
    }
    if content.len() as u64 > CFG_MAX {
        return Err("Файл слишком большой".to_string());
    }
    // запасная копия на случай ошибки
    let bak = p.with_extension(format!(
        "{}.bak",
        p.extension().and_then(|x| x.to_str()).unwrap_or("")
    ));
    let _ = std::fs::copy(&p, &bak);
    std::fs::write(&p, content).map_err(e)
}

// ---------------------------------------------------------------------------
//  Логи и краши
// ---------------------------------------------------------------------------

fn tail(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

fn read_lossy(p: &Path) -> String {
    std::fs::read(p)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default()
}

/// Свежий отчёт о крахе и хвост лога сборки
#[tauri::command]
pub fn crash_info(instance_id: String) -> Result<Value, String> {
    safe_id(&instance_id)?;
    let d = dir_of(&instance_id);
    let mut crash: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(d.join("crash-reports")) {
        for en in rd.flatten() {
            if let Ok(t) = en.metadata().and_then(|m| m.modified()) {
                if crash.as_ref().map(|c| t > c.0).unwrap_or(true) {
                    crash = Some((t, en.path()));
                }
            }
        }
    }
    let crash_text = crash
        .as_ref()
        .map(|c| tail(&read_lossy(&c.1), 40_000))
        .unwrap_or_default();
    let crash_age = crash
        .as_ref()
        .and_then(|c| c.0.elapsed().ok())
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);
    let crash_name = crash
        .as_ref()
        .and_then(|c| c.1.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();
    let log = tail(&read_lossy(&d.join("logs").join("latest.log")), 30_000);
    let launcher_log = tail(&read_lossy(&d.join("launcher_log.txt")), 20_000);
    Ok(json!({
        "crash_name": crash_name,
        "crash_age": crash_age,
        "crash": crash_text,
        "log": log,
        "launcher_log": launcher_log
    }))
}

#[tauri::command]
pub fn open_crash_folder(instance_id: String) -> Result<(), String> {
    safe_id(&instance_id)?;
    let p = dir_of(&instance_id).join("crash-reports");
    std::fs::create_dir_all(&p).map_err(e)?;
    crate::instances::open_instance_folder(instance_id, Some("crash-reports".to_string()))
}

// ---------------------------------------------------------------------------
//  Статистика по дням
// ---------------------------------------------------------------------------

fn stats_path() -> PathBuf {
    root().join("stats.json")
}

fn today() -> String {
    // дата по UTC, этого достаточно для графика
    let days = crate::instances::now() / 86400;
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

pub fn stats_add(_instance_id: &str, secs: u64) {
    let mut v: Value = std::fs::read_to_string(stats_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({ "days": {}, "launches": {} }));
    let day = today();
    let cur = v["days"][&day].as_u64().unwrap_or(0);
    v["days"][&day] = json!(cur + secs);
    let _ = std::fs::write(stats_path(), v.to_string());
}

pub fn stats_launch(instance_id: &str) {
    let mut v: Value = std::fs::read_to_string(stats_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({ "days": {}, "launches": {} }));
    let cur = v["launches"][instance_id].as_u64().unwrap_or(0);
    v["launches"][instance_id] = json!(cur + 1);
    let _ = std::fs::write(stats_path(), v.to_string());
}

#[tauri::command]
pub fn stats_get() -> Value {
    std::fs::read_to_string(stats_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({ "days": {}, "launches": {} }))
}

// ---------------------------------------------------------------------------
//  Библиотека своих скинов
// ---------------------------------------------------------------------------

fn skins_dir() -> PathBuf {
    root().join("skins")
}

fn skin_name_ok(n: &str) -> Result<(), String> {
    if n.is_empty()
        || n.contains('/')
        || n.contains('\\')
        || n.contains("..")
        || !n.to_lowercase().ends_with(".png")
    {
        return Err("Недопустимое имя файла".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn skin_lib_list() -> Result<Vec<Value>, String> {
    use base64::Engine;
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(skins_dir()) {
        for en in rd.flatten() {
            let name = en.file_name().to_string_lossy().to_string();
            if skin_name_ok(&name).is_err() {
                continue;
            }
            if let Ok(b) = std::fs::read(en.path()) {
                if b.len() < 200_000 {
                    out.push(json!({
                        "name": name,
                        "data": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
                    }));
                }
            }
        }
    }
    out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(out)
}

#[tauri::command]
pub fn skin_lib_add(path: String) -> Result<String, String> {
    let src = PathBuf::from(&path);
    let name = src
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("skin.png")
        .to_string();
    skin_name_ok(&name)?;
    // проверка, что это настоящий скин нужного размера
    crate::auth::skin_preview(path)?;
    std::fs::create_dir_all(skins_dir()).map_err(e)?;
    std::fs::copy(&src, skins_dir().join(&name)).map_err(e)?;
    Ok(name)
}

#[tauri::command]
pub fn skin_lib_delete(name: String) -> Result<(), String> {
    skin_name_ok(&name)?;
    std::fs::remove_file(skins_dir().join(name)).map_err(e)
}

#[tauri::command]
pub async fn skin_lib_apply(name: String, slim: bool) -> Result<(), String> {
    skin_name_ok(&name)?;
    let p = skins_dir().join(&name);
    crate::auth::skin_upload_ms(p.to_string_lossy().to_string(), slim).await
}
