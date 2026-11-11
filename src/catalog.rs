//! Каталог модов, ресурспаков, шейдеров, датапаков, миров и сборок (Modrinth + CurseForge).

use crate::instances::{self, Instance};
use crate::{e, progress};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const MR: &str = "https://api.modrinth.com/v2";
const CF: &str = "https://api.curseforge.com";
const CF_GAME: &str = "432"; // Minecraft

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("MineLauncher/0.3")
        .build()
        .map_err(e)
}

// ---------------------------------------------------------------------------
//  Запросы
// ---------------------------------------------------------------------------

async fn mr_get(c: &reqwest::Client, path: &str, q: &[(&str, String)]) -> Result<Value, String> {
    let r = c
        .get(format!("{}{}", MR, path))
        .query(q)
        .send()
        .await
        .map_err(|_| "Нет связи с Modrinth".to_string())?;
    if !r.status().is_success() {
        return Err(format!("Modrinth вернул ошибку {}", r.status().as_u16()));
    }
    r.json().await.map_err(e)
}

async fn cf_get(
    c: &reqwest::Client,
    key: &str,
    path: &str,
    q: &[(&str, String)],
) -> Result<Value, String> {
    if key.trim().is_empty() {
        return Err("Нужен API-ключ CurseForge. Вставьте его в Настройки → Каталог".to_string());
    }
    let r = c
        .get(format!("{}{}", CF, path))
        .header("x-api-key", key.trim())
        .query(q)
        .send()
        .await
        .map_err(|_| "Нет связи с CurseForge".to_string())?;
    let code = r.status().as_u16();
    if code == 401 || code == 403 {
        return Err("CurseForge не принял ключ. Проверьте его в настройках".to_string());
    }
    if !r.status().is_success() {
        return Err(format!("CurseForge вернул ошибку {}", code));
    }
    r.json().await.map_err(e)
}

async fn fetch(c: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let r = c
        .get(url)
        .send()
        .await
        .map_err(|_| "Не удалось скачать файл (нет сети?)".to_string())?;
    if !r.status().is_success() {
        return Err(format!(
            "Сервер не отдал файл (код {})",
            r.status().as_u16()
        ));
    }
    let b = r.bytes().await.map_err(e)?;
    Ok(b.to_vec())
}

fn sha1_hex(b: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(b);
    h.finalize().iter().map(|x| format!("{:02x}", x)).collect()
}

fn cf_loader(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn cf_class(kind: &str) -> Option<&'static str> {
    match kind {
        "mod" => Some("6"),
        "resourcepack" => Some("12"),
        "shader" => Some("6552"),
        "datapack" => Some("6945"),
        "world" => Some("17"),
        "modpack" => Some("4471"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
//  Поиск
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn catalog_search(
    source: String,
    kind: String,
    query: String,
    mc: String,
    loader: String,
    sort: String,
    offset: u32,
    cf_key: String,
    category: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let c = client()?;
    let limit = limit.unwrap_or(24).clamp(10, 100);
    let uses_loader = kind == "mod" || kind == "modpack";

    if source == "modrinth" {
        if kind == "world" {
            return Err("Миры есть только в CurseForge".to_string());
        }
        let mut facets: Vec<String> = vec![format!("[\"project_type:{}\"]", kind)];
        if !mc.is_empty() {
            facets.push(format!("[\"versions:{}\"]", mc));
        }
        if !loader.is_empty() && uses_loader {
            facets.push(format!("[\"categories:{}\"]", loader));
        }
        if let Some(cat) = category.as_ref().filter(|x| {
            !x.is_empty() && x.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        }) {
            facets.push(format!("[\"categories:{}\"]", cat));
        }
        let index = match sort.as_str() {
            "downloads" => "downloads",
            "follows" => "follows",
            "newest" => "newest",
            "updated" => "updated",
            _ => "relevance",
        };
        let q = vec![
            ("query", query),
            ("facets", format!("[{}]", facets.join(","))),
            ("index", index.to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];
        let v = mr_get(&c, "/search", &q).await?;
        let hits: Vec<Value> = v["hits"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|h| {
                json!({
                    "source": "modrinth",
                    "id": h["project_id"],
                    "title": h["title"],
                    "author": h["author"],
                    "description": h["description"],
                    "icon": h["icon_url"],
                    "downloads": h["downloads"],
                    "updated": h["date_modified"],
                    "categories": h["display_categories"],
                    "url": format!("https://modrinth.com/{}/{}", kind, h["slug"].as_str().unwrap_or(""))
                })
            })
            .collect();
        return Ok(json!({ "hits": hits, "total": v["total_hits"] }));
    }

    // CurseForge
    let class = cf_class(&kind).ok_or_else(|| "Неизвестный тип".to_string())?;
    let sort_field = match sort.as_str() {
        "downloads" => "6",
        "newest" => "11",
        "updated" => "3",
        "follows" => "2",
        _ => "2",
    };
    let mut q: Vec<(&str, String)> = vec![
        ("gameId", CF_GAME.to_string()),
        ("classId", class.to_string()),
        ("searchFilter", query),
        ("sortField", sort_field.to_string()),
        ("sortOrder", "desc".to_string()),
        ("pageSize", limit.min(50).to_string()),
        ("index", offset.to_string()),
    ];
    if !mc.is_empty() {
        q.push(("gameVersion", mc));
        if uses_loader {
            if let Some(t) = cf_loader(&loader) {
                q.push(("modLoaderType", t.to_string()));
            }
        }
    }
    let v = cf_get(&c, &cf_key, "/v1/mods/search", &q).await?;
    let hits: Vec<Value> = v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|h| {
            let cats: Vec<Value> = h["categories"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|x| x["name"].clone())
                .collect();
            json!({
                "source": "curseforge",
                "id": h["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                "title": h["name"],
                "author": h["authors"][0]["name"],
                "description": h["summary"],
                "icon": h["logo"]["thumbnailUrl"],
                "downloads": h["downloadCount"],
                "updated": h["dateModified"],
                "categories": cats,
                "url": h["links"]["websiteUrl"]
            })
        })
        .collect();
    Ok(json!({ "hits": hits, "total": v["pagination"]["totalCount"] }))
}

// ---------------------------------------------------------------------------
//  Установка одного файла (с зависимостями)
// ---------------------------------------------------------------------------

fn target_dir(idir: &Path, kind: &str, world: &Option<String>) -> Result<PathBuf, String> {
    match kind {
        "mod" => Ok(idir.join("mods")),
        "resourcepack" => Ok(idir.join("resourcepacks")),
        "shader" => Ok(idir.join("shaderpacks")),
        "world" => Ok(idir.join("saves")),
        "datapack" => {
            let w = world
                .as_ref()
                .ok_or_else(|| "Выберите мир для датапака".to_string())?;
            instances::safe_file(w)?;
            let d = idir.join("saves").join(w);
            if !d.is_dir() {
                return Err("Мир не найден".to_string());
            }
            Ok(d.join("datapacks"))
        }
        _ => Err("Неизвестный тип".to_string()),
    }
}

/// Распаковывает мир из zip в папку saves
fn extract_world(bytes: Vec<u8>, saves: &Path, fallback_name: &str) -> Result<String, String> {
    let mut zip =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Файл мира повреждён".to_string())?;
    // ищем level.dat с наименьшей вложенностью
    let mut prefix: Option<String> = None;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(e)?;
        let name = f.name().to_string();
        if name.ends_with("level.dat") {
            let pre = name[..name.len() - "level.dat".len()].to_string();
            let better = match &prefix {
                Some(p) => pre.len() < p.len(),
                None => true,
            };
            if better {
                prefix = Some(pre);
            }
        }
    }
    let prefix = prefix.ok_or_else(|| "В архиве нет level.dat: это не мир".to_string())?;
    let mut world_name = prefix
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();
    if world_name.is_empty() {
        world_name = fallback_name.to_string();
    }
    let world_name: String = world_name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let dest = saves.join(&world_name);
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(e)?;
        let name = f.name().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }
        let Some(enclosed) = f.enclosed_name() else {
            continue;
        };
        let enclosed_str = enclosed.to_string_lossy().replace('\\', "/");
        let rel = enclosed_str[prefix.len().min(enclosed_str.len())..].to_string();
        if rel.is_empty() {
            continue;
        }
        let out = dest.join(&rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(e)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(e)?;
            }
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(e)?;
            std::fs::write(&out, buf).map_err(e)?;
        }
    }
    Ok(world_name)
}

