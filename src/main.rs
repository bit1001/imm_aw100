#![windows_subsystem = "windows"]

mod app;
mod config;
mod hex_utils;
mod highlight;
mod network;

use app::SocketClientApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Socket Client GUI",
        native_options,
        Box::new(|cc| {
            setup_cjk_fonts(&cc.egui_ctx);
            Box::new(SocketClientApp::default())
        }),
    )
}

fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_dir = std::path::Path::new(r"C:\Windows\Fonts");
    let candidates = [
        ("msyh.ttc", "Microsoft YaHei"),
        ("msyhbd.ttc", "Microsoft YaHei Bold"),
        ("SIMHEI.TTF", "SimHei"),
        ("simsun.ttc", "SimSun"),
        ("deng.ttf", "DengXian"),
        ("msyhl.ttc", "Microsoft YaHei Light"),
    ];

    for (file, _label) in &candidates {
        let path = font_dir.join(file);
        if !path.exists() {
            continue;
        }
        match std::fs::read(&path) {
            Ok(data) => {
                let name = format!("cjk_{}", file);
                fonts
                    .font_data
                    .insert(name.clone(), egui::FontData::from_owned(data));
                // Insert at front of proportional family
                if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                    list.insert(0, name.clone());
                }
                // Insert at front of monospace family
                if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                    list.insert(0, name);
                }
                ctx.set_fonts(fonts);
                return;
            }
            Err(_) => continue,
        }
    }

    // If no system CJK font found, keep defaults (will show boxes for CJK)
    eprintln!("未找到中文字体，中文可能无法正常显示");
}
