use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use eframe::emath::Align2;
use eframe::{egui, Frame};
use egui::{Color32, Context, TextEdit, Ui, Vec2};
use encoding_rs::Encoding;

use crate::config::{self, Preferences, Profile};
use crate::hex_utils;
use crate::highlight;
use crate::network::{self, ProxyKind, SendResult, TestResult};

const CUSTOM_PROFILE: &str = "< 自定义配置 >";
const MAX_TEXT_LEN: usize = 64 * 1024; // max stored input/preview text

// ---------------------------------------------------------------------------

pub struct SocketClientApp {
    // Preferences & profiles
    prefs: Preferences,
    profiles_map: BTreeMap<String, Profile>,
    profile_names: Vec<String>,

    // Server
    ip: String,
    port: String,
    selected_profile: String,

    // Proxy
    proxy_type: ProxyKind,
    proxy_host: String,
    proxy_port: String,
    proxy_status: String,
    proxy_status_color: Color32,

    // Data format
    timeout: String,
    encoding_mode: String, // "UTF-8", "GBK", "Hexadecimal (16进制)"
    prepend_length: bool,
    // hex_interpret_charset removed — derived from encoding_mode

    // Input tabs
    input_tab: usize, // 0=direct, 1=raw dump, 2=convert
    input_text: String,
    raw_dump_text: String,

    // Preview / output
    length_label: String,
    data_preview: String,
    hex_decode_preview: String,
    output_text: String,

    // Encoding converter
    hex_convert_text: String,
    string_convert_text: String,
    convert_charset: String,

    // Async results
    send_rx: Option<Receiver<SendResult>>,
    test_rx: Option<Receiver<TestResult>>,
    sending: bool,
    testing: bool,

    // Dialog state
    show_save_dialog: bool,
    show_rename_dialog: bool,
    show_delete_confirm: bool,
    dialog_name: String,
    rename_new_name: String,
    delete_target: String,

    // Save flag
    dirty: bool,
    // Packet length config
    length_field_size: u32,
    length_offset: u32,
    length_type: String,
    length_correction: i32,
    // Theme
    dark_mode: bool,
}

impl Default for SocketClientApp {
    fn default() -> Self {
        let prefs = config::load_preferences();
        let profiles_map = config::load_profiles();
        let profile_names: Vec<_> = profiles_map.keys().cloned().collect();

        let mut app = Self {
            prefs,
            profiles_map,
            profile_names,
            ip: String::new(),
            port: String::new(),
            selected_profile: CUSTOM_PROFILE.to_string(),
            proxy_type: ProxyKind::None,
            proxy_host: String::new(),
            proxy_port: String::new(),
            proxy_status: "状态: 未测试".to_string(),
            proxy_status_color: Color32::GRAY,
            timeout: "10000".to_string(),
            encoding_mode: "UTF-8".to_string(),
            prepend_length: true,
            // hex_interpret_charset removed
            input_tab: 0,
            input_text: String::new(),
            raw_dump_text: String::new(),
            length_label: "输入数据字节长度: 0".to_string(),
            data_preview: String::new(),
            hex_decode_preview: String::new(),
            output_text: String::new(),
            hex_convert_text: String::new(),
            string_convert_text: String::new(),
            convert_charset: "UTF-8".to_string(),
            send_rx: None,
            test_rx: None,
            sending: false,
            testing: false,
            show_save_dialog: false,
            show_rename_dialog: false,
            show_delete_confirm: false,
            dialog_name: String::new(),
            rename_new_name: String::new(),
            delete_target: String::new(),
            dirty: false,
            length_field_size: 5,
            length_offset: 0,
            length_type: "十进制".to_string(),
            length_correction: 0,
            dark_mode: true,
        };
        app.apply_prefs();
        app
    }
}

impl SocketClientApp {
    /// Apply loaded preferences to the UI state.
    fn apply_prefs(&mut self) {
        self.selected_profile = self.prefs.last_profile.clone();
        if self.selected_profile == CUSTOM_PROFILE {
            self.ip = self.prefs.custom_ip.clone();
            self.port = self.prefs.custom_port.clone();
        } else if let Some(p) = self.profiles_map.get(&self.selected_profile) {
            self.ip = p.ip.clone();
            self.port = p.port.clone();
        }
        self.timeout = self.prefs.timeout.clone();
        self.encoding_mode = self.prefs.encoding_mode.clone();
        self.prepend_length = self.prefs.prepend_length;
        self.proxy_type = match self.prefs.proxy_type.as_str() {
            "SOCKS5" => ProxyKind::Socks5,
            "HTTP" => ProxyKind::Http,
            _ => ProxyKind::None,
        };
        self.proxy_host = self.prefs.proxy_host.clone();
        self.proxy_port = self.prefs.proxy_port.clone();
        self.input_text = self.prefs.input_text.clone();
        self.raw_dump_text = self.prefs.raw_dump_text.clone();
        self.length_field_size = self.prefs.length_field_size;
        self.length_offset = self.prefs.length_offset;
        self.length_type = self.prefs.length_type.clone();
        self.length_correction = self.prefs.length_correction;
        self.dark_mode = self.prefs.dark_mode;
    }

