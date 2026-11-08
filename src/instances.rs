//! Сборки (instances): у каждой своя папка, свои моды и настройки.

use crate::{e, root};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn d_ram() -> u32 {
    4096
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    /// vanilla | fabric | quilt | forge | neoforge
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    /// id установленного профиля загрузчика (папка в versions)
    #[serde(default)]
    pub profile_id: String,
    #[serde(default = "d_ram")]
    pub ram_mb: u32,
    #[serde(default)]
    pub jvm_args: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub last_played: u64,
    #[serde(default)]
    pub play_secs: u64,
    #[serde(default)]
    pub icon: String,
}

pub fn safe_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err("Некорректный id сборки".to_string())
    }
}

pub fn safe_file(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        Err("Некорректное имя файла".to_string())
    } else {
        Ok(())
    }
}

pub fn dir_of(id: &str) -> PathBuf {
    root().join("instances").join(id)
}

pub fn load_instance(id: &str) -> Result<Instance, String> {
    safe_id(id)?;
    let s = std::fs::read_to_string(dir_of(id).join("instance.json"))
        .map_err(|_| "Сборка не найдена".to_string())?;
    serde_json::from_str(&s).map_err(e)
}

pub fn save_instance(i: &Instance) -> Result<(), String> {
    safe_id(&i.id)?;
    let d = dir_of(&i.id);
    std::fs::create_dir_all(&d).map_err(e)?;
    let text = serde_json::to_string_pretty(i).map_err(e)?;
    std::fs::write(d.join("instance.json"), text).map_err(e)
}

fn make_id(name: &str) -> String {
    let slug: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>()
        .to_lowercase();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let h = md5::compute(format!("{}{}", name, nanos)).0;
    let hex: String = h.iter().take(3).map(|b| format!("{:02x}", b)).collect();
    if slug.is_empty() {
        format!("pack-{}", hex)
    } else {
        format!("{}-{}", slug, hex)
    }
}

pub fn create(
    name: &str,
    mc_version: &str,
    loader: &str,
    loader_version: &str,
    icon: &str,
) -> Result<Instance, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Введите название сборки".to_string());
    }
    if mc_version.trim().is_empty() {
        return Err("Выберите версию игры".to_string());
    }
    let inst = Instance {
        id: make_id(name),
        name: name.to_string(),
        mc_version: mc_version.trim().to_string(),
        loader: loader.to_string(),
        loader_version: loader_version.to_string(),
        profile_id: String::new(),
        ram_mb: 4096,
        jvm_args: String::new(),
        created: now(),
        last_played: 0,
        play_secs: 0,
        icon: icon.to_string(),
    };
    save_instance(&inst)?;
    for sub in ["mods", "resourcepacks", "shaderpacks", "saves"] {
        let _ = std::fs::create_dir_all(dir_of(&inst.id).join(sub));
    }
    Ok(inst)
}

pub fn touch_played(id: &str) {
    if let Ok(mut i) = load_instance(id) {
        i.last_played = now();
        let _ = save_instance(&i);
    }
}

pub fn add_playtime(id: &str, secs: u64) {
    if let Ok(mut i) = load_instance(id) {
        i.last_played = now();
        i.play_secs += secs;
        let _ = save_instance(&i);
    }
}

// ---------------------------------------------------------------------------
//  Команды для интерфейса
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_instances() -> Vec<Instance> {
    let mut v: Vec<Instance> = vec![];
    if let Ok(rd) = std::fs::read_dir(root().join("instances")) {
        for d in rd.flatten() {
            let name = d.file_name().to_string_lossy().to_string();
            if let Ok(i) = load_instance(&name) {
                v.push(i);
            }
        }
    }
    v.sort_by(|a, b| {
        let ka = a.last_played.max(a.created);
        let kb = b.last_played.max(b.created);
        kb.cmp(&ka)
    });
    v
}

#[tauri::command]
pub fn create_instance(
    name: String,
    mc_version: String,
    loader: String,
    loader_version: String,
) -> Result<Instance, String> {
    let ok = ["vanilla", "fabric", "quilt", "forge", "neoforge"];
    if !ok.contains(&loader.as_str()) {
        return Err("Неизвестный загрузчик".to_string());
    }
    create(&name, &mc_version, &loader, &loader_version, "")
}

#[tauri::command]
pub fn update_instance(
    id: String,
    name: Option<String>,
    ram_mb: Option<u32>,
    jvm_args: Option<String>,
) -> Result<Instance, String> {
    let mut i = load_instance(&id)?;
    if let Some(n) = name {
        if !n.trim().is_empty() {
            i.name = n.trim().to_string();
        }
    }
    if let Some(r) = ram_mb {
        i.ram_mb = r.clamp(1024, 65536);
    }
    if let Some(a) = jvm_args {
        i.jvm_args = a.trim().to_string();
    }
    save_instance(&i)?;
    Ok(i)
}