struct Installed {
    title: String,
    saved: String,
    deps: Vec<String>,
}

fn save_file(
    inst: &Instance,
    kind: &str,
    world: &Option<String>,
    file_name: &str,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let idir = instances::dir_of(&inst.id);
    let dir = target_dir(&idir, kind, world)?;
    std::fs::create_dir_all(&dir).map_err(e)?;
    if kind == "world" {
        let stem = file_name.trim_end_matches(".zip").to_string();
        return extract_world(bytes, &dir, &stem);
    }
    instances::safe_file(file_name)?;
    std::fs::write(dir.join(file_name), bytes).map_err(e)?;
    Ok(file_name.to_string())
}

async fn mr_install(
    c: &reqwest::Client,
    inst: &Instance,
    kind: &str,
    pid: &str,
    loader: &str,
    world: &Option<String>,
) -> Result<Installed, String> {
    let proj = mr_get(c, &format!("/project/{}", pid), &[]).await?;
    let title = proj["title"].as_str().unwrap_or(pid).to_string();
    let icon = proj["icon_url"].as_str().unwrap_or("").to_string();

    let mut q: Vec<(&str, String)> = vec![];
    if !inst.mc_version.is_empty() {
        q.push(("game_versions", format!("[\"{}\"]", inst.mc_version)));
    }
    if !loader.is_empty() {
        q.push(("loaders", format!("[\"{}\"]", loader)));
    }
    let path = format!("/project/{}/version", pid);
    let mut versions = mr_get(c, &path, &q)
        .await?
        .as_array()
        .cloned()
        .unwrap_or_default();
    if versions.is_empty() && kind != "mod" {
        versions = mr_get(c, &path, &[])
            .await?
            .as_array()
            .cloned()
            .unwrap_or_default();
    }
    if versions.is_empty() {
        return Err(format!(
            "Для «{}» нет версии под {} {}",
            title, inst.loader, inst.mc_version
        ));
    }
    let ver = versions
        .iter()
        .find(|v| v["version_type"].as_str() == Some("release"))
        .unwrap_or(&versions[0])
        .clone();
    let files = ver["files"].as_array().cloned().unwrap_or_default();
    let f = files
        .iter()
        .find(|f| f["primary"].as_bool() == Some(true))
        .or(files.first())
        .ok_or_else(|| "В версии нет файлов".to_string())?;
    let file_name = f["filename"].as_str().unwrap_or("file.jar").to_string();
    let url = f["url"]
        .as_str()
        .ok_or_else(|| "Нет ссылки на файл".to_string())?;
    let bytes = fetch(c, url).await?;
    if let Some(h) = f["hashes"]["sha1"].as_str() {
        if !sha1_hex(&bytes).eq_ignore_ascii_case(h) {
            return Err("Файл повреждён при скачивании (не совпала контрольная сумма)".to_string());
        }
    }
    let saved = save_file(inst, kind, world, &file_name, bytes)?;
    if kind != "datapack" && kind != "world" {
        instances::add_record(
            &inst.id,
            json!({ "file_name": saved, "kind": kind, "source": "modrinth",
                    "project_id": pid, "title": title, "icon": icon }),
        );
    }
    let mut deps: Vec<String> = vec![];
    if kind == "mod" {
        for d in ver["dependencies"].as_array().cloned().unwrap_or_default() {
            if d["dependency_type"].as_str() == Some("required") {
                if let Some(p) = d["project_id"].as_str() {
                    deps.push(p.to_string());
                }
            }
        }
    }
    Ok(Installed { title, saved, deps })
}

