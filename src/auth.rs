//! Аккаунты (Ely.by и Microsoft), скины, authlib-injector и поиск Java.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const ELY: &str = "https://authserver.ely.by";
const MS_SCOPE: &str = "XboxLive.signin offline_access";
const MS_DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MS_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MC_API: &str = "https://api.minecraftservices.com";

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("MineLauncher/0.2")
        .build()
        .map_err(e)
}

// ---------------------------------------------------------------------------
//  Хранение аккаунта (файл account.json в папке лаунчера)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Account {
    pub kind: String, // "ely" | "microsoft"
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    #[serde(default)]
    pub client_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub ms_client_id: String,
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub skin_url: String,
    #[serde(default)]
    pub skin_variant: String,
}

/// Что видит интерфейс — без токенов
#[derive(Serialize)]
pub struct AccountInfo {
    kind: String,
    name: String,
    uuid: String,
    skin_url: String,
    skin_variant: String,
}

impl From<&Account> for AccountInfo {
    fn from(a: &Account) -> Self {
        AccountInfo {
            kind: a.kind.clone(),
            name: a.name.clone(),
            uuid: a.uuid.clone(),
            skin_url: a.skin_url.clone(),
            skin_variant: a.skin_variant.clone(),
        }
    }
}

fn account_path() -> PathBuf {
    crate::root().join("account.json")
}

fn load() -> Option<Account> {
    serde_json::from_str(&std::fs::read_to_string(account_path()).ok()?).ok()
}

fn save(a: &Account) -> Result<(), String> {
    std::fs::create_dir_all(crate::root()).map_err(e)?;
    std::fs::write(account_path(), serde_json::to_string_pretty(a).map_err(e)?).map_err(e)
}

#[tauri::command]
pub fn get_account() -> Option<AccountInfo> {
    load().as_ref().map(AccountInfo::from)
}

#[tauri::command]
pub fn logout() {
    let _ = std::fs::remove_file(account_path());
}

// ---------------------------------------------------------------------------
//  Ely.by (Yggdrasil)
// ---------------------------------------------------------------------------

fn token_now() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let h = md5::compute(format!("mine-launcher-{}", n)).0;
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

fn ely_error(v: &Value) -> String {
    let msg = v["errorMessage"].as_str().unwrap_or("");
    let low = msg.to_lowercase();
    if low.contains("two factor") || low.contains("two-factor") {
        "Включена двухфакторная защита: введите пароль в виде «пароль:код» (например, mypass:123456)"
            .into()
    } else if low.contains("invalid credentials") || low.contains("invalid username") {
        "Неверный логин или пароль".into()
    } else if msg.is_empty() {
        "Ely.by отклонил вход".into()
    } else {
        format!("Ely.by: {}", msg)
    }
}

#[tauri::command]
pub async fn ely_login(username: String, password: String) -> Result<AccountInfo, String> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("Введите логин и пароль".into());
    }
    let c = http()?;
    let client_token = token_now();
    let r = c
        .post(format!("{}/auth/authenticate", ELY))
        .json(&json!({
            "username": username.trim(),
            "password": password,
            "clientToken": client_token,
            "requestUser": true
        }))
        .send()
        .await
        .map_err(|_| "Нет связи с Ely.by".to_string())?;
    let ok = r.status().is_success();
    let v: Value = r.json().await.unwrap_or(json!({}));
    if !ok {
        return Err(ely_error(&v));
    }
    let name = v["selectedProfile"]["name"]
        .as_str()
        .ok_or("На аккаунте Ely.by нет профиля игрока")?
        .to_string();
    let acc = Account {
        kind: "ely".into(),
        uuid: v["selectedProfile"]["id"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        access_token: v["accessToken"].as_str().unwrap_or("").to_string(),
        client_token,
        skin_url: format!("https://skinsystem.ely.by/skins/{}.png", name),
        name,
        ..Default::default()
    };
    save(&acc)?;
    Ok((&acc).into())
}

async fn ely_refresh(c: &reqwest::Client, a: &mut Account) -> Result<(), String> {
    let r = c
        .post(format!("{}/auth/refresh", ELY))
        .json(&json!({ "accessToken": a.access_token, "clientToken": a.client_token }))
        .send()
        .await
        .map_err(|_| "Нет связи с Ely.by".to_string())?;
    if !r.status().is_success() {
        return Err("Сессия Ely.by истекла. Выйдите и войдите заново.".into());
    }
    let v: Value = r.json().await.map_err(e)?;
    a.access_token = v["accessToken"].as_str().unwrap_or("").to_string();
    save(a)
}