    /// Collect preferences from current UI state.
    fn collect_prefs(&self) -> Preferences {
        Preferences {
            last_profile: self.selected_profile.clone(),
            custom_ip: if self.selected_profile == CUSTOM_PROFILE {
                self.ip.clone()
            } else {
                self.prefs.custom_ip.clone()
            },
            custom_port: if self.selected_profile == CUSTOM_PROFILE {
                self.port.clone()
            } else {
                self.prefs.custom_port.clone()
            },
            timeout: self.timeout.clone(),
            encoding_mode: self.encoding_mode.clone(),
            prepend_length: self.prepend_length,
            proxy_type: match self.proxy_type {
                ProxyKind::None => "无代理".to_string(),
                ProxyKind::Socks5 => "SOCKS5".to_string(),
                ProxyKind::Http => "HTTP".to_string(),
            },
            proxy_host: self.proxy_host.clone(),
            proxy_port: self.proxy_port.clone(),
            input_text: truncate(&self.input_text, MAX_TEXT_LEN),
            raw_dump_text: truncate(&self.raw_dump_text, MAX_TEXT_LEN),
            window_width: 1200.0,
            window_height: 800.0,
            length_field_size: self.length_field_size,
            length_offset: self.length_offset,
            length_type: self.length_type.clone(),
            length_correction: self.length_correction,
            dark_mode: self.dark_mode,
        }
    }

    /// Save state to disk.
    fn save_state(&mut self) {
        config::save_preferences(&self.collect_prefs());
        config::save_profiles(&self.profiles_map);
        self.dirty = false;
    }

    // ---------- helpers ----------

    fn is_hex_mode(&self) -> bool {
        self.encoding_mode == "Hexadecimal (16进制)"
    }

    /// Recalculate the preview and length label.
    fn update_preview(&mut self) {
        self.length_label = "输入数据字节长度: 0".to_string();
        self.data_preview.clear();
        self.hex_decode_preview.clear();

        if self.input_text.is_empty() {
            return;
        }

        let len_result = if self.is_hex_mode() {
            let cleaned: String = self
                .input_text
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect();
            if cleaned.len() % 2 != 0 || cleaned.is_empty() {
                self.length_label = if cleaned.is_empty() {
                    "16进制输入为空".to_string()
                } else {
                    "16进制数据长度必须为偶数".to_string()
                };
                self.data_preview = "错误: 无效的16进制输入".to_string();
                self.hex_decode_preview = "错误: 无效的16进制输入".to_string();
                return;
            }
            let bytes = hex_utils::hex_to_bytes(&cleaned);
            if bytes.is_none() {
                self.length_label = "错误: 无效的16进制字符".to_string();
                self.data_preview = "错误: 无效的16进制字符".to_string();
                self.hex_decode_preview = "错误: 无效的16进制字符".to_string();
                return;
            }
            let bytes = bytes.unwrap();
            self.length_label =
                format!("数据字节长度 (16进制解码后): {}", bytes.len());
            // Hex decode preview
            let charset = response_charset(&self.encoding_mode);
            let (decoded, _) = charset.decode_without_bom_handling(&bytes);
            self.hex_decode_preview = decoded.to_string();
            build_payload(
                &self.input_text,
                &self.encoding_mode,
                self.prepend_length,
                self.length_field_size,
                self.length_offset,
                &self.length_type,
                self.length_correction,
            )
        } else {
            let bytes = encode_bytes(&self.input_text, &self.encoding_mode);
            self.length_label = format!(
                "数据字节长度 (编码: {}): {}",
                self.encoding_mode,
                bytes.len()
            );
            build_payload(
                &self.input_text,
                &self.encoding_mode,
                self.prepend_length,
                self.length_field_size,
                self.length_offset,
                &self.length_type,
                self.length_correction,
            )
        };

        match len_result {
            Ok(payload) => {
                if self.prepend_length {
                    self.data_preview = hex_utils::bytes_to_hex(&payload);
                } else if self.is_hex_mode() {
                    self.data_preview = self.input_text.clone();
                } else {
                    self.data_preview = self.input_text.clone();
                }
            }
            Err(e) => {
                self.data_preview = format!("错误: {}", e);
            }
        }
    }