fn cf_download_url(f: &Value) -> Option<String> {
    if let Some(u) = f["downloadUrl"].as_str() {
        if !u.is_empty() {
            return Some(u.to_string());
        }
    }
    let id = f["id"].as_u64()?;
    let name = f["fileName"].as_str()?;
    Some(format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        id / 1000,
        id % 1000,
        name
    ))
}

fn cf_sha1(f: &Value) -> Option<String> {
    f["hashes"]
        .as_array()?
        .iter()
        .find(|h| h["algo"].as_u64() == Some(1))
        .and_then(|h| h["value"].as_str())
        .map(|s| s.to_string())
}

async fn cf_install(
    c: &reqwest::Client,
    key: &str,
    inst: &Instance,
    kind: &str,
    pid: &str,
    loader: &str,
    world: &Option<String>,
) -> Result<Installed, String> {
    let m = cf_get(c, key, &format!("/v1/mods/{}", pid), &[]).await?;
    let md = &m["data"];
    let title = md["name"].as_str().unwrap_or(pid).to_string();
    let icon = md["logo"]["thumbnailUrl"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let mut q: Vec<(&str, String)> = vec![("pageSize", "50".to_string())];
    if !inst.mc_version.is_empty() {
        q.push(("gameVersion", inst.mc_version.clone()));
    }
    if let Some(t) = cf_loader(loader) {
        q.push(("modLoaderType", t.to_string()));
    }
    let path = format!("/v1/mods/{}/files", pid);
    let mut files = cf_get(c, key, &path, &q).await?["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if files.is_empty() && kind != "mod" {
        files = cf_get(c, key, &path, &[("pageSize", "50".to_string())]).await?["data"]
            .as_array()
            .cloned()
            .unwrap_or_default();
    }
    if files.is_empty() {
        return Err(format!(
            "Для «{}» нет файла под {} {}",
            title, inst.loader, inst.mc_version
        ));
    }
    files.sort_by(|a, b| {
        let da = a["fileDate"].as_str().unwrap_or("");
        let db = b["fileDate"].as_str().unwrap_or("");
        db.cmp(da)
    });
    let f = files
        .iter()
        .find(|f| f["releaseType"].as_u64() == Some(1))
        .or(files.first())
        .ok_or_else(|| "Нет файлов".to_string())?
        .clone();
    if f["downloadUrl"].is_null() && md["allowModDistribution"].as_bool() == Some(false) {
        return Err(format!(
            "Автор «{}» запретил скачивание вне сайта CurseForge",
            title
        ));
    }
    let url = cf_download_url(&f).ok_or_else(|| "Нет ссылки на файл".to_string())?;
    let file_name = f["fileName"].as_str().unwrap_or("file.jar").to_string();
    let bytes = fetch(c, &url).await?;
    if let Some(h) = cf_sha1(&f) {
        if !sha1_hex(&bytes).eq_ignore_ascii_case(&h) {
            return Err("Файл повреждён при скачивании (не совпала контрольная сумма)".to_string());
        }
    }
    let saved = save_file(inst, kind, world, &file_name, bytes)?;
    if kind != "datapack" && kind != "world" {
        instances::add_record(
            &inst.id,
            json!({ "file_name": saved, "kind": kind, "source": "curseforge",
                    "project_id": pid, "title": title, "icon": icon }),
        );
    }
    let mut deps: Vec<String> = vec![];
    if kind == "mod" {
        for d in f["dependencies"].as_array().cloned().unwrap_or_default() {
            if d["relationType"].as_u64() == Some(3) {
                if let Some(id) = d["modId"].as_u64() {
                    deps.push(id.to_string());
                }
            }
        }
    }
    Ok(Installed { title, saved, deps })
}

/// Ставит один файл (без зависимостей) в сборку. Нужна для «Кода сборки».
/// Возвращает название установленного проекта.
pub(crate) async fn install_one(
    c: &reqwest::Client,
    cf_key: &str,
    inst: &Instance,
    source: &str,
    kind: &str,
    pid: &str,
) -> Result<String, String> {
    let loader = if kind == "mod" && inst.loader != "vanilla" {
        inst.loader.clone()
    } else {
        String::new()
    };
    let res = if source == "modrinth" {
        mr_install(c, inst, kind, pid, &loader, &None).await?
    } else {
        cf_install(c, cf_key, inst, kind, pid, &loader, &None).await?
    };
    Ok(res.title)
}

pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    client()
}

#[tauri::command]
pub async fn catalog_install(
    app: AppHandle,
    instance_id: String,
    source: String,
    kind: String,
    project_id: String,
    cf_key: String,
    world: Option<String>,
) -> Result<String, String> {
    let inst = instances::load_instance(&instance_id)?;
    let c = client()?;
    let loader = if kind == "mod" && inst.loader != "vanilla" {
        inst.loader.clone()
    } else {
        String::new()
    };
    let mut queue: Vec<(String, u8)> = vec![(project_id, 0)];
    let mut seen: HashSet<String> = HashSet::new();
    let mut done: Vec<String> = vec![];
    let mut skipped: u32 = 0;
    while let Some((pid, depth)) = queue.pop() {
        if !seen.insert(pid.clone()) {
            continue;
        }
        progress(&app, "Скачиваю…", 0.5);
        let res = if source == "modrinth" {
            mr_install(&c, &inst, &kind, &pid, &loader, &world).await
        } else {
            cf_install(&c, &cf_key, &inst, &kind, &pid, &loader, &world).await
        };
        match res {
            Ok(it) => {
                done.push(it.title);
                if depth < 3 {
                    for d in it.deps {
                        queue.push((d, depth + 1));
                    }
                }
            }
            Err(err) => {
                if depth == 0 {
                    return Err(err);
                }
                skipped += 1;
            }
        }
    }
    progress(&app, "Готово", 1.0);
    let mut msg = format!("Установлено: {}", done.join(", "));
    if skipped > 0 {
        msg.push_str(&format!(". Зависимостей пропущено: {}", skipped));
    }
    Ok(msg)
}

