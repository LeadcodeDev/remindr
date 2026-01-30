use chrono::Utc;
use uuid::{NoContext, Timestamp, Uuid};

pub mod app;
pub mod domain;
pub mod infrastructure;

pub struct Utils;

impl Utils {
    pub fn generate_uuid() -> Uuid {
        let now = Utc::now();
        let seconds: u64 = now.timestamp().try_into().unwrap_or(0);
        let timestamp = Timestamp::from_unix(NoContext, seconds, 0);

        Uuid::new_v7(timestamp)
    }

    /// Returns a date format string based on the system locale.
    /// e.g. "fr" -> "%d/%m/%Y", "en-US" -> "%m/%d/%Y", default -> "%Y/%m/%d"
    pub fn locale_date_format() -> &'static str {
        use sys_locale::get_locale;

        static FORMAT: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
        FORMAT.get_or_init(|| {
            let locale = get_locale().unwrap_or_default().to_lowercase();
            let lang = locale.split('-').next().unwrap_or("");

            match lang {
                "fr" | "it" | "pt" | "ro" | "vi" | "cs" | "sk" | "sl" | "hr" | "sr" | "bg"
                | "uk" | "ru" | "pl" | "el" | "tr" | "ar" | "he" | "th" | "id" | "ms" | "nl"
                | "de" | "es" | "da" | "no" | "nb" | "nn" | "sv" | "fi" => "%d/%m/%Y",
                "en" => {
                    if locale.contains("gb")
                        || locale.contains("au")
                        || locale.contains("nz")
                        || locale.contains("ie")
                        || locale.contains("in")
                    {
                        "%d/%m/%Y"
                    } else {
                        "%m/%d/%Y"
                    }
                }
                "zh" | "ja" | "ko" | "hu" | "lt" | "fa" => "%Y/%m/%d",
                _ => "%Y/%m/%d",
            }
        })
    }
}

#[derive(Clone)]
pub enum LoadingState<T> {
    Loading,
    Loaded(T),
    Error(String),
}
