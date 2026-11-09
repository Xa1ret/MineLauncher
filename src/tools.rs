//! Ремонт сборки, импорт из других лаунчеров, музыка лаунчера

use crate::instances::{self, copy_dir, dir_of, safe_id};
use crate::{e, root};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
//  Ремонт сборки: проверка файлов игры по контрольным суммам
// ---------------------------------------------------------------------------

fn sha1_file(p: &Path) -> Option<String> {
    use sha1::{Digest, Sha1};
    let b = std::fs::read(p).ok()?;
    let mut h = Sha1::new();
    h.update(&b);
    Some(h.finalize().iter().map(|x| format!("{:02x}", x)).collect())
}

fn check_file(p: &Path, want: &str, checked: &mut u32, fixed: &mut u32) {
    if !p.exists() {
        return;
    }
    *checked += 1;
    if sha1_file(p).as_deref() != Some(want) {
        let _ = std::fs::remove_file(p);
        *fixed += 1;
    }
}

fn collect_libs(v: &Value, base: &Path, checked: &mut u32, fixed: &mut u32) {
    for lib in v["libraries"].as_array().cloned().unwrap_or_default() {
        let a = &lib["downloads"]["artifact"];
        if let (Some(path), Some(sha)) = (a["path"].as_str(), a["sha1"].as_str()) {
            check_file(&base.join("libraries").join(path), sha, checked, fixed);
        }
        if let Some(cls) = lib["downloads"]["classifiers"].as_object() {
            for (_, c) in cls {
                if let (Some(path), Some(sha)) = (c["path"].as_str(), c["sha1"].as_str()) {
                    check_file(&base.join("libraries").join(path), sha, checked, fixed);
                }
            }
        }
    }
}

fn remove_junk(dir: &Path, fixed: &mut u32, depth: usize) {
    if depth > 8 {
        return;
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for en in rd.flatten() {
            let p = en.path();
            if p.is_dir() {
                remove_junk(&p, fixed, depth + 1);
            } else if let Ok(m) = en.metadata() {
                let part = p.extension().and_then(|x| x.to_str()) == Some("part");
                if part || m.len() == 0 {
                    let _ = std::fs::remove_file(&p);
                    *fixed += 1;
                }
            }
        }
    }
}