// ---------------------------------------------------------------------------
//  Сборки от других людей
// ---------------------------------------------------------------------------

fn read_zip_text(zip: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

/// Копирует файлы из папок overrides внутрь сборки
fn extract_overrides(
    zip: &mut zip::ZipArchive<Cursor<Vec<u8>>>,
    prefixes: &[String],
    dest: &Path,
) -> Result<(), String> {
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(e)?;
        let name = f.name().replace('\\', "/");
        let mut rel: Option<String> = None;
        for p in prefixes {
            if let Some(r) = name.strip_prefix(p.as_str()) {
                rel = Some(r.to_string());
                break;
            }
        }
        let Some(rel) = rel else { continue };
        if rel.is_empty() || rel.contains("..") || rel.starts_with('/') {
            continue;
        }
        let out = dest.join(&rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(e)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(e)?;
            }
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(e)?;
            std::fs::write(&out, buf).map_err(e)?;
        }
    }
    Ok(())
}

struct PackFile {
    urls: Vec<String>,
    dest: PathBuf,
    sha1: Option<String>,
}

async fn download_pack_files(app: &AppHandle, c: &reqwest::Client, files: Vec<PackFile>) -> u32 {
    let total = files.len().max(1);
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let results: Vec<bool> = futures_util::stream::iter(files)
        .map(|f| {
            let c = c.clone();
            let app = app.clone();
            let counter = counter.clone();
            async move {
                let mut ok = false;
                for u in &f.urls {
                    if let Ok(bytes) = fetch(&c, u).await {
                        let good = match &f.sha1 {
                            Some(h) => sha1_hex(&bytes).eq_ignore_ascii_case(h),
                            None => true,
                        };
                        if good {
                            if let Some(p) = f.dest.parent() {
                                let _ = std::fs::create_dir_all(p);
                            }
                            if std::fs::write(&f.dest, bytes).is_ok() {
                                ok = true;
                                break;
                            }
                        }
                    }
                }
                let d = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                let pct = 0.05 + 0.9 * (d as f64 / total as f64);
                progress(&app, &format!("Сборка: файл {} из {}", d, total), pct);
                ok
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;
    results.iter().filter(|ok| !**ok).count() as u32
}

fn rel_ok(p: &str) -> bool {
    !(p.is_empty() || p.starts_with('/') || p.contains("..") || p.contains(':'))
}

async fn install_mrpack(
    app: &AppHandle,
    c: &reqwest::Client,
    bytes: Vec<u8>,
    fallback_name: &str,
    icon: &str,
) -> Result<Instance, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "Файл сборки повреждён".to_string())?;
    let text = read_zip_text(&mut zip, "modrinth.index.json")
        .ok_or_else(|| "В архиве нет modrinth.index.json".to_string())?;
    let idx: Value = serde_json::from_str(&text).map_err(e)?;
    let deps = &idx["dependencies"];
    let mc = deps["minecraft"]
        .as_str()
        .ok_or_else(|| "В сборке не указана версия игры".to_string())?
        .to_string();
    let (loader, lv) = if let Some(v) = deps["fabric-loader"].as_str() {
        ("fabric", v.to_string())
    } else if let Some(v) = deps["quilt-loader"].as_str() {
        ("quilt", v.to_string())
    } else if let Some(v) = deps["neoforge"].as_str() {
        ("neoforge", v.to_string())
    } else if let Some(v) = deps["forge"].as_str() {
        ("forge", format!("{}-{}", mc, v))
    } else {
        ("vanilla", String::new())
    };
    let name = idx["name"].as_str().unwrap_or(fallback_name).to_string();
    let inst = instances::create(&name, &mc, loader, &lv, icon)?;
    let idir = instances::dir_of(&inst.id);

    let mut files: Vec<PackFile> = vec![];
    for f in idx["files"].as_array().cloned().unwrap_or_default() {
        if f["env"]["client"].as_str() == Some("unsupported") {
            continue;
        }
        let path = f["path"].as_str().unwrap_or("");
        if !rel_ok(path) {
            continue;
        }
        let urls: Vec<String> = f["downloads"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|u| u.as_str().map(|s| s.to_string()))
            .collect();
        files.push(PackFile {
            urls,
            dest: idir.join(path),
            sha1: f["hashes"]["sha1"].as_str().map(|s| s.to_string()),
        });
    }
    download_pack_files(app, c, files).await;
    extract_overrides(
        &mut zip,
        &["overrides/".to_string(), "client-overrides/".to_string()],
        &idir,
    )?;
    Ok(inst)
}

async fn install_cfpack(
    app: &AppHandle,
    c: &reqwest::Client,
    key: &str,
    bytes: Vec<u8>,
    fallback_name: &str,
    icon: &str,
) -> Result<Instance, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "Файл сборки повреждён".to_string())?;
    let text = read_zip_text(&mut zip, "manifest.json")
        .ok_or_else(|| "В архиве нет manifest.json".to_string())?;
    let man: Value = serde_json::from_str(&text).map_err(e)?;
    let mc = man["minecraft"]["version"]
        .as_str()
        .ok_or_else(|| "В сборке не указана версия игры".to_string())?
        .to_string();
    let loaders = man["minecraft"]["modLoaders"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let primary = loaders
        .iter()
        .find(|l| l["primary"].as_bool() == Some(true))
        .or(loaders.first());
    let (loader, lv) = match primary.and_then(|l| l["id"].as_str()) {
        Some(id) => match id.split_once('-') {
            Some(("forge", v)) => ("forge", format!("{}-{}", mc, v)),
            Some(("neoforge", v)) => ("neoforge", v.to_string()),
            Some(("fabric", v)) => ("fabric", v.to_string()),
            Some(("quilt", v)) => ("quilt", v.to_string()),
            _ => ("vanilla", String::new()),
        },
        None => ("vanilla", String::new()),
    };
    let name = man["name"].as_str().unwrap_or(fallback_name).to_string();
    let inst = instances::create(&name, &mc, loader, &lv, icon)?;
    let idir = instances::dir_of(&inst.id);

    let ids: Vec<Value> = man["files"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|f| f["fileID"].as_u64())
        .map(|n| json!(n))
        .collect();
    if !ids.is_empty() {
        if key.trim().is_empty() {
            return Err("Для сборок CurseForge нужен API-ключ (Настройки → Каталог)".to_string());
        }
        let r = c
            .post(format!("{}/v1/mods/files", CF))
            .header("x-api-key", key.trim())
            .json(&json!({ "fileIds": ids }))
            .send()
            .await
            .map_err(|_| "Нет связи с CurseForge".to_string())?;
        if !r.status().is_success() {
            return Err("CurseForge не отдал список файлов сборки".to_string());
        }
        let data: Value = r.json().await.map_err(e)?;
        let mut files: Vec<PackFile> = vec![];
        for f in data["data"].as_array().cloned().unwrap_or_default() {
            let fname = f["fileName"].as_str().unwrap_or("").to_string();
            if !rel_ok(&fname) {
                continue;
            }
            let Some(url) = cf_download_url(&f) else {
                continue;
            };
            let folder = if fname.ends_with(".jar") {
                "mods"
            } else {
                "resourcepacks"
            };
            files.push(PackFile {
                urls: vec![url],
                dest: idir.join(folder).join(&fname),
                sha1: cf_sha1(&f),
            });
        }
        download_pack_files(app, c, files).await;
    }
    let over = man["overrides"].as_str().unwrap_or("overrides");
    extract_overrides(&mut zip, &[format!("{}/", over)], &idir)?;
    Ok(inst)
}

