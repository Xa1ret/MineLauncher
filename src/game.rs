//! Запуск игры для выбранной сборки (с учётом загрузчика модов).

use crate::instances::{self};
use crate::{
    auth, collect_args, download, e, ensure_java, extract_natives, loaders, os_name, progress,
    root, rules_allow, subst, ASSETS_URL, MANIFEST,
};
use futures_util::StreamExt;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

/// JSON ванильной версии (из кэша или с серверов Mojang)
async fn vanilla_json(c: &reqwest::Client, base: &Path, id: &str) -> Result<Value, String> {
    let p = base.join("versions").join(id).join(format!("{}.json", id));
    if let Ok(s) = std::fs::read_to_string(&p) {
        if let Ok(v) = serde_json::from_str::<Value>(&s) {
            return Ok(v);
        }
    }
    let m: Value = c
        .get(MANIFEST)
        .send()
        .await
        .map_err(|_| "Нет связи с серверами Mojang".to_string())?
        .json()
        .await
        .map_err(e)?;
    let url = m["versions"]
        .as_array()
        .and_then(|v| v.iter().find(|x| x["id"].as_str() == Some(id)))
        .and_then(|x| x["url"].as_str())
        .ok_or_else(|| "Версия не найдена".to_string())?
        .to_string();
    download(c, &url, &p).await?;
    let text = std::fs::read_to_string(&p).map_err(e)?;
    serde_json::from_str(&text).map_err(e)
}

/// Склеивает профиль загрузчика (child) с ванильной версией (parent)
fn merge_versions(parent: &Value, child: &Value) -> Value {
    let mut out = parent.clone();
    if let (Some(o), Some(c)) = (out.as_object_mut(), child.as_object()) {
        for (k, v) in c {
            match k.as_str() {
                "inheritsFrom" => {}
                "libraries" => {
                    let mut libs = v.as_array().cloned().unwrap_or_default();
                    libs.extend(parent["libraries"].as_array().cloned().unwrap_or_default());
                    o.insert(k.clone(), Value::Array(libs));
                }
                "arguments" => {
                    let mut args: Map<String, Value> = Map::new();
                    for part in ["game", "jvm"] {
                        let mut arr = parent["arguments"][part]
                            .as_array()
                            .cloned()
                            .unwrap_or_default();
                        arr.extend(v[part].as_array().cloned().unwrap_or_default());
                        args.insert(part.to_string(), Value::Array(arr));
                    }
                    o.insert(k.clone(), Value::Object(args));
                }
                _ => {
                    o.insert(k.clone(), v.clone());
                }
            }
        }
    }
    out
}

/// group/artifact/version[:classifier][@ext] -> путь в maven-хранилище
fn maven_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let (version, ext) = match parts[2].split_once('@') {
        Some((v, x)) => (v, x),
        None => (parts[2], "jar"),
    };
    let file = match parts.get(3) {
        Some(cl) => format!("{}-{}-{}.{}", artifact, version, cl, ext),
        None => format!("{}-{}.{}", artifact, version, ext),
    };
    Some(format!("{}/{}/{}/{}", group, artifact, version, file))
}

/// Ключ без версии: группа:артефакт[:классификатор] — чтобы не дублировать библиотеки
fn lib_key(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() >= 4 {
        format!("{}:{}:{}", parts[0], parts[1], parts[3])
    } else if parts.len() >= 2 {
        format!("{}:{}", parts[0], parts[1])
    } else {
        name.to_string()
    }
}

