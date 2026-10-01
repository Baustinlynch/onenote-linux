mod config;
mod discovery;
mod keys;
mod nav;
mod notify;
mod settings;
mod setup;
mod tray;
mod window;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::focus_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let cfg = config::Config::load(&handle);
            let first_run = setup::is_first_run(&handle);

            // The main window is always built, so the tray has something to
            // show, but it stays hidden behind the setup wizard.
            window::build_main(&handle, &cfg)?;
            tray::build(&handle)?;

            if first_run {
                setup::show_setup_window(&handle);
                if let Some(main) = handle.get_webview_window(window::MAIN_LABEL) {
                    let _ = main.hide();
                }
            } else if !cfg.start_minimized {
                window::show_main(&handle);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            let label = window.label().to_string();
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Settings and setup are normal dialogs: let them close.
                    if label != window::SETTINGS_LABEL
                        && label != setup::LABEL
                        && config::Config::load(window.app_handle()).close_to_tray
                    {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                    if label == window::MAIN_LABEL =>
                {
                    window::persist_state(window.app_handle());
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start onenote-linux")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // Closing the last window is not a quit request while the tray
                // icon is live. An explicit `app.exit(0)` carries a code and
                // is always honoured.
                if code.is_none() && config::Config::load(app).close_to_tray {
                    api.prevent_exit();
                }
            }
        });
}