/// Установка сборки из каталога: создаёт новую сборку
#[tauri::command]
pub async fn modpack_install(
    app: AppHandle,
    source: String,
    project_id: String,
    title: String,
    icon: String,
    cf_key: String,
) -> Result<instances::Instance, String> {
    let c = client()?;
    progress(&app, "Ищу файл сборки…", 0.02);
    if source == "modrinth" {
        let versions = mr_get(&c, &format!("/project/{}/version", project_id), &[])
            .await?
            .as_array()
            .cloned()
            .unwrap_or_default();
        let ver = versions
            .iter()
            .find(|v| v["version_type"].as_str() == Some("release"))
            .or(versions.first())
            .ok_or_else(|| "У сборки нет версий".to_string())?
            .clone();
        let files = ver["files"].as_array().cloned().unwrap_or_default();
        let f = files
            .iter()
            .find(|f| f["primary"].as_bool() == Some(true))
            .or(files.first())
            .ok_or_else(|| "В версии нет файлов".to_string())?;
        let url = f["url"]
            .as_str()
            .ok_or_else(|| "Нет ссылки на файл".to_string())?;
        progress(&app, "Скачиваю сборку…", 0.03);
        let bytes = fetch(&c, url).await?;
        install_mrpack(&app, &c, bytes, &title, &icon).await
    } else {
        let r = cf_get(
            &c,
            &cf_key,
            &format!("/v1/mods/{}/files", project_id),
            &[("pageSize", "20".to_string())],
        )
        .await?;
        let mut files = r["data"].as_array().cloned().unwrap_or_default();
        files.sort_by(|a, b| {
            let da = a["fileDate"].as_str().unwrap_or("");
            let db = b["fileDate"].as_str().unwrap_or("");
            db.cmp(da)
        });
        let f = files
            .iter()
            .find(|f| f["releaseType"].as_u64() == Some(1))
            .or(files.first())
            .ok_or_else(|| "У сборки нет файлов".to_string())?
            .clone();
        let url = cf_download_url(&f).ok_or_else(|| "Нет ссылки на файл".to_string())?;
        progress(&app, "Скачиваю сборку…", 0.03);
        let bytes = fetch(&c, &url).await?;
        install_cfpack(&app, &c, &cf_key, bytes, &title, &icon).await
    }
}

/// Импорт сборки из файла (.mrpack или zip от CurseForge)
#[tauri::command]
pub async fn import_pack(
    app: AppHandle,
    path: String,
    cf_key: String,
) -> Result<instances::Instance, String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    let bytes = std::fs::read(&p).map_err(|_| "Не удалось прочитать файл".to_string())?;
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Сборка".to_string());
    let c = client()?;
    let is_mr = {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone()))
            .map_err(|_| "Это не архив сборки".to_string())?;
        let found = z.by_name("modrinth.index.json").is_ok();
        found
    };
    if is_mr {
        install_mrpack(&app, &c, bytes, &stem, "").await
    } else {
        install_cfpack(&app, &c, &cf_key, bytes, &stem, "").await
    }
}

// ---------------------------------------------------------------------------
//  Обновление установленного
// ---------------------------------------------------------------------------

