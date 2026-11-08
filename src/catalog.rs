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
    Ok(Installed { title, deps })
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
    Ok(Installed { title, deps })
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
