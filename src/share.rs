//! «Код сборки»: сборка превращается в короткую строку текста.
//! Отправил строку другу, он вставил её в лаунчер, и лаунчер скачал те же моды.
//! Ничего не загружается на сервера: в коде только список проектов Modrinth / CurseForge.

use crate::catalog;
use crate::instances;
use crate::progress;
use base64::Engine;
use futures_util::StreamExt;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tauri::AppHandle;

const PREFIX: &str = "ML1.";
const MAX_ITEMS: usize = 600;
const MAX_TEXT: usize = 60_000;

fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '|')
        .collect::<String>()
        .trim()
        .chars()
        .take(60)
        .collect()
}

fn kind_char(kind: &str) -> Option<char> {
    match kind {
        "mod" => Some('m'),
        "resourcepack" => Some('r'),
        "shader" => Some('s'),
        _ => None,
    }
}

fn kind_name(c: char) -> Option<&'static str> {
    match c {
        'm' => Some("mod"),
        'r' => Some("resourcepack"),
        's' => Some("shader"),
        _ => None,
    }
}

fn source_char(src: &str) -> Option<char> {
    match src {
        "modrinth" => Some('m'),
        "curseforge" => Some('c'),
        _ => None,
    }
}

fn source_name(c: char) -> Option<&'static str> {
    match c {
        'm' => Some("modrinth"),
        'c' => Some("curseforge"),
        _ => None,
    }
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

fn ver_ok(v: &str) -> bool {
    v.len() <= 60
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '+')
}

fn checksum(body: &str) -> String {
    let mut h = Sha1::new();
    h.update(body.as_bytes());
    let d = h.finalize();
    d.iter().take(2).map(|b| format!("{:02x}", b)).collect()
}

struct Parsed {
    name: String,
    mc: String,
    loader: String,
    loader_version: String,
    /// (источник, тип, id проекта)
    items: Vec<(String, String, String)>,
}

/// Достаёт код из любого текста (на случай, если человек скопировал лишнее)
fn find_code(text: &str) -> Option<String> {
    let start = text.find(PREFIX)?;
    let tail = &text[start..];
    let code: String = tail
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    Some(code)
}

fn build_code(
    name: &str,
    mc: &str,
    loader: &str,
    lv: &str,
    items: &[(char, char, String)],
) -> String {
    let mut text = format!("{}|{}|{}|{}", clean(name), mc, loader, lv);
    for (s, k, id) in items {
        text.push('\n');
        text.push(*s);
        text.push(*k);
        text.push_str(id);
    }
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(text.as_bytes());
    format!("{}{}.{}", PREFIX, b64, checksum(&b64))
}

fn parse_code(raw: &str) -> Result<Parsed, String> {
    let code = find_code(raw)
        .ok_or_else(|| "В тексте нет кода сборки (он начинается с ML1.)".to_string())?;
    let body = &code[PREFIX.len()..];
    let (b64, chk) = body
        .rsplit_once('.')
        .ok_or_else(|| "Код сборки неполный. Скопируй его целиком".to_string())?;
    if checksum(b64) != chk.to_lowercase() {
        return Err("Код повреждён или скопирован не до конца. Скопируй его целиком".to_string());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|_| "Код повреждён".to_string())?;
    if bytes.len() > MAX_TEXT {
        return Err("Код слишком большой".to_string());
    }
    let text = String::from_utf8(bytes).map_err(|_| "Код повреждён".to_string())?;
    let mut lines = text.lines();
    let head = lines.next().ok_or_else(|| "Код пустой".to_string())?;
    let parts: Vec<&str> = head.split('|').collect();
    if parts.len() != 4 {
        return Err("Код от другой версии лаунчера".to_string());
    }
    let (name, mc, loader, lv) = (parts[0], parts[1], parts[2], parts[3]);
    if mc.is_empty() || !ver_ok(mc) || !ver_ok(lv) {
        return Err("В коде неверная версия игры".to_string());
    }
    if !["vanilla", "fabric", "quilt", "forge", "neoforge"].contains(&loader) {
        return Err("В коде неизвестный загрузчик".to_string());
    }
    let mut items = vec![];
    for l in lines {
        let l = l.trim();
        if l.chars().count() < 3 {
            continue;
        }
        let mut ch = l.chars();
        let s = ch.next().and_then(source_name);
        let k = ch.next().and_then(kind_name);
        let id: String = ch.collect();
        if let (Some(s), Some(k)) = (s, k) {
            if id_ok(&id) {
                items.push((s.to_string(), k.to_string(), id));
            }
        }
        if items.len() > MAX_ITEMS {
            return Err("В коде слишком много модов".to_string());
        }
    }
    Ok(Parsed {
        name: name.to_string(),
        mc: mc.to_string(),
        loader: loader.to_string(),
        loader_version: lv.to_string(),
        items,
    })
}