async fn mr_latest_name(c: &reqwest::Client, mc: &str, pid: &str, loader: &str) -> Option<String> {
    let mut q: Vec<(&str, String)> = vec![];
    if !mc.is_empty() {
        q.push(("game_versions", format!("[\"{}\"]", mc)));
    }
    if !loader.is_empty() {
        q.push(("loaders", format!("[\"{}\"]", loader)));
    }
    let v = mr_get(c, &format!("/project/{}/version", pid), &q)
        .await
        .ok()?;
    let versions = v.as_array()?;
    let ver = versions
        .iter()
        .find(|x| x["version_type"].as_str() == Some("release"))
        .or(versions.first())?;
    let files = ver["files"].as_array()?;
    let f = files
        .iter()
        .find(|x| x["primary"].as_bool() == Some(true))
        .or(files.first())?;
    f["filename"].as_str().map(|s| s.to_string())
}

async fn cf_latest_name(
    c: &reqwest::Client,
    key: &str,
    mc: &str,
    pid: &str,
    loader: &str,
) -> Option<String> {
    let mut q: Vec<(&str, String)> = vec![("pageSize", "50".to_string())];
    if !mc.is_empty() {
        q.push(("gameVersion", mc.to_string()));
    }
    if let Some(t) = cf_loader(loader) {
        q.push(("modLoaderType", t.to_string()));
    }
    let v = cf_get(c, key, &format!("/v1/mods/{}/files", pid), &q)
        .await
        .ok()?;
    let mut files = v["data"].as_array()?.clone();
    files.sort_by(|a, b| {
        let da = a["fileDate"].as_str().unwrap_or("");
        let db = b["fileDate"].as_str().unwrap_or("");
        db.cmp(da)
    });
    let f = files
        .iter()
        .find(|x| x["releaseType"].as_u64() == Some(1))
        .or(files.first())?;
    f["fileName"].as_str().map(|s| s.to_string())
}

fn content_folder(kind: &str) -> Result<&'static str, String> {
    match kind {
        "mod" => Ok("mods"),
        "resourcepack" => Ok("resourcepacks"),
        "shader" => Ok("shaderpacks"),
        _ => Err("Для этого раздела обновления не проверяются".to_string()),
    }
}

/// Ищет более новые версии установленных модов, ресурспаков или шейдеров
#[tauri::command]
pub async fn content_check_updates(
    instance_id: String,
    kind: String,
    cf_key: String,
) -> Result<Vec<Value>, String> {
    let inst = instances::load_instance(&instance_id)?;
    let folder = content_folder(&kind)?;
    let c = client()?;
    let loader = if kind == "mod" && inst.loader != "vanilla" {
        inst.loader.clone()
    } else {
        String::new()
    };
    let dir = instances::dir_of(&instance_id).join(folder);
    let recs: Vec<Value> = instances::read_records(&instance_id)
        .into_iter()
        .filter(|r| {
            r["kind"].as_str() == Some(kind.as_str())
                && r["project_id"].is_string()
                && r["source"].is_string()
        })
        .collect();
    let mc = inst.mc_version.clone();
    let results: Vec<Option<Value>> = futures_util::stream::iter(recs.into_iter().map(|r| {
        let c = c.clone();
        let mc = mc.clone();
        let loader = loader.clone();
        let key = cf_key.clone();
        let dir = dir.clone();
        async move {
            let file = r["file_name"].as_str()?.to_string();
            let present =
                dir.join(&file).exists() || dir.join(format!("{}.disabled", file)).exists();
            if !present {
                return None;
            }
            let pid = r["project_id"].as_str()?.to_string();
            let src = r["source"].as_str()?.to_string();
            let latest = if src == "modrinth" {
                mr_latest_name(&c, &mc, &pid, &loader).await?
            } else {
                cf_latest_name(&c, &key, &mc, &pid, &loader).await?
            };
            if latest == file {
                return None;
            }
            Some(json!({
                "file_name": file,
                "title": r["title"],
                "project_id": pid,
                "source": src,
                "latest": latest
            }))
        }
    }))
    .buffer_unordered(8)
    .collect()
    .await;
    Ok(results.into_iter().flatten().collect())
}

/// Ставит свежую версию и удаляет старый файл
#[tauri::command]
pub async fn content_update(
    app: AppHandle,
    instance_id: String,
    kind: String,
    source: String,
    project_id: String,
    old_file: String,
    cf_key: String,
) -> Result<String, String> {
    instances::safe_file(&old_file)?;
    let folder = content_folder(&kind)?;
    let inst = instances::load_instance(&instance_id)?;
    let c = client()?;
    let loader = if kind == "mod" && inst.loader != "vanilla" {
        inst.loader.clone()
    } else {
        String::new()
    };
    let none: Option<String> = None;
    let it = if source == "modrinth" {
        mr_install(&c, &inst, &kind, &project_id, &loader, &none).await?
    } else {
        cf_install(&c, &cf_key, &inst, &kind, &project_id, &loader, &none).await?
    };
    if it.saved != old_file {
        let dir = instances::dir_of(&instance_id).join(folder);
        let old_dis = dir.join(format!("{}.disabled", old_file));
        let was_disabled = old_dis.exists();
        let _ = std::fs::remove_file(dir.join(&old_file));
        let _ = std::fs::remove_file(&old_dis);
        instances::remove_record(&instance_id, &old_file);
        if was_disabled {
            let _ = std::fs::rename(
                dir.join(&it.saved),
                dir.join(format!("{}.disabled", it.saved)),
            );
        }
    }
    progress(&app, "Готово", 1.0);
    Ok(it.title)
}

// ---------------------------------------------------------------------------
//  Экспорт сборки в .mrpack
// ---------------------------------------------------------------------------

fn add_dir<W: std::io::Write + std::io::Seek>(
    zw: &mut zip::ZipWriter<W>,
    dir: &Path,
    prefix: &str,
    opts: zip::write::SimpleFileOptions,
    count: &mut u32,
) -> Result<(), String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for en in rd.flatten() {
        let name = en.file_name().to_string_lossy().to_string();
        let path = en.path();
        let rel = format!("{}/{}", prefix, name);
        if path.is_dir() {
            add_dir(zw, &path, &rel, opts.clone(), count)?;
        } else {
            zw.start_file(rel, opts.clone()).map_err(e)?;
            let mut fh = std::fs::File::open(&path).map_err(e)?;
            std::io::copy(&mut fh, zw).map_err(e)?;
            *count += 1;
        }
    }
    Ok(())
}