    /// Start a connectivity test in a background thread.
    fn start_test(&mut self, ctx: Context) {
        if self.testing {
            return;
        }
        let ip = self.ip.clone();
        let port = self.port.clone();
        let timeout = self.timeout.clone();
        let proxy_kind = self.proxy_type.clone();
        let proxy_host = self.proxy_host.clone();
        let proxy_port = self.proxy_port.clone();

        self.testing = true;
        self.proxy_status = "状态: 测试中...".to_string();
        self.proxy_status_color = Color32::BLUE;

        let (tx, rx) = mpsc::channel();
        self.test_rx = Some(rx);

        thread::spawn(move || {
            let port_u16 = port.parse().unwrap_or(0);
            let timeout_u64 = timeout.parse().unwrap_or(10000);
            let result = network::test_connectivity(
                &ip,
                port_u16,
                timeout_u64,
                &proxy_kind,
                &proxy_host,
                proxy_port.parse().unwrap_or(0),
            );
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    /// Start a send operation in a background thread.
    fn start_send(&mut self, ctx: Context) {
        if self.sending {
            return;
        }
        let ip = self.ip.clone();
        let port = self.port.clone();
        let timeout = self.timeout.clone();
        let proxy_kind = self.proxy_type.clone();
        let proxy_host = self.proxy_host.clone();
        let proxy_port = self.proxy_port.clone();
        let encoding_mode = self.encoding_mode.clone();
        let prepend_length = self.prepend_length;
        let length_field_size = self.length_field_size;
        let length_offset = self.length_offset;
        let length_type = self.length_type.clone();
        let length_correction = self.length_correction;
        let input_text = self.input_text.clone();

        self.sending = true;
        self.output_text = format!(
            "正在发送和接收 (超时: {}ms)...\n",
            timeout
        );

        let (tx, rx) = mpsc::channel();
        self.send_rx = Some(rx);

        thread::spawn(move || {
            let start = std::time::Instant::now();

            // Build payload
            let payload = match build_payload(
                &input_text,
                &encoding_mode,
                prepend_length,
                length_field_size,
                length_offset,
                &length_type,
                length_correction,
            ) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(SendResult {
                        success: false,
                        response: String::new(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: e,
                    });
                    return;
                }
            };

            let port_u16 = match port.parse() {
                Ok(p) if p > 0 => p,
                _ => {
                    let _ = tx.send(SendResult {
                        success: false,
                        response: String::new(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: "无效的端口号".to_string(),
                    });
                    return;
                }
            };
            let timeout_u64 = match timeout.parse() {
                Ok(t) if t > 0 => t,
                _ => 10000,
            };

            let response_charset = response_charset(&encoding_mode);

            match network::send_and_receive(
                &ip,
                port_u16,
                timeout_u64,
                &proxy_kind,
                &proxy_host,
                proxy_port.parse().unwrap_or(0),
                &payload,
                1024 * 1024,
            ) {
                Ok((raw, elapsed)) => {
                    let (decoded, _) =
                        response_charset.decode_without_bom_handling(&raw);
                    let _ = tx.send(SendResult {
                        success: true,
                        response: decoded.to_string(),
                        duration_ms: elapsed,
                        error: String::new(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(SendResult {
                        success: false,
                        response: String::new(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: e,
                    });
                }
            }
            ctx.request_repaint();
        });
    }

    /// Check for async results and update UI state.
    fn check_async_results(&mut self) {
        // Check test result
        if let Some(rx) = &self.test_rx {
            if let Ok(result) = rx.try_recv() {
                self.testing = false;
                self.test_rx = None;
                if result.success {
                    self.proxy_status =
                        format!("状态: 成功 - {}", result.message);
                    self.proxy_status_color = Color32::from_rgb(0, 180, 0);
                } else {
                    self.proxy_status =
                        format!("状态: {}", result.message);
                    self.proxy_status_color = Color32::RED;
                }
            }
        }

        // Check send result
        if let Some(rx) = &self.send_rx {
            if let Ok(result) = rx.try_recv() {
                self.sending = false;
                self.send_rx = None;
                let status = if result.success { "成功" } else { "失败" };
                let line = format!(
                    "操作{} (耗时: {} ms)\n",
                    status, result.duration_ms
                );
                self.output_text.push_str(&line);
                if result.success {
                    self.output_text.push_str(&format!(
                        "服务器响应:\n{}\n",
                        result.response
                    ));
                } else {
                    self.output_text.push_str(&format!(
                        "{}\n",
                        result.error
                    ));
                }
            }
        }
    }

    // ---------- encoding converter ----------

    fn convert_hex_to_string(&mut self) {
        let cleaned: String = self
            .hex_convert_text
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        if cleaned.is_empty() {
            self.string_convert_text.clear();
            return;
        }
        if cleaned.len() % 2 != 0 {
            self.string_convert_text = "错误: HEX长度必须为偶数".to_string();
            return;
        }
        match hex_utils::hex_to_bytes(&cleaned) {
            Some(bytes) => {
                let encoding =
                    Encoding::for_label(self.convert_charset.as_bytes())
                        .unwrap_or(encoding_rs::UTF_8);
                let (decoded, _) = encoding.decode_without_bom_handling(&bytes);
                self.string_convert_text = decoded.to_string();
            }
            None => {
                self.string_convert_text = "错误: 无效的HEX字符".to_string();
            }
        }
    }

    fn convert_string_to_hex(&mut self) {
        if self.string_convert_text.is_empty() {
            self.hex_convert_text.clear();
            return;
        }
        let encoding = Encoding::for_label(self.convert_charset.as_bytes())
            .unwrap_or(encoding_rs::UTF_8);
        let (bytes, _, _) = encoding.encode(&self.string_convert_text);
        self.hex_convert_text = hex_utils::bytes_to_hex(&bytes);
    }

    // ---------- hex dump extraction ----------

    fn extract_hex_from_dump(&mut self) {
        if self.raw_dump_text.trim().is_empty() {
            return;
        }
        let extracted = hex_utils::parse_hex_dump(&self.raw_dump_text);
        if extracted.is_empty() {
            return;
        }
        if let Some(bytes) = hex_utils::hex_to_bytes(&extracted) {
            self.input_text = String::from_utf8_lossy(&bytes).into_owned();
        } else {
            self.input_text = extracted;
        }
        self.input_tab = 0;
        self.encoding_mode = "String (字符串)".to_string();
        self.update_preview();
    }

    // ---------- save hex to file ----------

    fn save_hex_to_file(&mut self) {
        let cleaned: String = self
            .input_text
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        if cleaned.len() % 2 != 0 || cleaned.is_empty() {
            return;
        }
        let bytes = match hex_utils::hex_to_bytes(&cleaned) {
            Some(b) => b,
            None => return,
        };

        // Use rfd file dialog on main thread
        if let Some(path) = rfd::FileDialog::new()
            .set_title("保存16进制数据到文件")
            .set_file_name("hex_data.bin")
            .add_filter("二进制文件 (*.bin, *.dat)", &["bin", "dat"])
            .save_file()
        {
            if std::fs::write(&path, &bytes).is_ok() {
                self.output_text.push_str(&format!(
                    "文件已保存: {}\n",
                    path.display()
                ));
            } else {
                self.output_text
                    .push_str(&format!("保存文件出错: {}\n", path.display()));
            }
        }
    }

    // ---------- profile management ----------

    fn profile_selected(&mut self, name: String) {
        self.selected_profile = name;
        if self.selected_profile == CUSTOM_PROFILE {
            self.ip = self.prefs.custom_ip.clone();
            self.port = self.prefs.custom_port.clone();
        } else if let Some(p) = self.profiles_map.get(&self.selected_profile) {
            self.ip = p.ip.clone();
            self.port = p.port.clone();
        }
        self.dirty = true;
    }

    fn on_ip_port_edited(&mut self) {
        if self.selected_profile != CUSTOM_PROFILE {
            self.selected_profile = CUSTOM_PROFILE.to_string();
            self.dirty = true;
        }
    }

    fn save_profile(&mut self) {
        let name = self.dialog_name.trim().to_string();
        if name.is_empty() || self.ip.trim().is_empty() || self.port.trim().is_empty() {
            return;
        }
        self.profiles_map.insert(
            name.clone(),
            Profile {
                ip: self.ip.trim().to_string(),
                port: self.port.trim().to_string(),
            },
        );
        self.profile_names = self.profiles_map.keys().cloned().collect();
        self.selected_profile = name;
        self.show_save_dialog = false;
        self.dirty = true;
        self.save_state();
    }

    fn rename_profile(&mut self) {
        let new_name = self.rename_new_name.trim().to_string();
        if new_name.is_empty() || self.profiles_map.contains_key(&new_name) {
            return;
        }
        let old_name = self.selected_profile.clone();
        if let Some(profile) = self.profiles_map.remove(&old_name) {
            self.profiles_map.insert(new_name.clone(), profile);
        }
        self.profile_names = self.profiles_map.keys().cloned().collect();
        self.selected_profile = new_name;
        self.show_rename_dialog = false;
        self.dirty = true;
        self.save_state();
    }

    fn delete_profile(&mut self) {
        let name = self.delete_target.clone();
        self.profiles_map.remove(&name);
        self.profile_names = self.profiles_map.keys().cloned().collect();
        self.selected_profile = CUSTOM_PROFILE.to_string();
        self.show_delete_confirm = false;
        self.dirty = true;
        self.save_state();
    }

    fn show_save_dialog(&mut self) {
        self.dialog_name = if self.selected_profile != CUSTOM_PROFILE {
            self.selected_profile.clone()
        } else {
            String::new()
        };
        self.show_save_dialog = true;
    }

    fn show_rename_dialog(&mut self) {
        if self.selected_profile == CUSTOM_PROFILE {
            return;
        }
        self.rename_new_name = self.selected_profile.clone();
        self.show_rename_dialog = true;
    }

    fn show_delete_confirm(&mut self) {
        if self.selected_profile == CUSTOM_PROFILE {
            return;
        }
        self.delete_target = self.selected_profile.clone();
        self.show_delete_confirm = true;
    }

}

// ---------- standalone helper functions ----------

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max / 4).collect()
    }
}

/// Encode a string to bytes using the given charset name.
/// Derive response charset from encoding mode.
fn response_charset(encoding_mode: &str) -> &'static encoding_rs::Encoding {
    if encoding_mode == "GBK" {
        encoding_rs::GBK
    } else {
        encoding_rs::UTF_8
    }
}

fn encode_bytes(text: &str, charset: &str) -> Vec<u8> {
    let encoding = Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (bytes, _, _) = encoding.encode(text);
    bytes.into_owned()
}

/// Build the payload bytes to send over the socket.
fn build_payload(
    input: &str,
    encoding_mode: &str,
    prepend_length: bool,
    length_field_size: u32,
    length_offset: u32,
    length_type: &str,
    length_correction: i32,
) -> Result<Vec<u8>, String> {
    let data: Vec<u8> = if encoding_mode == "Hexadecimal (16进制)" {
        let cleaned: String = input
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        if cleaned.len() % 2 != 0 {
            return Err("错误: 16进制数据长度必须为偶数。".to_string());
        }
        hex_utils::hex_to_bytes(&cleaned)
            .ok_or_else(|| "错误: 无效的16进制输入".to_string())?
    } else {
        let encoding =
            Encoding::for_label(encoding_mode.as_bytes()).unwrap_or(encoding_rs::UTF_8);
        let (bytes, _, _) = encoding.encode(input);
        bytes.into_owned()
    };

    if !prepend_length {
        return Ok(data);
    }

    let field_value = (data.len() as i32 - length_correction).max(0) as u32;
    let len_bytes = encode_length(field_value, length_field_size, length_type)?;

    let offset = length_offset as usize;
    let mut payload = Vec::with_capacity(offset + len_bytes.len() + data.len());
    payload.resize(offset, 0u8);
    payload.extend_from_slice(&len_bytes);
    payload.extend_from_slice(&data);
    Ok(payload)
}

fn encode_length(value: u32, size: u32, ty: &str) -> Result<Vec<u8>, String> {
    match ty {
        "十进制" => {
            let s = format!("{:0width$}", value, width = size as usize);
            if s.len() > size as usize {
                return Err(format!("长度值 {} 超出 {} 位十进制表示", value, size));
            }
            Ok(s.into_bytes())
        }
        "HEX" => match size {
            1 => {
                if value > 0xFF {
                    return Err(format!("长度值 {} 超出1字节范围", value));
                }
                Ok(vec![value as u8])
            }
            2 => {
                if value > 0xFFFF {
                    return Err(format!("长度值 {} 超出2字节范围", value));
                }
                Ok(vec![(value >> 8) as u8, value as u8])
            }
            4 => Ok(vec![
                (value >> 24) as u8,
                (value >> 16) as u8,
                (value >> 8) as u8,
                value as u8,
            ]),
            _ => Ok((value as u32).to_be_bytes().to_vec()),
        },
        _ => Err(format!("不支持的长度类型: {}", ty)),
    }
}

// ---------- eframe::App implementation ----------

impl eframe::App for SocketClientApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // Apply theme
        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        self.check_async_results();

        // ---- Layout ----

        // Bottom panel: Send button + theme toggle
        egui::TopBottomPanel::bottom("bottom_panel")
            .min_height(30.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    // Theme toggle on the left
                    let theme_label = if self.dark_mode { "☀ 亮色" } else { "🌙 暗色" };
                    if ui.button(theme_label).clicked() {
                        self.dark_mode = !self.dark_mode;
                        self.dirty = true;
                    }
                    // Send button on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let send_label = if self.sending {
                            "发送中..."
                        } else {
                            "发送 (Send)"
                        };
                        let mut btn = egui::Button::new(send_label)
                            .min_size(Vec2::new(200.0, 32.0));
                        if self.sending || self.ip.trim().is_empty() || self.port.trim().is_empty()
                        {
                            btn = btn.fill(Color32::DARK_GRAY);
                        }
                        let clicked = ui
                            .add_enabled(!self.sending, btn)
                            .clicked();
                        if clicked {
                            self.start_send(ctx.clone());
                        }
                    });
                });
                ui.add_space(4.0);
            });

        // Left panel: settings + input
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(450.0)
            .min_width(300.0)
            .show(ctx, |ui| {
                self.ui_left_panel(ui, ctx);
            });

        // Central panel: previews + output
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(ctx.style().visuals.window_fill()))
            .show(ctx, |ui| {
                self.ui_right_panel(ui);
            });

        // Modals / dialogs
        self.show_dialogs(ctx);

        // Auto-refresh while async ops in flight
        if self.sending || self.testing {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.prefs = self.collect_prefs();
        config::save_preferences(&self.prefs);
        config::save_profiles(&self.profiles_map);
    }
}