/// Делает код из сборки
#[tauri::command]
pub fn pack_code_export(instance_id: String) -> Result<Value, String> {
    let inst = instances::load_instance(&instance_id)?;
    let idir = instances::dir_of(&instance_id);
    let mut items: Vec<(char, char, String)> = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut mods_total = 0u32;
    for kind in ["mod", "resourcepack", "shader"] {
        let sub = match kind {
            "mod" => "mods",
            "resourcepack" => "resourcepacks",
            _ => "shaderpacks",
        };
        if let Ok(rd) = std::fs::read_dir(idir.join(sub)) {
            for en in rd.flatten() {
                let n = en.file_name().to_string_lossy().to_string();
                if !n.ends_with(".disabled") {
                    mods_total += 1;
                }
            }
        }
    }
    for r in instances::read_records(&instance_id) {
        let kind = r["kind"].as_str().unwrap_or("");
        let (Some(k), Some(s)) = (kind_char(kind), r["source"].as_str().and_then(source_char))
        else {
            continue;
        };
        let pid = r["project_id"].as_str().unwrap_or("");
        let fname = r["file_name"].as_str().unwrap_or("");
        if !id_ok(pid) || fname.is_empty() {
            continue;
        }
        // только то, что реально лежит в папке и не выключено
        let sub = match kind {
            "mod" => "mods",
            "resourcepack" => "resourcepacks",
            _ => "shaderpacks",
        };
        if !idir.join(sub).join(fname).exists() {
            continue;
        }
        if seen.insert((s, k, pid.to_string())) {
            items.push((s, k, pid.to_string()));
        }
    }
    items.truncate(MAX_ITEMS);
    let skipped = mods_total.saturating_sub(items.len() as u32);
    let code = build_code(
        &inst.name,
        &inst.mc_version,
        &inst.loader,
        &inst.loader_version,
        &items,
    );
    Ok(json!({
        "code": code,
        "count": items.len(),
        "skipped": skipped,
        "length": code.len(),
        "name": inst.name,
    }))
}

/// Показывает, что внутри кода (без скачивания), чтобы человек подтвердил
#[tauri::command]
pub fn pack_code_preview(code: String) -> Result<Value, String> {
    let p = parse_code(&code)?;
    let mut mods = 0;
    let mut packs = 0;
    let mut shaders = 0;
    let mut cf = 0;
    for (s, k, _) in &p.items {
        match k.as_str() {
            "mod" => mods += 1,
            "resourcepack" => packs += 1,
            _ => shaders += 1,
        }
        if s == "curseforge" {
            cf += 1;
        }
    }
    Ok(json!({
        "name": p.name,
        "mc_version": p.mc,
        "loader": p.loader,
        "loader_version": p.loader_version,
        "mods": mods,
        "resourcepacks": packs,
        "shaders": shaders,
        "curseforge": cf,
    }))
}

/// Создаёт новую сборку по коду и скачивает всё, что в нём указано
#[tauri::command]
pub async fn pack_code_import(
    app: AppHandle,
    code: String,
    name: Option<String>,
    cf_key: String,
) -> Result<Value, String> {
    let p = parse_code(&code)?;
    let title = name
        .map(|n| clean(&n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if p.name.is_empty() {
                "Сборка по коду".to_string()
            } else {
                p.name.clone()
            }
        });
    let inst = instances::create(&title, &p.mc, &p.loader, &p.loader_version, "")?;
    let c = catalog::http_client()?;
    let total = p.items.len().max(1);
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let has_key = !cf_key.trim().is_empty();
    progress(&app, "Скачиваю по коду…", 0.02);
    let results: Vec<Result<String, String>> = futures_util::stream::iter(p.items.into_iter())
        .map(|(src, kind, pid)| {
            let c = c.clone();
            let inst = inst.clone();
            let key = cf_key.clone();
            let app = app.clone();
            let done = done.clone();
            async move {
                let r = if src == "curseforge" && !has_key {
                    Err(format!("{} (нужен ключ CurseForge)", pid))
                } else {
                    catalog::install_one(&c, &key, &inst, &src, &kind, &pid)
                        .await
                        .map_err(|m| format!("{}: {}", pid, m))
                };
                let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                progress(
                    &app,
                    &format!("Скачиваю по коду: {} из {}", d, total),
                    0.05 + 0.9 * (d as f64 / total as f64),
                );
                r
            }
        })
        .buffer_unordered(4)
        .collect()
        .await;
    progress(&app, "Готово", 1.0);
    let ok = results.iter().filter(|r| r.is_ok()).count();
    let failed: Vec<String> = results.into_iter().filter_map(|r| r.err()).collect();
    Ok(json!({
        "instance": inst,
        "ok": ok,
        "failed": failed,
    }))
}
