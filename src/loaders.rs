//! Загрузчики модов: Fabric, Quilt, Forge, NeoForge.

use crate::instances::{self, Instance};
use crate::{download, e, progress};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("MineLauncher/0.3")
        .build()
        .map_err(e)
}

fn xml_versions(xml: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = xml;
    while let Some(a) = rest.find("<version>") {
        let after = &rest[a + 9..];
        match after.find("</version>") {
            Some(b) => {
                out.push(after[..b].trim().to_string());
                rest = &after[b + 10..];
            }
            None => break,
        }
    }
    out
}

/// Список версий загрузчика для выбранной версии игры (лучшая — первой)
#[tauri::command]
pub async fn loader_versions(kind: String, mc: String) -> Result<Vec<String>, String> {
    let c = http()?;
    match kind.as_str() {
        "fabric" | "quilt" => {
            let url = if kind == "fabric" {
                format!("https://meta.fabricmc.net/v2/versions/loader/{}", mc)
            } else {
                format!("https://meta.quiltmc.org/v3/versions/loader/{}", mc)
            };
            let v: Value = c
                .get(url)
                .send()
                .await
                .map_err(|_| "Нет связи с сервером загрузчика".to_string())?
                .json()
                .await
                .map_err(e)?;
            let mut stable: Vec<String> = vec![];
            let mut other: Vec<String> = vec![];
            for it in v.as_array().cloned().unwrap_or_default() {
                let ver = it["loader"]["version"].as_str().unwrap_or("").to_string();
                if ver.is_empty() {
                    continue;
                }
                if it["loader"]["stable"].as_bool().unwrap_or(true) {
                    stable.push(ver);
                } else {
                    other.push(ver);
                }
            }
            stable.extend(other);
            Ok(stable)
        }
        "forge" => {
            let xml = c
                .get("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")
                .send()
                .await
                .map_err(|_| "Нет связи с сервером Forge".to_string())?
                .text()
                .await
                .map_err(e)?;
            let prefix = format!("{}-", mc);
            let mut list: Vec<String> = xml_versions(&xml)
                .into_iter()
                .filter(|v| v.starts_with(&prefix))
                .collect();
            list.reverse();
            // рекомендованная версия — первой
            if let Ok(resp) = c
                .get("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
                .send()
                .await
            {
                if let Ok(p) = resp.json::<Value>().await {
                    let rec_key = format!("{}-recommended", mc);
                    let last_key = format!("{}-latest", mc);
                    let pick = p["promos"][rec_key.as_str()]
                        .as_str()
                        .or_else(|| p["promos"][last_key.as_str()].as_str());
                    if let Some(rec) = pick {
                        let full = format!("{}-{}", mc, rec);
                        if let Some(pos) = list.iter().position(|v| *v == full) {
                            let item = list.remove(pos);
                            list.insert(0, item);
                        }
                    }
                }
            }
            Ok(list)
        }
        "neoforge" => {
            let v: Value = c
                .get("https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge")
                .send()
                .await
                .map_err(|_| "Нет связи с сервером NeoForge".to_string())?
                .json()
                .await
                .map_err(e)?;
            // Старая схема: 1.21.1 -> 21.1.x. Новая (26.x): 26.3 -> 26.3.0.x, 26.1.2 -> 26.1.2.x
            let prefix = if mc.starts_with("1.") {
                let short = mc.trim_start_matches("1.").to_string();
                if short.matches('.').count() == 0 {
                    format!("{}.0.", short)
                } else {
                    format!("{}.", short)
                }
            } else if mc.matches('.').count() == 1 {
                format!("{}.0.", mc)
            } else {
                format!("{}.", mc)
            };
            let mut stable: Vec<String> = vec![];
            let mut pre: Vec<String> = vec![];
            let all = v["versions"].as_array().cloned().unwrap_or_default();
            for x in all.iter().rev() {
                if let Some(s) = x.as_str() {
                    if s.starts_with(&prefix) {
                        if s.contains("beta") || s.contains("alpha") {
                            pre.push(s.to_string());
                        } else {
                            stable.push(s.to_string());
                        }
                    }
                }
            }
            stable.extend(pre);
            Ok(stable)
        }
        _ => Ok(vec![]),
    }
}

fn list_versions(base: &Path) -> Vec<String> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(base.join("versions")) {
        for en in rd.flatten() {
            if en.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                out.push(en.file_name().to_string_lossy().to_string());
            }
        }
    }
    out
}

