mod chat;
mod history;
pub mod i18n;
mod overlay;
mod providers;
mod selection;
mod settings;
mod tts;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(chat::ChatState::default())
        .invoke_handler(tauri::generate_handler![
            overlay::hide_overlay_cmd,
            overlay::set_overlay_height,
            overlay::open_settings,
            overlay::copy_text,
            overlay::hide_tray_menu,
            overlay::resize_tray_menu,
            overlay::tray_action,
            providers::list_providers,
            providers::upsert_provider,
            providers::delete_provider,
            providers::set_provider_key,
            providers::list_models,
            settings::get_settings,
            settings::save_settings,
            chat::start_chat,
            chat::abort_chat,
            history::list_chats,
            history::list_exchanges,
            history::get_chat,
            history::delete_chat,
            history::clear_history,
            tts::set_tts_key,
            tts::get_tts_key_status,
            tts::stop_tts_cmd,
            tts::test_tts,
            tts::get_local_tts_status,
            tts::get_tts_state,
        ]);

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                overlay::on_hotkey(app);
            }))
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))
            .plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, shortcut, event| {
                        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                            overlay::handle_global_shortcut(app, shortcut);
                        }
                    })
                    .build(),
            );
    }

    #[cfg(debug_assertions)]
    {
        if std::env::var("DUMBO_MCP_BRIDGE").map(|v| v == "1").unwrap_or(false) {
            builder = builder.plugin(tauri_plugin_mcp_bridge::init());
        }
    }

    builder
        .setup(|app| {
            settings::prepare_config_dir(app.handle())?;
            overlay::setup_tray(app.handle())?;
            overlay::harden_overlay_hwnd(app.handle());
            overlay::setup_blur_hide(app.handle());
            let _ = overlay::ensure_settings_window(app.handle());
            overlay::setup_settings_lifecycle(app.handle());
            let _ = overlay::ensure_tray_menu(app.handle());
            overlay::show_splash(app.handle());
            providers::ensure_defaults(app.handle())?;
            history::init_db(app.handle())?;
            tts::init(app.handle());
            let loaded = settings::load(app.handle())?;
            if let Err(err) = overlay::register_hotkeys(app.handle(), &loaded.hotkey, &loaded.tts_hotkey) {
                eprintln!("Hotkeys nicht aktiv: {err}");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Dumbo");
}
