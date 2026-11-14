use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct EnvCheckReport {
    pub ip: String,
    pub country_code: String,
    pub country_name: String,
    pub ip_timezone: String,
    pub local_timezone: String,
    pub vpn_ok: bool,
    pub vpn_message: String,
    pub tz_ok: bool,
    pub tz_hint: String,
}

#[derive(Deserialize)]
struct IpWhoIsResp {
    success: Option<bool>,
    ip: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    timezone: Option<IpWhoIsTz>,
}

#[derive(Deserialize)]
struct IpWhoIsTz {
    id: Option<String>,
}

#[derive(Deserialize)]
struct IpApiCoResp {
    ip: Option<String>,
    country_name: Option<String>,
    country_code: Option<String>,
    timezone: Option<String>,
}

#[derive(Deserialize)]
struct IpifyResp {
    ip: Option<String>,
}

const BLOCKED_COUNTRIES: &[&str] = &["RU", "BY"];

pub fn check_environment() -> EnvCheckReport {
    let local_timezone = detect_local_timezone();
    let (ip, country_code, country_name, ip_timezone) = fetch_external_geo();

    let vpn_ok = !ip.is_empty()
        && !country_code.is_empty()
        && !BLOCKED_COUNTRIES.contains(&country_code.as_str());

    let vpn_message = if ip.is_empty() {
        "Не удалось определить внешний IP. Проверьте интернет или VPN-подключение в Happ.".to_string()
    } else if BLOCKED_COUNTRIES.contains(&country_code.as_str()) {
        format!(
            "Внешний IP {} определяется как {} ({}). Включите зарубежную локацию в Happ перед входом в Claude.",
            ip, country_name, country_code
        )
    } else if country_code.is_empty() {
        format!("Внешний IP: {} (страну определить не удалось, проверьте локацию в Happ).", ip)
    } else {
        format!("VPN активен: {} ({}, {}).", ip, country_name, country_code)
    };

    let (tz_ok, tz_hint) = evaluate_timezone(&local_timezone, &ip_timezone, vpn_ok);

    EnvCheckReport {
        ip,
        country_code,
        country_name,
        ip_timezone,
        local_timezone,
        vpn_ok,
        vpn_message,
        tz_ok,
        tz_hint,
    }
}

fn fetch_external_geo() -> (String, String, String, String) {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(4))
        .build();

    if let Ok(resp) = agent.get("https://ipwho.is/").call() {
        if let Ok(data) = resp.into_json::<IpWhoIsResp>() {
            if data.success.unwrap_or(false) {
                return (
                    data.ip.unwrap_or_default(),
                    data.country_code.unwrap_or_default(),
                    data.country.unwrap_or_default(),
                    data.timezone.and_then(|t| t.id).unwrap_or_default(),
                );
            }
        }
    }

    if let Ok(resp) = agent.get("https://ipapi.co/json/").call() {
        if let Ok(data) = resp.into_json::<IpApiCoResp>() {
            if let Some(ip) = data.ip {
                return (
                    ip,
                    data.country_code.unwrap_or_default(),
                    data.country_name.unwrap_or_default(),
                    data.timezone.unwrap_or_default(),
                );
            }
        }
    }

    if let Ok(resp) = agent.get("https://api.ipify.org?format=json").call() {
        if let Ok(data) = resp.into_json::<IpifyResp>() {
            if let Some(ip) = data.ip {
                return (ip, String::new(), String::new(), String::new());
            }
        }
    }

    (String::new(), String::new(), String::new(), String::new())
}

fn detect_local_timezone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        let trimmed = tz.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(target) = std::fs::read_link("/etc/localtime") {
            let s = target.to_string_lossy().to_string();
            if let Some(idx) = s.find("zoneinfo/") {
                return s[idx + "zoneinfo/".len()..].to_string();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = std::process::Command::new("tzutil").args(["/g"]).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }

    "Не определена".to_string()
}

fn evaluate_timezone(local_tz: &str, ip_tz: &str, vpn_ok: bool) -> (bool, String) {
    let ru_markers = ["Moscow", "Samara", "Yekaterinburg", "Novosibirsk", "Vladivostok", "Minsk", "Kaliningrad", "Volgograd", "Russian", "Belarus"];
    let looks_russian = ru_markers.iter().any(|m| local_tz.contains(m));

    if !vpn_ok {
        return (
            false,
            format!(
                "Часовой пояс системы: {}. Сначала включите VPN, затем при необходимости выровняйте время под страну VPN.",
                local_tz
            ),
        );
    }

    if looks_russian {
        let target = if ip_tz.is_empty() {
            "часовой пояс страны вашего VPN".to_string()
        } else {
            ip_tz.to_string()
        };
        return (
            false,
            format!(
                "Часовой пояс системы сейчас: {}. Для регистрации и первого входа в новый аккаунт лучше временно выбрать {} в настройках даты и времени ОС (глобальный реестр мы не меняем, чтобы не ломать другие программы).",
                local_tz, target
            ),
        );
    }

    (
        true,
        format!("Часовой пояс системы ({}) не конфликтует с зарубежным VPN.", local_tz),
    )
}
