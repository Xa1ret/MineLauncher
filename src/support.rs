//! Тех. поддержка: чат в лаунчере. Сообщение уходит на маленький сервер (Cloudflare Worker),
//! а оттуда в Telegram автору. Ответ автора возвращается обратно в чат лаунчера.
//! Регистрироваться и вводить коды человеку не нужно: лаунчер сам создаёт случайный номер устройства.

use crate::{e, root};
use serde_json::{json, Value};
use std::time::Duration;

/// Адрес сервера поддержки, например "https://minelauncher-support.ИМЯ.workers.dev"
/// Пока пусто, кнопка «Тех. поддержка» сообщит, что поддержка ещё не подключена.
const SUPPORT_URL: &str = "https://minelauncher-support.vvvzx594.workers.dev";

fn random_hex() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u64(i);
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Случайный номер этого устройства (хранится в папке лаунчера)
fn device_id() -> String {
    let p = root().join("support.json");
    if let Ok(s) = std::fs::read_to_string(&p) {
        if let Ok(v) = serde_json::from_str::<Value>(&s) {
            if let Some(d) = v["device"].as_str() {
                if d.len() == 32 && d.chars().all(|c| c.is_ascii_hexdigit()) {
                    return d.to_string();
                }
            }
        }
    }
    let d = random_hex();
    let _ = std::fs::create_dir_all(root());
    let _ = std::fs::write(&p, json!({ "device": d }).to_string());
    d
}

fn base() -> Result<String, String> {
    let u = SUPPORT_URL.trim().trim_end_matches('/');
    if u.is_empty() {
        Err("Поддержка ещё не подключена".to_string())
    } else {
        Ok(u.to_string())
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("MineLauncher/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(e)
}

fn os_line() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

#[tauri::command]
pub fn support_info() -> Value {
    json!({
        "configured": !SUPPORT_URL.trim().is_empty(),
        "version": env!("CARGO_PKG_VERSION"),
        "os": os_line(),
    })
}

fn tail(s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut cut = s.len() - max;
    while !s.is_char_boundary(cut) {
        cut += 1;
    }
    s[cut..].to_string()
}

/// Отправляет сообщение в поддержку.
/// attach: необязательная диагностика (лог последнего запуска), человек сам ставит галочку.
#[tauri::command]
pub async fn support_send(
    name: String,
    text: String,
    context: Option<String>,
    attach: Option<String>,
) -> Result<Value, String> {
    let url = base()?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Напиши сообщение".to_string());
    }
    if text.chars().count() > 2000 {
        return Err("Сообщение слишком длинное (максимум 2000 знаков)".to_string());
    }
    let name: String = name.trim().chars().take(40).collect();
    let body = json!({
        "device": device_id(),
        "name": if name.is_empty() { "Без имени".to_string() } else { name },
        "text": text,
        "context": context.unwrap_or_default().chars().take(400).collect::<String>(),
        "attach": tail(attach.unwrap_or_default(), 150_000),
        "version": env!("CARGO_PKG_VERSION"),
        "os": os_line(),
    });
    let r = client()?
        .post(format!("{}/api/send", url))
        .json(&body)
        .send()
        .await
        .map_err(|_| "Нет связи с поддержкой. Проверь интернет".to_string())?;
    let code = r.status().as_u16();
    let v: Value = r.json().await.unwrap_or_else(|_| json!({}));
    match code {
        200..=299 => Ok(v),
        429 => Err("Слишком часто. Подожди немного и напиши ещё раз".to_string()),
        403 => Err("Поддержка не принимает сообщения с этого устройства".to_string()),
        _ => Err(v["error"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Ошибка сервера поддержки ({})", code))),
    }
}

/// Забирает новые ответы. after: номер последнего уже полученного ответа.
#[tauri::command]
pub async fn support_poll(after: i64) -> Result<Value, String> {
    let url = base()?;
    let r = client()?
        .get(format!("{}/api/poll", url))
        .query(&[("d", device_id()), ("after", after.max(0).to_string())])
        .send()
        .await
        .map_err(|_| "Нет связи с поддержкой".to_string())?;
    if !r.status().is_success() {
        return Err(format!(
            "Ошибка сервера поддержки ({})",
            r.status().as_u16()
        ));
    }
    r.json::<Value>().await.map_err(e)
}