// ---------------------------------------------------------------------------
//  Microsoft (лицензия): вход по коду на microsoft.com/link
// ---------------------------------------------------------------------------

async fn post_json(
    c: &reqwest::Client,
    url: &str,
    body: Value,
) -> Result<(reqwest::StatusCode, Value), String> {
    let r = c
        .post(url)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(e)?;
    let s = r.status();
    Ok((s, r.json().await.unwrap_or(json!({}))))
}

struct McLogin {
    token: String,
    expires_in: u64,
    id: String,
    name: String,
    skin_url: String,
    skin_variant: String,
}

/// Токен Microsoft -> Xbox Live -> XSTS -> Minecraft -> профиль
async fn ms_chain(c: &reqwest::Client, ms_token: &str) -> Result<McLogin, String> {
    let (s, xbl) = post_json(
        c,
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await?;
    if !s.is_success() {
        return Err("Xbox Live не принял вход".into());
    }
    let xbl_token = xbl["Token"].as_str().ok_or("Нет токена Xbox")?.to_string();

    let (s, xsts) = post_json(
        c,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;
    if !s.is_success() {
        return Err(match xsts["XErr"].as_u64() {
            Some(2148916233) => {
                "У этой учётной записи Microsoft нет профиля Xbox. Создайте его на xbox.com".into()
            }
            Some(2148916235) => "Xbox Live недоступен в вашей стране".into(),
            Some(2148916238) => "Это детский аккаунт: добавьте его в семью Microsoft".into(),
            _ => "Xbox не выдал разрешение".into(),
        });
    }
    let xsts_token = xsts["Token"].as_str().ok_or("Нет токена XSTS")?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or("Нет uhs")?;

    let (s, mc) = post_json(
        c,
        &format!("{}/authentication/login_with_xbox", MC_API),
        json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }),
    )
    .await?;
    if !s.is_success() {
        return Err(if s.as_u16() == 403 {
            "Mojang не разрешил этому приложению вход. Ваш Azure Client ID нужно одобрить (см. README)".into()
        } else {
            "Не удалось войти в Minecraft".into()
        });
    }
    let token = mc["access_token"]
        .as_str()
        .ok_or("Нет токена Minecraft")?
        .to_string();
    let expires_in = mc["expires_in"].as_u64().unwrap_or(86400);

    let pr = c
        .get(format!("{}/minecraft/profile", MC_API))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(e)?;
    if pr.status().as_u16() == 404 {
        return Err("На этом аккаунте нет Minecraft: Java Edition".into());
    }
    let p: Value = pr.json().await.map_err(e)?;
    let skin = p["skins"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["state"].as_str() == Some("ACTIVE")))
        .cloned()
        .unwrap_or(json!({}));
    Ok(McLogin {
        token,
        expires_in,
        id: p["id"].as_str().unwrap_or("").to_string(),
        name: p["name"].as_str().unwrap_or("").to_string(),
        skin_url: skin["url"].as_str().unwrap_or("").to_string(),
        skin_variant: skin["variant"].as_str().unwrap_or("CLASSIC").to_lowercase(),
    })
}

