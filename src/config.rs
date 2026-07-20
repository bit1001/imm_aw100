use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// Server profile: just IP and port.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub ip: String,
    pub port: String,
}

/// All application preferences.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preferences {
    // Last selected profile name
    pub last_profile: String,
    // Custom (last-used) IP and port
    pub custom_ip: String,
    pub custom_port: String,
    // Data format
    pub timeout: String,
    pub encoding_mode: String,
    pub prepend_length: bool,
    // Proxy
    pub proxy_type: String,
    pub proxy_host: String,
    pub proxy_port: String,
    // Input text (may be truncated)
    pub input_text: String,
    pub raw_dump_text: String,
    // Window
    pub window_width: f32,
    pub window_height: f32,
    // Packet length config
    #[serde(default)]
    pub length_field_size: u32,
    #[serde(default)]
    pub length_offset: u32,
    #[serde(default)]
    pub length_type: String,
    #[serde(default)]
    pub length_correction: i32,
    // Theme
    #[serde(default)]
    pub dark_mode: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            last_profile: "< 自定义配置 >".to_string(),
            custom_ip: "127.0.0.1".to_string(),
            custom_port: "8888".to_string(),
            timeout: "10000".to_string(),
            encoding_mode: "UTF-8".to_string(),
            prepend_length: true,
            proxy_type: "无代理".to_string(),
            proxy_host: String::new(),
            proxy_port: String::new(),
            input_text: String::new(),
            raw_dump_text: String::new(),
            window_width: 1200.0,
            window_height: 800.0,
            length_field_size: 5,
            length_offset: 0,
            length_type: "十进制".to_string(),
            length_correction: 0,
            dark_mode: true,
        }
    }
}

/// Gets the config directory path.
fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("SocketClientGUI_R")
}

/// Gets the preferences file path.
fn prefs_path() -> PathBuf {
    config_dir().join("preferences.json")
}

/// Gets the profiles file path.
fn profiles_path() -> PathBuf {
    config_dir().join("profiles.json")
}

/// Load preferences from disk.
pub fn load_preferences() -> Preferences {
    let path = prefs_path();
    if !path.exists() {
        return Preferences::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("加载配置失败 (JSON错误: {}), 使用默认设置", e);
                Preferences::default()
            }
        },
        Err(e) => {
            eprintln!("加载配置失败: {}, 使用默认设置", e);
            Preferences::default()
        }
    }
}

/// Save preferences to disk.
pub fn save_preferences(prefs: &Preferences) {
    let dir = config_dir();
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    let path = prefs_path();
    match serde_json::to_string_pretty(prefs) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, &json) {
                eprintln!("保存配置失败: {}", e);
            }
        }
        Err(e) => eprintln!("序列化配置失败: {}", e),
    }
}

/// Load all profiles from disk.
pub fn load_profiles() -> BTreeMap<String, Profile> {
    let path = profiles_path();
    if !path.exists() {
        return BTreeMap::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("加载配置集失败: {}", e);
                BTreeMap::new()
            }
        },
        Err(e) => {
            eprintln!("加载配置集失败: {}", e);
            BTreeMap::new()
        }
    }
}

/// Save all profiles to disk.
pub fn save_profiles(profiles: &BTreeMap<String, Profile>) {
    let dir = config_dir();
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    let path = profiles_path();
    match serde_json::to_string_pretty(profiles) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, &json) {
                eprintln!("保存配置集失败: {}", e);
            }
        }
        Err(e) => eprintln!("序列化配置集失败: {}", e),
    }
}