/// Устанавливает загрузчик (если ещё не установлен) и возвращает id профиля.
/// Ванильная версия (json и jar) к этому моменту уже скачана.
pub async fn ensure_profile(
    app: &AppHandle,
    c: &reqwest::Client,
    base: &Path,
    inst: &mut Instance,
    java: &str,
) -> Result<String, String> {
    if inst.loader == "vanilla" {
        return Ok(inst.mc_version.clone());
    }
    if !inst.profile_id.is_empty() {
        let p = base
            .join("versions")
            .join(&inst.profile_id)
            .join(format!("{}.json", inst.profile_id));
        if p.exists() {
            return Ok(inst.profile_id.clone());
        }
    }
    if inst.loader_version.is_empty() {
        let list = loader_versions(inst.loader.clone(), inst.mc_version.clone()).await?;
        inst.loader_version = list
            .first()
            .cloned()
            .ok_or_else(|| "Для этой версии игры нет такого загрузчика".to_string())?;
    }

    let id = match inst.loader.as_str() {
        "fabric" | "quilt" => {
            progress(app, "Скачиваю загрузчик…", 0.05);
            let url = if inst.loader == "fabric" {
                format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
                    inst.mc_version, inst.loader_version
                )
            } else {
                format!(
                    "https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json",
                    inst.mc_version, inst.loader_version
                )
            };
            let text = c
                .get(&url)
                .send()
                .await
                .map_err(|_| "Нет связи с сервером загрузчика".to_string())?
                .error_for_status()
                .map_err(|_| "Такой версии загрузчика нет".to_string())?
                .text()
                .await
                .map_err(e)?;
            let v: Value = serde_json::from_str(&text).map_err(e)?;
            let id = v["id"]
                .as_str()
                .ok_or_else(|| "В профиле нет id".to_string())?
                .to_string();
            let dir = base.join("versions").join(&id);
            std::fs::create_dir_all(&dir).map_err(e)?;
            std::fs::write(dir.join(format!("{}.json", id)), text).map_err(e)?;
            id
        }
        "forge" | "neoforge" => run_installer(app, c, base, inst, java).await?,
        _ => return Err("Неизвестный загрузчик".to_string()),
    };
    inst.profile_id = id.clone();
    instances::save_instance(inst)?;
    Ok(id)
}

async fn run_installer(
    app: &AppHandle,
    c: &reqwest::Client,
    base: &Path,
    inst: &Instance,
    java: &str,
) -> Result<String, String> {
    let lv = inst.loader_version.clone();
    let (url, fname) = if inst.loader == "forge" {
        (
            format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
                lv, lv
            ),
            format!("forge-{}-installer.jar", lv),
        )
    } else {
        (
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
                lv, lv
            ),
            format!("neoforge-{}-installer.jar", lv),
        )
    };
    progress(app, "Скачиваю установщик загрузчика…", 0.05);
    let jar = base.join("installers").join(&fname);
    download(c, &url, &jar).await?;

    // установщику нужен этот файл в папке игры
    let lp = base.join("launcher_profiles.json");
    if !lp.exists() {
        std::fs::write(&lp, "{\"profiles\":{},\"version\":3}").map_err(e)?;
    }

    let before = list_versions(base);
    progress(
        app,
        "Устанавливаю загрузчик. Это может занять несколько минут…",
        0.06,
    );
    let mut inst_cmd = tokio::process::Command::new(java);
    // без чёрного окна консоли Java
    #[cfg(windows)]
    inst_cmd.creation_flags(0x08000000);
    let mut child = inst_cmd
        .arg("-jar")
        .arg(&jar)
        .arg("--installClient")
        .arg(base)
        .current_dir(base)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Не удалось запустить Java для установщика".to_string())?;
    if let Some(out) = child.stdout.take() {
        let app2 = app.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = app2.emit("log", l);
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        let app2 = app.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = app2.emit("log", l);
            }
        });
    }
    let status = child.wait().await.map_err(e)?;
    if !status.success() {
        return Err(
            "Установщик загрузчика завершился с ошибкой. Подробности во вкладке «Консоль»"
                .to_string(),
        );
    }

    // ищем созданный профиль
    let keyword = inst.loader.clone();
    let after = list_versions(base);
    let fresh: Vec<&String> = after.iter().filter(|d| !before.contains(d)).collect();
    let pick = fresh
        .iter()
        .find(|d| d.to_lowercase().contains(&keyword))
        .or(fresh.first())
        .map(|d| d.to_string());
    if let Some(p) = pick {
        return Ok(p);
    }
    let tail = lv.splitn(2, '-').nth(1).unwrap_or(lv.as_str()).to_string();
    after
        .iter()
        .find(|d| d.to_lowercase().contains(&keyword) && d.contains(&tail))
        .cloned()
        .ok_or_else(|| "Загрузчик установился, но профиль не найден".to_string())
}