#[tauri::command]
pub async fn ms_login(app: AppHandle, client_id: String) -> Result<AccountInfo, String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Укажите Azure Client ID в настройках (Аккаунт → Microsoft)".into());
    }
    let c = http()?;
    let r = c
        .post(MS_DEVICE)
        .form(&[("client_id", client_id.as_str()), ("scope", MS_SCOPE)])
        .send()
        .await
        .map_err(|_| "Нет связи с Microsoft".to_string())?;
    let dc: Value = r.json().await.map_err(e)?;
    let device_code = dc["device_code"]
        .as_str()
        .ok_or_else(|| {
            format!(
                "Microsoft отклонил Client ID: {}",
                dc["error_description"].as_str().unwrap_or("проверьте его")
            )
        })?
        .to_string();
    let user_code = dc["user_code"].as_str().unwrap_or("").to_string();
    let _ = app.emit("ms-code", json!({ "code": user_code }));
    let _ = crate::open_site("ms_link".into());

    let mut interval = dc["interval"].as_u64().unwrap_or(5).max(2);
    let deadline = now() + dc["expires_in"].as_u64().unwrap_or(900);
    let tokens: Value = loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        if now() > deadline {
            return Err("Время ввода кода вышло. Попробуйте снова.".into());
        }
        let v: Value = c
            .post(MS_TOKEN)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id.as_str()),
                ("device_code", device_code.as_str()),
            ])
            .send()
            .await
            .map_err(e)?
            .json()
            .await
            .map_err(e)?;
        match v["error"].as_str() {
            None => break v,
            Some("authorization_pending") => {}
            Some("slow_down") => interval += 5,
            Some(_) => return Err("Вход отменён или отклонён".into()),
        }
    };

    let mc = ms_chain(&c, tokens["access_token"].as_str().ok_or("Нет токена")?).await?;
    let acc = Account {
        kind: "microsoft".into(),
        name: mc.name,
        uuid: mc.id,
        access_token: mc.token,
        refresh_token: tokens["refresh_token"].as_str().unwrap_or("").to_string(),
        ms_client_id: client_id,
        expires_at: now() + mc.expires_in,
        skin_url: mc.skin_url,
        skin_variant: mc.skin_variant,
        ..Default::default()
    };
    save(&acc)?;
    Ok((&acc).into())
}

/// Свежий аккаунт для запуска игры (токены обновляются при необходимости)
pub async fn fresh_account(c: &reqwest::Client) -> Result<Account, String> {
    let mut a = load().ok_or("Сначала войдите в аккаунт")?;
    if a.kind == "ely" {
        let valid = match c
            .post(format!("{}/auth/validate", ELY))
            .json(&json!({ "accessToken": a.access_token }))
            .send()
            .await
        {
            Ok(r) => r.status().is_success(),
            Err(_) => true, // нет сети — пробуем со старым токеном
        };
        if !valid {
            ely_refresh(c, &mut a).await?;
        }
    } else if now() + 300 >= a.expires_at {
        let v: Value = c
            .post(MS_TOKEN)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", a.ms_client_id.as_str()),
                ("refresh_token", a.refresh_token.as_str()),
                ("scope", MS_SCOPE),
            ])
            .send()
            .await
            .map_err(|_| "Нет связи с Microsoft".to_string())?
            .json()
            .await
            .map_err(e)?;
        let ms_token = v["access_token"]
            .as_str()
            .ok_or("Сессия Microsoft истекла. Выйдите и войдите заново.")?;
        let mc = ms_chain(c, ms_token).await?;
        a.access_token = mc.token;
        a.expires_at = now() + mc.expires_in;
        a.skin_url = mc.skin_url;
        a.skin_variant = mc.skin_variant;
        if let Some(rt) = v["refresh_token"].as_str() {
            a.refresh_token = rt.to_string();
        }
        save(&a)?;
    }
    Ok(a)
}

// ---------------------------------------------------------------------------
//  authlib-injector (нужен для скинов Ely.by)
// ---------------------------------------------------------------------------

pub async fn ensure_injector(c: &reqwest::Client, base: &Path) -> Result<PathBuf, String> {
    let jar = base.join("authlib-injector.jar");
    if jar.exists() {
        return Ok(jar);
    }
    let v: Value = c
        .get("https://authlib-injector.yushi.moe/artifact/latest.json")
        .send()
        .await
        .map_err(|_| "Не удалось скачать authlib-injector (нет сети?)".to_string())?
        .json()
        .await
        .map_err(e)?;
    let url = v["download_url"].as_str().ok_or("Нет ссылки на injector")?;
    if !url.starts_with("https://authlib-injector.yushi.moe/") {
        return Err("Подозрительная ссылка на injector".into());
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
    if let Some(expected) = v["checksums"]["sha256"].as_str() {
        let got: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        if !got.eq_ignore_ascii_case(expected) {
            return Err("Контрольная сумма injector не совпала".into());
        }
    }
    std::fs::create_dir_all(base).map_err(e)?;
    std::fs::write(&jar, &bytes).map_err(e)?;
    Ok(jar)
}

// ---------------------------------------------------------------------------
//  Скины
// ---------------------------------------------------------------------------

/// Читает PNG-скин и проверяет размер (64x64 или 64x32)
fn read_png(path: &str) -> Result<Vec<u8>, String> {
    let p = Path::new(path.trim().trim_matches('"'));
    let meta = std::fs::metadata(p).map_err(|_| "Файл не найден".to_string())?;
    if meta.len() > 200_000 {
        return Err("Файл слишком большой для скина".into());
    }
    let b = std::fs::read(p).map_err(e)?;
    if b.len() < 24 || &b[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("Это не PNG-файл".into());
    }
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    if w != 64 || (h != 64 && h != 32) {
        return Err(format!(
            "Скин должен быть 64×64 (или 64×32), а у файла {}×{}",
            w, h
        ));
    }
    Ok(b)
}

/// Для предпросмотра выбранного файла в интерфейсе
#[tauri::command]
pub fn skin_preview(path: String) -> Result<String, String> {
    let b = read_png(&path)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(b)
    ))
}