#[tauri::command]
pub fn delete_instance(id: String) -> Result<(), String> {
    safe_id(&id)?;
    let d = dir_of(&id);
    if d.is_dir() {
        std::fs::remove_dir_all(d).map_err(e)?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn duplicate_instance(id: String) -> Result<Instance, String> {
    let src = load_instance(&id)?;
    let new_id = make_id(&src.name);
    let from = dir_of(&id);
    let to = dir_of(&new_id);
    tokio::task::spawn_blocking(move || copy_dir(&from, &to))
        .await
        .map_err(e)?
        .map_err(e)?;
    let mut copy = src;
    copy.id = new_id;
    copy.name = format!("{} (копия)", copy.name);
    copy.created = now();
    copy.last_played = 0;
    copy.play_secs = 0;
    save_instance(&copy)?;
    Ok(copy)
}

#[tauri::command]
pub fn open_instance_folder(id: String, sub: Option<String>) -> Result<(), String> {
    safe_id(&id)?;
    let mut p = dir_of(&id);
    if let Some(s) = sub {
        let allowed = [
            "mods",
            "resourcepacks",
            "shaderpacks",
            "saves",
            "screenshots",
            "config",
        ];
        if allowed.contains(&s.as_str()) {
            p = p.join(s);
        }
    }
    std::fs::create_dir_all(&p).map_err(e)?;
    crate::open_target(&p.to_string_lossy())
}

// ---------------------------------------------------------------------------
//  Содержимое сборки (моды, ресурспаки, шейдеры, миры)
// ---------------------------------------------------------------------------

fn kind_dir(kind: &str) -> Result<&'static str, String> {
    match kind {
        "mod" => Ok("mods"),
        "resourcepack" => Ok("resourcepacks"),
        "shader" => Ok("shaderpacks"),
        "world" => Ok("saves"),
        _ => Err("Неизвестный тип".to_string()),
    }
}

fn records_path(id: &str) -> PathBuf {
    dir_of(id).join("minelauncher.content.json")
}

fn read_records(id: &str) -> Vec<Value> {
    std::fs::read_to_string(records_path(id))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok())
        .unwrap_or_default()
}

/// Запоминаем, что и откуда поставили (чтобы показать название и значок)
pub fn add_record(id: &str, rec: Value) {
    let mut all = read_records(id);
    let fname = rec["file_name"].as_str().unwrap_or("").to_string();
    all.retain(|r| r["file_name"].as_str() != Some(fname.as_str()));
    all.push(rec);
    let _ = std::fs::write(
        records_path(id),
        serde_json::to_string_pretty(&all).unwrap_or_default(),
    );
}

fn remove_record(id: &str, file_name: &str) {
    let mut all = read_records(id);
    all.retain(|r| r["file_name"].as_str() != Some(file_name));
    let _ = std::fs::write(
        records_path(id),
        serde_json::to_string_pretty(&all).unwrap_or_default(),
    );
}

#[derive(Serialize)]
pub struct LocalItem {
    file_name: String,
    enabled: bool,
    size: u64,
    title: String,
    icon: String,
    source: String,
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for en in rd.flatten() {
            if let Ok(m) = en.metadata() {
                if m.is_dir() {
                    total += dir_size(&en.path());
                } else {
                    total += m.len();
                }
            }
        }
    }
    total
}

#[tauri::command]
pub fn instance_content(id: String, kind: String) -> Result<Vec<LocalItem>, String> {
    safe_id(&id)?;
    let dir = dir_of(&id).join(kind_dir(&kind)?);
    let records = read_records(&id);
    let mut out: Vec<LocalItem> = vec![];
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for en in rd.flatten() {
            let name = en.file_name().to_string_lossy().to_string();
            let is_dir = en.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if kind == "world" && !is_dir {
                continue;
            }
            let base = name.trim_end_matches(".disabled").to_string();
            let rec = records
                .iter()
                .find(|r| r["file_name"].as_str() == Some(base.as_str()));
            let size = if is_dir {
                dir_size(&en.path())
            } else {
                en.metadata().map(|m| m.len()).unwrap_or(0)
            };
            out.push(LocalItem {
                enabled: !name.ends_with(".disabled"),
                size,
                title: rec
                    .and_then(|r| r["title"].as_str())
                    .unwrap_or(base.as_str())
                    .to_string(),
                icon: rec
                    .and_then(|r| r["icon"].as_str())
                    .unwrap_or("")
                    .to_string(),
                source: rec
                    .and_then(|r| r["source"].as_str())
                    .unwrap_or("")
                    .to_string(),
                file_name: name,
            });
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn content_toggle(id: String, kind: String, file_name: String) -> Result<(), String> {
    safe_id(&id)?;
    crate::instances::safe_file(&file_name)?;
    if kind == "world" {
        return Err("Миры нельзя отключать".to_string());
    }
    let dir = dir_of(&id).join(kind_dir(&kind)?);
    let from = dir.join(&file_name);
    let to = if let Some(base) = file_name.strip_suffix(".disabled") {
        dir.join(base)
    } else {
        dir.join(format!("{}.disabled", file_name))
    };
    std::fs::rename(from, to).map_err(e)
}

#[tauri::command]
pub fn content_delete(id: String, kind: String, file_name: String) -> Result<(), String> {
    safe_id(&id)?;
    safe_file(&file_name)?;
    let p = dir_of(&id).join(kind_dir(&kind)?).join(&file_name);
    if p.is_dir() {
        std::fs::remove_dir_all(&p).map_err(e)?;
    } else if p.is_file() {
        std::fs::remove_file(&p).map_err(e)?;
    }
    remove_record(&id, file_name.trim_end_matches(".disabled"));
    Ok(())
}

#[tauri::command]
pub fn list_worlds(id: String) -> Result<Vec<String>, String> {
    safe_id(&id)?;
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(dir_of(&id).join("saves")) {
        for en in rd.flatten() {
            if en.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                out.push(en.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

#[allow(dead_code)]
fn _unused() -> Value {
    json!({})
}