// ---------- UI sections ----------

impl SocketClientApp {
    fn ui_left_panel(&mut self, ui: &mut Ui, ctx: &Context) {
        // ---- Target Server ----
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("目标服务器").strong().size(14.0));
                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    ui.label("配置:");
                    let current = self.selected_profile.clone();
                    egui::ComboBox::from_id_source("profile_combo")
                        .selected_text(&current)
                        .show_ui(ui, |ui| {
                            let is_custom = ui
                                .selectable_label(current == CUSTOM_PROFILE, CUSTOM_PROFILE)
                                .clicked();
                            if is_custom {
                                self.profile_selected(CUSTOM_PROFILE.to_string());
                            }
                            for name in self.profile_names.clone() {
                                let is_selected = current == name;
                                if ui
                                    .selectable_label(is_selected, &name)
                                    .clicked()
                                {
                                    self.profile_selected(name);
                                }
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("IP:");
                    if ui.add(TextEdit::singleline(&mut self.ip).hint_text("127.0.0.1")).changed() {
                        self.on_ip_port_edited();
                        self.update_preview();
                    }
                    ui.label("端口:");
                    if ui.add(TextEdit::singleline(&mut self.port).hint_text("8888").desired_width(60.0)).changed() {
                        self.on_ip_port_edited();
                    }
                });

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("删除").clicked() { self.show_delete_confirm(); }
                        if ui.button("重命名").clicked() { self.show_rename_dialog(); }
                        if ui.button("保存").clicked() { self.show_save_dialog(); }
                    });
                });
            });

        ui.add_space(4.0);

        // ---- Proxy ----
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("代理设置").strong().size(14.0));
                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    ui.label("类型:");
                    let proxy_labels = ["无代理", "SOCKS5", "HTTP"];
                    let proxy_values = [ProxyKind::None, ProxyKind::Socks5, ProxyKind::Http];
                    let current_idx = proxy_values.iter().position(|v| *v == self.proxy_type).unwrap_or(0);
                    egui::ComboBox::from_id_source("proxy_type")
                        .selected_text(proxy_labels[current_idx])
                        .show_ui(ui, |ui| {
                            for (i, label) in proxy_labels.iter().enumerate() {
                                if ui.selectable_label(current_idx == i, *label).clicked() {
                                    self.proxy_type = proxy_values[i].clone();
                                    self.proxy_status = "状态: 未测试".to_string();
                                    self.proxy_status_color = Color32::GRAY;
                                }
                            }
                        });

                    let proxy_enabled = self.proxy_type != ProxyKind::None;
                    ui.add_enabled_ui(proxy_enabled, |ui| {
                        ui.label("主机:");
                        if ui.add(TextEdit::singleline(&mut self.proxy_host).hint_text("127.0.0.1").desired_width(100.0)).changed() { self.dirty = true; }
                        ui.label("端口:");
                        if ui.add(TextEdit::singleline(&mut self.proxy_port).hint_text("1080").desired_width(50.0)).changed() { self.dirty = true; }
                    });
                });

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(!self.testing, egui::Button::new(if self.testing { "测试中..." } else { "测试连通性" })).clicked() {
                            self.start_test(ctx.clone());
                        }
                        ui.colored_label(self.proxy_status_color, &self.proxy_status);
                    });
                });
            });

        ui.add_space(4.0);

        // ---- Data Format ----
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("数据格式").strong().size(14.0));
                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    ui.label("超时(ms):");
                    if ui.add(TextEdit::singleline(&mut self.timeout).hint_text("10000").desired_width(60.0)).changed() { self.dirty = true; }

                    ui.label("输入类型/编码:");
                    let encoding_options = ["UTF-8", "GBK", "Hexadecimal (16进制)"];
                    let current_enc = encoding_options.iter().position(|e| *e == self.encoding_mode).unwrap_or(0);
                    egui::ComboBox::from_id_source("encoding_mode")
                        .selected_text(encoding_options[current_enc])
                        .show_ui(ui, |ui| {
                            for (i, opt) in encoding_options.iter().enumerate() {
                                if ui.selectable_label(current_enc == i, *opt).clicked() {
                                    self.encoding_mode = opt.to_string();
                                    self.update_preview();
                                }
                            }
                        });

                    if ui.checkbox(&mut self.prepend_length, "附加长度头").changed() {
                        self.update_preview();
                    }
                });

                if self.prepend_length {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label("字节数:");
                        let sizes = [1u32, 2, 4, 5];
                        egui::ComboBox::from_id_source("len_size")
                            .selected_text(format!("{}", self.length_field_size))
                            .show_ui(ui, |ui| {
                                for s in &sizes {
                                    if ui.selectable_label(self.length_field_size == *s, format!("{}", s)).clicked() {
                                        self.length_field_size = *s;
                                        self.update_preview();
                                    }
                                }
                            });

                        ui.label("类型:");
                        egui::ComboBox::from_id_source("len_type")
                            .selected_text(&self.length_type)
                            .show_ui(ui, |ui| {
                                let types = ["十进制", "HEX"];
                                for t in &types {
                                    if ui.selectable_label(self.length_type == *t, *t).clicked() {
                                        self.length_type = t.to_string();
                                        self.update_preview();
                                    }
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("偏移量:");
                        if ui.add(egui::DragValue::new(&mut self.length_offset).clamp_range(0.0..=65535.0)).changed() {
                            self.update_preview();
                        }
                        ui.label("修正值:");
                        if ui.add(egui::DragValue::new(&mut self.length_correction).clamp_range(-99999.0..=99999.0)).changed() {
                            self.update_preview();
                        }
                    });
                }
            });

        ui.add_space(4.0);

        // ---- Hex interpretation (only shown in hex mode) ----
        if self.is_hex_mode() {
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "16进制解码预览编码: {}",
                            if self.encoding_mode == "GBK" { "GBK" } else { "UTF-8" }
                        ));
                    });
                });
            ui.add_space(2.0);
        }

        // ---- Input Tabs ----
        let tab_labels = ["直接输入", "从报文粘贴", "编码转换"];
        ui.horizontal(|ui| {
            let mut selected = self.input_tab;
            for (i, label) in tab_labels.iter().enumerate() {
                if ui.selectable_value(&mut selected, i, *label).changed() {
                    self.input_tab = i;
                }
            }
        });
        ui.separator();

        match self.input_tab {
            0 => self.ui_direct_input(ui),
            1 => self.ui_raw_dump(ui),
            2 => self.ui_encoding_convert(ui),
            _ => {}
        }
    }

    // ---- Direct Input tab ----
    fn ui_direct_input(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(&self.length_label);
            if !self.is_hex_mode() {
                if ui.button("格式化 JSON/XML").clicked() {
                    let lang = highlight::detect_language(&self.input_text);
                    let result = match lang {
                        highlight::Language::Json => highlight::format_json(&self.input_text),
                        highlight::Language::Xml => highlight::format_xml(&self.input_text),
                        highlight::Language::Plain => return,
                    };
                    if let Ok(f) = result {
                        self.input_text = f;
                        self.update_preview();
                    }
                }
            }
            if self.is_hex_mode() {
                if ui.button("保存16进制为文件").clicked() {
                    self.save_hex_to_file();
                }
            }
        });
        let h = (ui.available_height() - 4.0).max(60.0);
        let hint = if self.is_hex_mode() {
            "输入16进制数据 (例如: 48 65 6C 6C 6F)"
        } else {
            "输入要发送的文本数据"
        };
        egui::ScrollArea::vertical()
            .id_source("input_scroll")
            .max_height(h)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let mut layouter = highlight::make_layouter();
                let changed = ui.add(
                    TextEdit::multiline(&mut self.input_text)
                        .desired_rows(10)
                        .hint_text(hint)
                        .margin(egui::Margin::ZERO)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter),
                ).changed();
                if changed {
                    self.update_preview();
                    self.on_ip_port_edited();
                }
            });
    }

    // ---- Raw Dump tab ----
    fn ui_raw_dump(&mut self, ui: &mut Ui) {
        ui.label("在此处粘贴完整的十六进制转储报文:");
        let h = (ui.available_height() - 30.0).max(60.0);
        egui::ScrollArea::vertical()
            .id_source("raw_dump_scroll")
            .max_height(h)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.raw_dump_text)
                        .desired_rows(10)
                        .hint_text("00000000h: 48 65 6C 6C 6F ...")
                        .margin(egui::Margin::ZERO)
                        .desired_width(f32::INFINITY),
                );
            });
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("从报文提取HEX到直接输入区").clicked() {
                    self.extract_hex_from_dump();
                }
            });
        });
    }

    // ---- Encoding Convert tab ----
    fn ui_encoding_convert(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("编码:");
            let charset_opts = [
                "UTF-8", "GBK", "GB2312", "GB18030", "ISO-8859-1", "US-ASCII",
                "Shift_JIS", "EUC-JP", "EUC-KR", "UTF-16", "UTF-16BE", "UTF-16LE",
            ];
            let cur = charset_opts
                .iter()
                .position(|c| *c == self.convert_charset)
                .unwrap_or(0);
            egui::ComboBox::from_id_source("convert_charset")
                .selected_text(charset_opts[cur])
                .show_ui(ui, |ui| {
                    for (i, opt) in charset_opts.iter().enumerate() {
                        if ui
                            .selectable_label(cur == i, *opt)
                            .clicked()
                        {
                            self.convert_charset = opt.to_string();
                        }
                    }
                });
            if ui.button("清空").clicked() {
                self.hex_convert_text.clear();
                self.string_convert_text.clear();
            }
            if ui.button("交换内容").clicked() {
                std::mem::swap(
                    &mut self.hex_convert_text,
                    &mut self.string_convert_text,
                );
            }
        });

        ui.add_space(4.0);
        let half = ((ui.available_height() - 8.0) / 2.0).max(40.0);

        // HEX section
        ui.label("HEX (16进制):");
        egui::ScrollArea::vertical()
            .id_source("hex_convert_scroll")
            .max_height(half)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add(TextEdit::multiline(&mut self.hex_convert_text).desired_rows(4).margin(egui::Margin::ZERO).desired_width(f32::INFINITY));
            });
        if ui.button("HEX → 字符").clicked() {
            self.convert_hex_to_string();
        }

        ui.add_space(4.0);

        // String section
        ui.label("字符/文本:");
        egui::ScrollArea::vertical()
            .id_source("string_convert_scroll")
            .max_height(half)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add(TextEdit::multiline(&mut self.string_convert_text).desired_rows(4).margin(egui::Margin::ZERO).desired_width(f32::INFINITY));
            });
        if ui.button("字符 → HEX").clicked() {
            self.convert_string_to_hex();
        }
    }

    // ---- Right panel (previews + output) ----
    fn ui_right_panel(&mut self, ui: &mut Ui) {
        let total_h = ui.available_height();
        let gap = 2.0;
        let overhead = 22.0;

        let right_section = |ui: &mut Ui,
                             label: &str,
                             text: &mut String,
                             max_text_height: f32| {
            egui::Frame {
                    inner_margin: egui::Margin::ZERO,
                    outer_margin: egui::Margin::ZERO,
                    rounding: egui::Rounding::ZERO,
                    shadow: egui::epaint::Shadow::NONE,
                    fill: ui.style().visuals.window_fill(),
                    stroke: ui.style().visuals.window_stroke(),
                }
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(label).strong().size(13.0));
                    let mut layouter = highlight::make_layouter();
                    egui::ScrollArea::vertical()
                        .id_source(label.to_string())
                        .max_height(max_text_height)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(text)
                                    .desired_rows(3)
                                    .font(egui::TextStyle::Monospace)
                                    .margin(egui::Margin::ZERO)
                                    .desired_width(f32::INFINITY)
                                    .layouter(&mut layouter),
                            );
                        });
                });
        };

        if self.is_hex_mode() {
            let sec_h = (total_h - gap * 2.0) / 3.0;
            let text_h = (sec_h - overhead).max(30.0);
            let preview_title = if self.prepend_length {
                "待发送数据预览 (长度头+数据)"
            } else {
                "待发送数据预览 (原始数据)"
            };
            right_section(ui, preview_title, &mut self.data_preview, text_h);
            ui.add_space(gap);
            right_section(
                ui,
                "16进制解码预览 (使用上述编码)",
                &mut self.hex_decode_preview,
                text_h,
            );
            ui.add_space(gap);
            right_section(ui, "输出区 (服务器响应)", &mut self.output_text, text_h);
        } else {
            let sec_h = (total_h - gap) / 2.0;
            let text_h = (sec_h - overhead).max(30.0);
            let preview_title = if self.prepend_length {
                "待发送数据预览 (长度头+数据)"
            } else {
                "待发送数据预览 (原始数据)"
            };
            right_section(ui, preview_title, &mut self.data_preview, text_h);
            ui.add_space(gap);
            right_section(ui, "输出区 (服务器响应)", &mut self.output_text, text_h);
        }
    }

    // ---- Dialogs ----
    fn show_dialogs(&mut self, ctx: &Context) {
        // Save dialog
        if self.show_save_dialog {
            let mut open = true;
            egui::Window::new("保存服务器配置")
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("请输入配置名称:");
                    ui.text_edit_singleline(&mut self.dialog_name);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("保存").clicked() {
                            self.save_profile();
                        }
                        if ui.button("取消").clicked() {
                            self.show_save_dialog = false;
                        }
                    });
                });
            if !open {
                self.show_save_dialog = false;
            }
        }

        // Rename dialog
        if self.show_rename_dialog {
            let mut open = true;
            egui::Window::new("重命名配置")
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("请输入新的配置名称:");
                    ui.text_edit_singleline(&mut self.rename_new_name);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("重命名").clicked() {
                            self.rename_profile();
                        }
                        if ui.button("取消").clicked() {
                            self.show_rename_dialog = false;
                        }
                    });
                });
            if !open {
                self.show_rename_dialog = false;
            }
        }

        // Delete confirm dialog
        if self.show_delete_confirm {
            let mut open = true;
            egui::Window::new("确认删除")
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "确定要删除配置 \"{}\" 吗?",
                        self.delete_target
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("删除").clicked() {
                            self.delete_profile();
                        }
                        if ui.button("取消").clicked() {
                            self.show_delete_confirm = false;
                        }
                    });
                });
            if !open {
                self.show_delete_confirm = false;
            }
        }
    }
}