/// Загрузка скина в лицензионный аккаунт Mojang
#[tauri::command]
pub async fn skin_upload_ms(path: String, slim: bool) -> Result<(), String> {
    let c = http()?;
    let mut a = fresh_account(&c).await?;
    if a.kind != "microsoft" {
        return Err("Загрузка работает только для лицензионного аккаунта".into());
    }
    let bytes = read_png(&path)?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(e)?;
    let form = reqwest::multipart::Form::new()
        .text("variant", if slim { "slim" } else { "classic" })
        .part("file", part);
    let r = c
        .post(format!("{}/minecraft/profile/skins", MC_API))
        .bearer_auth(&a.access_token)
        .multipart(form)
        .send()
        .await
        .map_err(e)?;
    if !r.status().is_success() {
        return Err(format!(
            "Mojang не принял скин (код {})",
            r.status().as_u16()
        ));
    }
    a.skin_variant = if slim {
        "slim".into()
    } else {
        "classic".into()
    };
    let _ = save(&a);
    Ok(())
}

// ---------------------------------------------------------------------------
//  Поиск Java на компьютере
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct JavaInfo {
    path: String,
    version: String,
}

fn java_exe() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn subdirs(dir: &str) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|d| d.path()).collect())
        .unwrap_or_default()
}

fn java_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = vec![];
    if let Ok(h) = std::env::var("JAVA_HOME") {
        out.push(PathBuf::from(h).join("bin").join(java_exe()));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            out.push(d.join(java_exe()));
        }
    }
    let mut roots: Vec<String> = vec![];
    if cfg!(windows) {
        for pf in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Ok(p) = std::env::var(pf) {
                for sub in [
                    "Java",
                    "Eclipse Adoptium",
                    "Microsoft",
                    "Zulu",
                    "BellSoft",
                    "Amazon Corretto",
                    "Semeru",
                    "Programs\\Eclipse Adoptium",
                ] {
                    roots.push(format!("{}\\{}", p, sub));
                }
            }
        }
    } else {
        roots.push("/usr/lib/jvm".into());
        roots.push("/Library/Java/JavaVirtualMachines".into());
        if let Some(h) = dirs::home_dir() {
            roots.push(
                h.join(".sdkman/candidates/java")
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    for r in roots {
        for d in subdirs(&r) {
            out.push(d.join("bin").join(java_exe()));
            out.push(d.join("Contents/Home/bin").join(java_exe()));
        }
    }
    // Java, которую скачал сам лаунчер
    for d in subdirs(&crate::root().join("runtime").to_string_lossy()) {
        out.push(d.join("bin").join(java_exe()));
    }
    out
}

fn java_version(path: &Path) -> Option<String> {
    let mut cmd = std::process::Command::new(path);
    cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}

#[tauri::command]
pub async fn detect_java() -> Vec<JavaInfo> {
    tokio::task::spawn_blocking(|| {
        let mut seen = std::collections::HashSet::new();
        let mut res = vec![];
        for p in java_candidates() {
            if !p.is_file() {
                continue;
            }
            let key = std::fs::canonicalize(&p).unwrap_or(p.clone());
            if !seen.insert(key) {
                continue;
            }
            if let Some(v) = java_version(&p) {
                res.push(JavaInfo {
                    path: p.to_string_lossy().to_string(),
                    version: v,
                });
            }
        }
        res
    })
    .await
    .unwrap_or_default()
}

/// Проверка пути, который ввёл пользователь
#[tauri::command]
pub async fn check_java(path: String) -> Result<String, String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    tokio::task::spawn_blocking(move || {
        if !p.is_file() {
            return Err("Файл не найден".to_string());
        }
        java_version(&p).ok_or_else(|| "Это не Java (не удалось получить версию)".to_string())
    })
    .await
    .map_err(e)?
}