#[tauri::command]
pub async fn repair_instance(instance_id: String) -> Result<Value, String> {
    let mut inst = instances::load_instance(&instance_id)?;
    tokio::task::spawn_blocking(move || {
        let base = root();
        let (mut checked, mut fixed) = (0u32, 0u32);
        let mut reinstall_loader = false;

        // ванильная версия: клиент, библиотеки, ассеты
        let vp = base
            .join("versions")
            .join(&inst.mc_version)
            .join(format!("{}.json", inst.mc_version));
        if let Ok(text) = std::fs::read_to_string(&vp) {
            match serde_json::from_str::<Value>(&text) {
                Ok(v) => {
                    if let Some(sha) = v["downloads"]["client"]["sha1"].as_str() {
                        check_file(
                            &base
                                .join("versions")
                                .join(&inst.mc_version)
                                .join(format!("{}.jar", inst.mc_version)),
                            sha,
                            &mut checked,
                            &mut fixed,
                        );
                    }
                    collect_libs(&v, &base, &mut checked, &mut fixed);
                    if let Some(id) = v["assetIndex"]["id"].as_str() {
                        let ip = base
                            .join("assets")
                            .join("indexes")
                            .join(format!("{}.json", id));
                        if let Some(idx) = std::fs::read_to_string(&ip)
                            .ok()
                            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                        {
                            if let Some(objs) = idx["objects"].as_object() {
                                for (_, o) in objs {
                                    if let Some(h) = o["hash"].as_str() {
                                        if h.len() > 2 {
                                            check_file(
                                                &base
                                                    .join("assets")
                                                    .join("objects")
                                                    .join(&h[..2])
                                                    .join(h),
                                                h,
                                                &mut checked,
                                                &mut fixed,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // повреждён сам json версии, скачается заново
                    let _ = std::fs::remove_file(&vp);
                    fixed += 1;
                }
            }
        }

        // профиль загрузчика
        if !inst.profile_id.is_empty() {
            let pp = base
                .join("versions")
                .join(&inst.profile_id)
                .join(format!("{}.json", inst.profile_id));
            match std::fs::read_to_string(&pp)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            {
                Some(v) => collect_libs(&v, &base, &mut checked, &mut fixed),
                None => reinstall_loader = true,
            }
        }
        if reinstall_loader {
            let _ = std::fs::remove_dir_all(base.join("versions").join(&inst.profile_id));
            inst.profile_id = String::new();
            let _ = instances::save_instance(&inst);
            fixed += 1;
        }

        // обрывки загрузок и пустые файлы, старые распакованные natives
        remove_junk(&base.join("libraries"), &mut fixed, 0);
        remove_junk(&base.join("assets"), &mut fixed, 0);
        let _ =
            std::fs::remove_dir_all(base.join("versions").join(&inst.mc_version).join("natives"));
        if !inst.profile_id.is_empty() {
            let _ = std::fs::remove_dir_all(
                base.join("versions").join(&inst.profile_id).join("natives"),
            );
        }
        Ok::<Value, String>(
            json!({ "checked": checked, "fixed": fixed, "loader": reinstall_loader }),
        )
    })
    .await
    .map_err(e)?
}

// ---------------------------------------------------------------------------
//  Импорт из других лаунчеров
// ---------------------------------------------------------------------------

fn count_entries(p: &Path) -> usize {
    std::fs::read_dir(p).map(|r| r.count()).unwrap_or(0)
}

fn game_dir(root_dir: &Path) -> Option<PathBuf> {
    for cand in [
        root_dir.to_path_buf(),
        root_dir.join("minecraft"),
        root_dir.join(".minecraft"),
    ] {
        if cand.join("mods").is_dir()
            || cand.join("saves").is_dir()
            || cand.join("options.txt").is_file()
        {
            return Some(cand);
        }
    }
    None
}

/// Разбирает имя версии из папки versions обычного .minecraft
fn parse_version_id(id: &str) -> Option<Value> {
    if let Some(r) = id.strip_prefix("fabric-loader-") {
        let (lv, mc) = r.split_once('-')?;
        return Some(json!({"mc": mc, "loader": "fabric", "loader_version": lv}));
    }
    if let Some(r) = id.strip_prefix("quilt-loader-") {
        let (lv, mc) = r.split_once('-')?;
        return Some(json!({"mc": mc, "loader": "quilt", "loader_version": lv}));
    }
    if let Some((mc, lv)) = id.split_once("-forge-") {
        return Some(json!({"mc": mc, "loader": "forge", "loader_version": lv}));
    }
    if let Some(lv) = id.strip_prefix("neoforge-") {
        let parts: Vec<&str> = lv.split('.').collect();
        if parts.len() >= 2 {
            let mc = if parts[1] == "0" {
                format!("1.{}", parts[0])
            } else {
                format!("1.{}.{}", parts[0], parts[1])
            };
            return Some(json!({"mc": mc, "loader": "neoforge", "loader_version": lv}));
        }
        return None;
    }
    if id
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
        && id.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return Some(json!({"mc": id, "loader": "vanilla", "loader_version": ""}));
    }
    None
}

fn prism_detect(dir: &Path) -> Vec<Value> {
    let Some(v) = std::fs::read_to_string(dir.join("mmc-pack.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return vec![];
    };
    let (mut mc, mut loader, mut lv) = (String::new(), "vanilla", String::new());
    for c in v["components"].as_array().cloned().unwrap_or_default() {
        let uid = c["uid"].as_str().unwrap_or("");
        let ver = c["version"].as_str().unwrap_or("").to_string();
        match uid {
            "net.minecraft" => mc = ver,
            "net.fabricmc.fabric-loader" => {
                loader = "fabric";
                lv = ver
            }
            "org.quiltmc.quilt-loader" => {
                loader = "quilt";
                lv = ver
            }
            "net.minecraftforge" => {
                loader = "forge";
                lv = ver
            }
            "net.neoforged" => {
                loader = "neoforge";
                lv = ver
            }
            _ => {}
        }
    }
    if mc.is_empty() {
        return vec![];
    }
    vec![json!({"mc": mc, "loader": loader, "loader_version": lv})]
}

fn curse_detect(dir: &Path) -> Vec<Value> {
    let Some(v) = std::fs::read_to_string(dir.join("minecraftinstance.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return vec![];
    };
    let mc = v["gameVersion"].as_str().unwrap_or("").to_string();
    let name = v["baseModLoader"]["name"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    if mc.is_empty() {
        return vec![];
    }
    let (loader, lv) = if let Some(x) = name.strip_prefix("fabric-") {
        ("fabric", x.to_string())
    } else if let Some(x) = name.strip_prefix("forge-") {
        ("forge", x.to_string())
    } else if let Some(x) = name.strip_prefix("neoforge-") {
        ("neoforge", x.to_string())
    } else if let Some(x) = name.strip_prefix("quilt-") {
        ("quilt", x.to_string())
    } else {
        ("vanilla", String::new())
    };
    vec![json!({"mc": mc, "loader": loader, "loader_version": lv})]
}

fn push_candidate(out: &mut Vec<Value>, launcher: &str, name: String, dir: &Path, det: Vec<Value>) {
    let Some(g) = game_dir(dir) else { return };
    out.push(json!({
        "launcher": launcher,
        "name": name,
        "path": g.to_string_lossy(),
        "mods": count_entries(&g.join("mods")),
        "worlds": count_entries(&g.join("saves")),
        "detected": det
    }));
}

fn scan_instances_dir(out: &mut Vec<Value>, launcher: &str, dir: PathBuf, mode: u8) {
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for en in rd.flatten() {
            let p = en.path();
            if !p.is_dir() {
                continue;
            }
            let name = en.file_name().to_string_lossy().to_string();
            let det = match mode {
                1 => prism_detect(&p),
                2 => curse_detect(&p),
                _ => vec![],
            };
            push_candidate(out, launcher, name, &p, det);
        }
    }
}

/// Ищет установленные лаунчеры в обычных местах
#[tauri::command]
pub fn scan_launchers() -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    let data = dirs::data_dir().unwrap_or_default();
    let home = dirs::home_dir().unwrap_or_default();
    // обычный .minecraft (в том числе TLauncher и официальный лаунчер)
    for p in [data.join(".minecraft"), home.join(".minecraft")] {
        if p.is_dir() {
            let mut det: Vec<Value> = vec![];
            if let Ok(rd) = std::fs::read_dir(p.join("versions")) {
                for en in rd.flatten() {
                    if let Some(d) = parse_version_id(&en.file_name().to_string_lossy()) {
                        det.push(d);
                    }
                }
            }
            let mut name = ".minecraft (официальный, TLauncher и др.)".to_string();
            if p.join("tlauncher_profiles.json").exists() {
                name = ".minecraft (TLauncher)".to_string();
            }
            let before = out.len();
            push_candidate(&mut out, "Minecraft", name, &p, det);
            if out.len() > before && data == home {
                break;
            }
        }
    }
    for (l, sub, mode) in [
        ("Prism Launcher", "PrismLauncher/instances", 1u8),
        ("PolyMC", "PolyMC/instances", 1),
        ("MultiMC", "MultiMC/instances", 1),
        ("Modrinth App", "ModrinthApp/profiles", 0),
        ("ATLauncher", "ATLauncher/instances", 0),
        ("GDLauncher", "gdlauncher_next/instances", 0),
    ] {
        scan_instances_dir(&mut out, l, data.join(sub), mode);
    }
    scan_instances_dir(
        &mut out,
        "CurseForge",
        home.join("curseforge").join("minecraft").join("Instances"),
        2,
    );
    out
}

/// Определяет версию в выбранной вручную папке
#[tauri::command]
pub fn inspect_folder(path: String) -> Result<Value, String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    if !p.is_dir() {
        return Err("Это не папка".to_string());
    }
    let g = game_dir(&p).ok_or_else(|| {
        "В этой папке нет ни модов, ни миров, ни настроек. Выбери папку, где лежат mods или saves."
            .to_string()
    })?;
    let mut det = prism_detect(&p);
    if det.is_empty() {
        det = curse_detect(&p);
    }
    if det.is_empty() {
        if let Ok(rd) = std::fs::read_dir(g.join("versions")) {
            for en in rd.flatten() {
                if let Some(d) = parse_version_id(&en.file_name().to_string_lossy()) {
                    det.push(d);
                }
            }
        }
    }
    Ok(json!({
        "launcher": "Папка",
        "name": p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "path": g.to_string_lossy(),
        "mods": count_entries(&g.join("mods")),
        "worlds": count_entries(&g.join("saves")),
        "detected": det
    }))
}

#[tauri::command]
pub async fn import_folder(
    path: String,
    name: String,
    mc_version: String,
    loader: String,
    loader_version: String,
    with_worlds: bool,
) -> Result<instances::Instance, String> {
    let src = PathBuf::from(path.trim());
    if !src.is_dir() {
        return Err("Папка не найдена".to_string());
    }
    if !["vanilla", "fabric", "quilt", "forge", "neoforge"].contains(&loader.as_str()) {
        return Err("Неизвестный загрузчик".to_string());
    }
    let inst = instances::create(&name, &mc_version, &loader, &loader_version, "")?;
    let dst = dir_of(&inst.id);
    let sub: Vec<&str> = if with_worlds {
        vec!["mods", "config", "resourcepacks", "shaderpacks", "saves"]
    } else {
        vec!["mods", "config", "resourcepacks", "shaderpacks"]
    };
    let id = inst.id.clone();
    tokio::task::spawn_blocking(move || {
        for s in sub {
            let from = src.join(s);
            if from.is_dir() {
                copy_dir(&from, &dst.join(s)).map_err(|x| x.to_string())?;
            }
        }
        for f in ["options.txt", "servers.dat"] {
            let from = src.join(f);
            if from.is_file() {
                let _ = std::fs::copy(&from, dst.join(f));
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(e)??;
    instances::load_instance(&id)
}

// ---------------------------------------------------------------------------
//  Музыка лаунчера
// ---------------------------------------------------------------------------

fn music_dir() -> PathBuf {
    root().join("music")
}

fn music_name_ok(n: &str) -> Result<(), String> {
    let l = n.to_lowercase();
    let ok = ["mp3", "ogg", "wav", "m4a", "flac", "opus"]
        .iter()
        .any(|x| l.ends_with(&format!(".{}", x)));
    if n.is_empty() || n.contains('/') || n.contains('\\') || n.contains("..") || !ok {
        return Err("Недопустимое имя файла".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn music_list() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(music_dir())
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| music_name_ok(n).is_ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[tauri::command]
pub fn music_add(path: String) -> Result<String, String> {
    let src = PathBuf::from(&path);
    let name = src
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("track.mp3")
        .to_string();
    music_name_ok(&name)?;
    let size = std::fs::metadata(&src).map_err(e)?.len();
    if size > 40 * 1024 * 1024 {
        return Err("Файл больше 40 МБ".to_string());
    }
    std::fs::create_dir_all(music_dir()).map_err(e)?;
    std::fs::copy(&src, music_dir().join(&name)).map_err(e)?;
    Ok(name)
}

#[tauri::command]
pub fn music_delete(name: String) -> Result<(), String> {
    music_name_ok(&name)?;
    std::fs::remove_file(music_dir().join(name)).map_err(e)
}

#[tauri::command]
pub fn music_read(name: String) -> Result<String, String> {
    use base64::Engine;
    music_name_ok(&name)?;
    let b = std::fs::read(music_dir().join(&name)).map_err(e)?;
    let l = name.to_lowercase();
    let mime = if l.ends_with(".mp3") {
        "audio/mpeg"
    } else if l.ends_with(".ogg") || l.ends_with(".opus") {
        "audio/ogg"
    } else if l.ends_with(".wav") {
        "audio/wav"
    } else if l.ends_with(".flac") {
        "audio/flac"
    } else {
        "audio/mp4"
    };
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(b)
    ))
}

#[allow(dead_code)]
fn _keep(id: &str) -> Result<(), String> {
    safe_id(id)
}