/// Сохраняет сборку в файл .mrpack: его можно отправить другу и импортировать
#[tauri::command]
pub async fn export_instance(
    instance_id: String,
    dest: String,
    include_worlds: bool,
) -> Result<String, String> {
    let inst = instances::load_instance(&instance_id)?;
    let idir = instances::dir_of(&instance_id);
    let mut dest = PathBuf::from(dest.trim().trim_matches('"'));
    let is_mrpack = dest
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.eq_ignore_ascii_case("mrpack"))
        .unwrap_or(false);
    if !is_mrpack {
        dest.set_extension("mrpack");
    }
    tokio::task::spawn_blocking(move || -> Result<String, String> {
        use std::io::Write;
        let file = std::fs::File::create(&dest).map_err(e)?;
        let mut zw = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut deps = serde_json::Map::new();
        deps.insert("minecraft".to_string(), json!(inst.mc_version));
        match inst.loader.as_str() {
            "fabric" => {
                deps.insert("fabric-loader".to_string(), json!(inst.loader_version));
            }
            "quilt" => {
                deps.insert("quilt-loader".to_string(), json!(inst.loader_version));
            }
            "neoforge" => {
                deps.insert("neoforge".to_string(), json!(inst.loader_version));
            }
            "forge" => {
                let v = inst
                    .loader_version
                    .splitn(2, '-')
                    .nth(1)
                    .unwrap_or(inst.loader_version.as_str())
                    .to_string();
                deps.insert("forge".to_string(), json!(v));
            }
            _ => {}
        }
        let index = json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": inst.name,
            "summary": "Экспорт из MineLauncher",
            "files": [],
            "dependencies": deps
        });
        zw.start_file("modrinth.index.json", opts.clone())
            .map_err(e)?;
        zw.write_all(serde_json::to_string_pretty(&index).map_err(e)?.as_bytes())
            .map_err(e)?;
        let mut count: u32 = 0;
        let mut dirs = vec!["mods", "config", "resourcepacks", "shaderpacks"];
        if include_worlds {
            dirs.push("saves");
        }
        for d in dirs {
            add_dir(
                &mut zw,
                &idir.join(d),
                &format!("overrides/{}", d),
                opts.clone(),
                &mut count,
            )?;
        }
        for f in ["options.txt", "servers.dat", "minelauncher.content.json"] {
            let p = idir.join(f);
            if p.is_file() {
                zw.start_file(format!("overrides/{}", f), opts.clone())
                    .map_err(e)?;
                let mut fh = std::fs::File::open(&p).map_err(e)?;
                std::io::copy(&mut fh, &mut zw).map_err(e)?;
                count += 1;
            }
        }
        zw.finish().map_err(e)?;
        Ok(format!(
            "Сохранено в {} ({} файлов)",
            dest.to_string_lossy(),
            count
        ))
    })
    .await
    .map_err(e)?
}