#[tauri::command]
pub async fn launch(
    app: AppHandle,
    instance_id: String,
    java_path: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    fullscreen: Option<bool>,
    detach: Option<bool>,
    server: Option<String>,
) -> Result<(), String> {
    let mut inst = instances::load_instance(&instance_id)?;
    let client = reqwest::Client::builder()
        .user_agent("MineLauncher/0.3")
        .build()
        .map_err(e)?;
    let base = root();

    progress(&app, "Проверяю аккаунт…", 0.01);
    let acc = auth::fresh_account(&client).await?;

    // 1. Версия игры и Java
    progress(&app, "Получаю данные версии…", 0.03);
    let vanilla = vanilla_json(&client, &base, &inst.mc_version).await?;
    let need = vanilla["javaVersion"]["majorVersion"].as_u64().unwrap_or(8);
    let own_java = if inst.java_path.trim().is_empty() {
        java_path
    } else {
        Some(inst.java_path.clone())
    };
    let java = match own_java.filter(|s| !s.trim().is_empty()) {
        Some(p) => p,
        None => {
            let comp = vanilla["javaVersion"]["component"]
                .as_str()
                .unwrap_or("jre-legacy");
            ensure_java(&app, &client, &base, comp)
                .await?
                .to_string_lossy()
                .to_string()
        }
    };

    // 2. Клиент игры (нужен и установщикам Forge/NeoForge)
    let client_jar = base
        .join("versions")
        .join(&inst.mc_version)
        .join(format!("{}.jar", inst.mc_version));
    let client_url = vanilla["downloads"]["client"]["url"]
        .as_str()
        .ok_or_else(|| "У версии нет клиента".to_string())?;
    progress(&app, "Скачиваю игру…", 0.04);
    download(&client, client_url, &client_jar).await?;

    // 3. Загрузчик модов
    let profile_id = loaders::ensure_profile(&app, &client, &base, &mut inst, &java).await?;
    let version: Value = if profile_id == inst.mc_version {
        vanilla.clone()
    } else {
        let p = base
            .join("versions")
            .join(&profile_id)
            .join(format!("{}.json", profile_id));
        let text = std::fs::read_to_string(&p).map_err(e)?;
        let child: Value = serde_json::from_str(&text).map_err(e)?;
        merge_versions(&vanilla, &child)
    };

    // 4. Библиотеки
    let mut jobs: Vec<(String, PathBuf)> = vec![];
    let mut classpath: Vec<PathBuf> = vec![];
    let mut natives: Vec<PathBuf> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    let libs_dir = base.join("libraries");

    for lib in version["libraries"].as_array().cloned().unwrap_or_default() {
        if !rules_allow(lib.get("rules")) {
            continue;
        }
        let name = lib["name"].as_str().unwrap_or("").to_string();
        if !name.is_empty() && !seen.insert(lib_key(&name)) {
            continue;
        }
        let mut artifact: Option<(String, String)> = None;
        if let Some(path) = lib["downloads"]["artifact"]["path"].as_str() {
            let url = lib["downloads"]["artifact"]["url"]
                .as_str()
                .unwrap_or("")
                .to_string();
            artifact = Some((path.to_string(), url));
        } else if let Some(path) = maven_path(&name) {
            let url = match lib["url"].as_str() {
                Some(b) => format!("{}/{}", b.trim_end_matches('/'), path),
                None => String::new(),
            };
            artifact = Some((path, url));
        }
        if let Some((path, url)) = artifact {
            let p = libs_dir.join(&path);
            if !url.is_empty() {
                jobs.push((url, p.clone()));
            }
            classpath.push(p);
        }
        // старый формат нативных библиотек
        if let Some(classifier) = lib["natives"][os_name()].as_str() {
            let classifier = classifier.replace("${arch}", "64");
            let a = &lib["downloads"]["classifiers"][classifier.as_str()];
            if let (Some(path), Some(url)) = (a["path"].as_str(), a["url"].as_str()) {
                let p = libs_dir.join(path);
                jobs.push((url.to_string(), p.clone()));
                natives.push(p);
            }
        }
    }
    classpath.push(client_jar.clone());

    // 5. Ресурсы (звуки, текстуры, языки)
    let assets_id = version["assetIndex"]["id"]
        .as_str()
        .unwrap_or("legacy")
        .to_string();
    let assets_url = version["assetIndex"]["url"]
        .as_str()
        .ok_or_else(|| "Нет индекса ресурсов".to_string())?;
    let index_path = base
        .join("assets")
        .join("indexes")
        .join(format!("{}.json", assets_id));
    download(&client, assets_url, &index_path).await?;
    let idx: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).map_err(e)?).map_err(e)?;
    if let Some(objs) = idx["objects"].as_object() {
        for (_, o) in objs {
            if let Some(hash) = o["hash"].as_str() {
                let pre = &hash[0..2];
                jobs.push((
                    format!("{}/{}/{}", ASSETS_URL, pre, hash),
                    base.join("assets").join("objects").join(pre).join(hash),
                ));
            }
        }
    }

    // 6. Параллельная загрузка
    let total = jobs.len().max(1);
    let done = Arc::new(AtomicUsize::new(0));
    let results: Vec<Result<(), String>> = futures_util::stream::iter(jobs)
        .map(|(url, path)| {
            let c = client.clone();
            let app = app.clone();
            let done = done.clone();
            async move {
                let r = download(&c, &url, &path).await;
                let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                if d % 15 == 0 || d == total {
                    let pct = 0.1 + 0.85 * (d as f64 / total as f64);
                    progress(&app, &format!("Загрузка файлов: {} / {}", d, total), pct);
                }
                r
            }
        })
        .buffer_unordered(16)
        .collect()
        .await;
    if let Some(Err(err)) = results.into_iter().find(|r| r.is_err()) {
        return Err(format!("Ошибка загрузки: {}", err));
    }

    progress(&app, "Распаковываю библиотеки…", 0.96);
    let natives_dir = base.join("versions").join(&profile_id).join("natives");
    for n in &natives {
        extract_natives(n, &natives_dir)?;
    }

    // 7. Аргументы запуска
    let game_dir = instances::dir_of(&inst.id);
    std::fs::create_dir_all(&game_dir).map_err(e)?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let cp = classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep);
    let assets_root = base.join("assets").to_string_lossy().to_string();

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", acc.name.clone());
    // Должно совпадать с именем клиентского jar (<версия>.jar): Forge/NeoForge
    // исключают его из модулей по этому имени (-DignoreList=...,${version_name}.jar)
    vars.insert("version_name", inst.mc_version.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", assets_root.clone());
    vars.insert("game_assets", assets_root);
    vars.insert("assets_index_name", assets_id);
    vars.insert("auth_uuid", acc.uuid.clone());
    vars.insert("auth_access_token", acc.access_token.clone());
    vars.insert("auth_session", acc.access_token.clone());
    vars.insert(
        "user_type",
        if acc.kind == "microsoft" {
            "msa"
        } else {
            "mojang"
        }
        .to_string(),
    );
    vars.insert("user_properties", "{}".to_string());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("version_type", "MineLauncher".to_string());
    vars.insert(
        "natives_directory",
        natives_dir.to_string_lossy().to_string(),
    );
    vars.insert("launcher_name", "MineLauncher".to_string());
    vars.insert("launcher_version", "0.3".to_string());
    vars.insert("classpath", cp.clone());
    vars.insert("library_directory", libs_dir.to_string_lossy().to_string());
    vars.insert("classpath_separator", sep.to_string());

    let mut jvm: Vec<String>;
    let game: Vec<String>;
    if version["arguments"].is_object() {
        jvm = collect_args(&version["arguments"]["jvm"], &vars);
        game = collect_args(&version["arguments"]["game"], &vars);
    } else {
        // старый формат (до 1.13)
        jvm = vec![
            format!("-Djava.library.path={}", natives_dir.display()),
            "-cp".to_string(),
            cp,
        ];
        game = version["minecraftArguments"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .map(|s| subst(s, &vars))
            .collect();
    }

    let main_class = version["mainClass"]
        .as_str()
        .ok_or_else(|| "Нет mainClass".to_string())?
        .to_string();
    let mut args: Vec<String> = vec![
        format!("-Xmx{}M", inst.ram_mb.max(1024)),
        "-Xms512M".to_string(),
    ];
    for extra in inst.jvm_args.split_whitespace() {
        args.push(extra.to_string());
    }

    // authlib-injector: скины Ely.by
    if acc.kind == "ely" {
        progress(&app, "Готовлю систему скинов…", 0.97);
        let jar = auth::ensure_injector(&client, &base).await?;
        args.push(format!("-javaagent:{}=ely.by", jar.display()));
    }
    args.append(&mut jvm);
    args.push(main_class);
    args.extend(game);
    // сразу зайти на сервер
    if let Some(sv) = server.as_ref().map(|x| x.trim()).filter(|x| !x.is_empty()) {
        let minor: u32 = inst
            .mc_version
            .split('.')
            .nth(1)
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        if inst.mc_version.starts_with("1.") && minor < 20 {
            let (h, p) = match sv.rsplit_once(':') {
                Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.to_string()),
                _ => (sv.to_string(), "25565".to_string()),
            };
            args.push("--server".to_string());
            args.push(h);
            args.push("--port".to_string());
            args.push(p);
        } else {
            args.push("--quickPlayMultiplayer".to_string());
            args.push(sv.to_string());
        }
    }
    // размер окна игры из настроек лаунчера
    if fullscreen.unwrap_or(false) {
        args.push("--fullscreen".to_string());
    } else if let (Some(w), Some(h)) = (width, height) {
        if w >= 320 && h >= 240 {
            args.push("--width".to_string());
            args.push(w.to_string());
            args.push("--height".to_string());
            args.push(h.to_string());
        }
    }

    // 8. Запуск
    progress(&app, &format!("Запускаю игру (Java {})…", need), 1.0);
    let detach = detach.unwrap_or(false);
    let mut cmd = tokio::process::Command::new(&java);
    cmd.args(&args).current_dir(&game_dir);
    if detach {
        // лаунчер закроется, поэтому вывод игры пишем в файл, а не в трубу
        let log_path = game_dir.join("launcher_log.txt");
        let f = std::fs::File::create(&log_path).map_err(e)?;
        let f2 = f.try_clone().map_err(e)?;
        cmd.stdout(Stdio::from(f)).stderr(Stdio::from(f2));
    } else {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|_| {
        format!(
            "Не удалось запустить Java. Выберите «Автоматически» в настройках Java или установите Java {}+.",
            need
        )
    })?;

    instances::touch_played(&inst.id);

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
    let app3 = app.clone();
    let id = inst.id.clone();
    let started = std::time::Instant::now();
    tokio::spawn(async move {
        let code = child.wait().await.ok().and_then(|s| s.code()).unwrap_or(-1);
        instances::add_playtime(&id, started.elapsed().as_secs());
        let _ = app3.emit("game-exit", code);
    });

    Ok(())
}
