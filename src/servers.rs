//! Список серверов: проверка онлайна.

use crate::e;
use serde_json::{json, Value};

/// Узнаёт, работает ли сервер и сколько на нём игроков (через api.mcsrvstat.us)
#[tauri::command]
pub async fn server_status(address: String) -> Result<Value, String> {
    let addr = address.trim().to_string();
    if addr.is_empty()
        || addr.len() > 100
        || !addr
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
    {
        return Err("Неверный адрес сервера".to_string());
    }
    let c = reqwest::Client::builder()
        .user_agent("MineLauncher/0.4")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(e)?;
    let v: Value = c
        .get(format!("https://api.mcsrvstat.us/3/{}", addr))
        .send()
        .await
        .map_err(|_| "Нет связи".to_string())?
        .json()
        .await
        .map_err(e)?;
    let motd = v["motd"]["clean"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    Ok(json!({
        "online": v["online"].as_bool().unwrap_or(false),
        "players": v["players"]["online"],
        "max": v["players"]["max"],
        "version": v["version"],
        "motd": motd,
        "icon": v["icon"]
    }))
}