/// Проверяет моды Modrinth: каких обязательных зависимостей не хватает и какие моды конфликтуют
#[tauri::command]
pub async fn content_check_deps(instance_id: String) -> Result<Value, String> {
    let inst = instances::load_instance(&instance_id)?;
    let c = client()?;
    let loader = if inst.loader != "vanilla" {
        inst.loader.clone()
    } else {
        String::new()
    };
    let dir = instances::dir_of(&instance_id).join("mods");
    let recs: Vec<Value> = instances::read_records(&instance_id)
        .into_iter()
        .filter(|r| {
            r["kind"].as_str() == Some("mod")
                && r["source"].as_str() == Some("modrinth")
                && r["project_id"].is_string()
        })
        .filter(|r| {
            // отключённые моды в проверке не участвуют
            r["file_name"]
                .as_str()
                .map(|f| dir.join(f).exists())
                .unwrap_or(false)
        })
        .collect();
    let installed: HashSet<String> = instances::read_records(&instance_id)
        .into_iter()
        .filter_map(|r| r["project_id"].as_str().map(|s| s.to_string()))
        .collect();
    let mc = inst.mc_version.clone();
    let found: Vec<(String, String, String, String)> =
        futures_util::stream::iter(recs.into_iter().map(|r| {
            let c = c.clone();
            let mc = mc.clone();
            let loader = loader.clone();
            async move {
                let mut out: Vec<(String, String, String, String)> = vec![];
                let pid = r["project_id"].as_str().unwrap_or("").to_string();
                let title = r["title"].as_str().unwrap_or("").to_string();
                let mut q: Vec<(&str, String)> = vec![];
                if !mc.is_empty() {
                    q.push(("game_versions", format!("[\"{}\"]", mc)));
                }
                if !loader.is_empty() {
                    q.push(("loaders", format!("[\"{}\"]", loader)));
                }
                if let Ok(v) = mr_get(&c, &format!("/project/{}/version", pid), &q).await {
                    if let Some(ver) = v.as_array().and_then(|a| a.first()) {
                        for d in ver["dependencies"].as_array().cloned().unwrap_or_default() {
                            let ty = d["dependency_type"].as_str().unwrap_or("");
                            if let Some(dp) = d["project_id"].as_str() {
                                if ty == "required" || ty == "incompatible" {
                                    out.push((
                                        ty.to_string(),
                                        title.clone(),
                                        pid.clone(),
                                        dp.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
                out
            }
        }))
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect();
    // названия проектов-зависимостей
    let mut ids: Vec<String> = found.iter().map(|x| x.3.clone()).collect();
    ids.sort();
    ids.dedup();
    let mut names: std::collections::HashMap<String, String> = Default::default();
    if !ids.is_empty() {
        let arr = serde_json::to_string(&ids).map_err(e)?;
        if let Ok(v) = mr_get(&c, "/projects", &[("ids", arr)]).await {
            for p in v.as_array().cloned().unwrap_or_default() {
                if let (Some(id), Some(t)) = (p["id"].as_str(), p["title"].as_str()) {
                    names.insert(id.to_string(), t.to_string());
                }
            }
        }
    }
    let mut missing: Vec<Value> = vec![];
    let mut conflicts: Vec<Value> = vec![];
    for (ty, who, who_id, dep) in found {
        let dep_name = names.get(&dep).cloned().unwrap_or_else(|| dep.clone());
        if ty == "required" && !installed.contains(&dep) {
            missing.push(json!({ "mod": who, "needs": dep_name, "project_id": dep }));
        } else if ty == "incompatible" && installed.contains(&dep) && dep != who_id {
            conflicts.push(json!({ "mod": who, "with": dep_name }));
        }
    }
    Ok(json!({ "missing": missing, "conflicts": conflicts }))
}

// ---------------------------------------------------------------------------
//  Ускорение игры, смена версии и перенос модов
// ---------------------------------------------------------------------------

/// Ставит проверенные моды для скорости (Modrinth, без ключей и регистрации)
#[tauri::command]
pub async fn boost_instance(app: AppHandle, instance_id: String) -> Result<Value, String> {
    let inst = instances::load_instance(&instance_id)?;
    let slugs: &[&str] = match inst.loader.as_str() {
        "fabric" | "quilt" => &[
            "sodium",
            "lithium",
            "ferrite-core",
            "modernfix",
            "immediatelyfast",
            "entityculling",
        ],
        "forge" => &["embeddium", "ferrite-core", "modernfix"],
        "neoforge" => &["sodium", "ferrite-core", "modernfix", "immediatelyfast"],
        _ => {
            return Err(
                "Для ванильной сборки нет модов-ускорителей. Создай сборку с Fabric, это самый простой вариант."
                    .to_string(),
            )
        }
    };
    let c = client()?;
    let installed: HashSet<String> = instances::read_records(&instance_id)
        .into_iter()
        .filter_map(|r| r["project_id"].as_str().map(|s| s.to_string()))
        .collect();
    let (mut added, mut skipped, mut present) = (vec![], vec![], vec![]);
    for slug in slugs {
        let proj = match mr_get(&c, &format!("/project/{}", slug), &[]).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let title = proj["title"].as_str().unwrap_or(slug).to_string();
        if proj["id"]
            .as_str()
            .map(|i| installed.contains(i))
            .unwrap_or(false)
        {
            present.push(title);
            continue;
        }
        match catalog_install(
            app.clone(),
            instance_id.clone(),
            "modrinth".to_string(),
            "mod".to_string(),
            slug.to_string(),
            String::new(),
            None,
        )
        .await
        {
            Ok(_) => added.push(title),
            Err(_) => skipped.push(title),
        }
    }
    progress(&app, "Готово", 1.0);
    Ok(json!({ "added": added, "skipped": skipped, "present": present }))
}

/// Меняет версию игры (и при желании версию загрузчика). Загрузчик скачается при запуске.
#[tauri::command]
pub fn change_version(
    instance_id: String,
    mc_version: String,
    loader: String,
    loader_version: String,
) -> Result<instances::Instance, String> {
    let mut i = instances::load_instance(&instance_id)?;
    if mc_version.trim().is_empty() {
        return Err("Выбери версию игры".to_string());
    }
    if !["vanilla", "fabric", "quilt", "forge", "neoforge"].contains(&loader.as_str()) {
        return Err("Неизвестный загрузчик".to_string());
    }
    i.loader = loader;
    i.mc_version = mc_version.trim().to_string();
    i.loader_version = loader_version.trim().to_string();
    i.profile_id = String::new();
    instances::save_instance(&i)?;
    Ok(i)
}

/// Для каждого мода из каталога смотрит, есть ли версия под текущую игру и загрузчик
#[tauri::command]
pub async fn content_migrate_plan(
    instance_id: String,
    cf_key: String,
) -> Result<Vec<Value>, String> {
    let inst = instances::load_instance(&instance_id)?;
    let c = client()?;
    let loader = if inst.loader != "vanilla" {
        inst.loader.clone()
    } else {
        String::new()
    };
    let dir = instances::dir_of(&instance_id).join("mods");
    let recs: Vec<Value> = instances::read_records(&instance_id)
        .into_iter()
        .filter(|r| {
            r["kind"].as_str() == Some("mod")
                && r["project_id"].is_string()
                && r["source"].is_string()
        })
        .collect();
    let mc = inst.mc_version.clone();
    let out: Vec<Option<Value>> = futures_util::stream::iter(recs.into_iter().map(|r| {
        let c = c.clone();
        let mc = mc.clone();
        let loader = loader.clone();
        let key = cf_key.clone();
        let dir = dir.clone();
        async move {
            let file = r["file_name"].as_str()?.to_string();
            let on = dir.join(&file).exists();
            let off = dir.join(format!("{}.disabled", file)).exists();
            if !on && !off {
                return None;
            }
            let pid = r["project_id"].as_str()?.to_string();
            let src = r["source"].as_str()?.to_string();
            let latest = if src == "modrinth" {
                mr_latest_name(&c, &mc, &pid, &loader).await
            } else {
                cf_latest_name(&c, &key, &mc, &pid, &loader).await
            };
            Some(json!({
                "file_name": file,
                "title": r["title"],
                "project_id": pid,
                "source": src,
                "latest": latest,
                "enabled": on
            }))
        }
    }))
    .buffer_unordered(8)
    .collect()
    .await;
    Ok(out.into_iter().flatten().collect())
}
