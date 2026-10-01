mod config;
mod discovery;
mod keys;
mod nav;
mod notify;
mod settings;
mod setup;
mod toolbar;
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
            let cfg = config::Config::load_cached(&handle);
            // A config on disk means setup already ran, even if it was written
            // by an older build that had no `setup_complete` flag.
            let first_run = !config::Config::exists();

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
                        && config::Config::get()
                            .map(|c| c.close_to_tray)
                            .unwrap_or(true)
                    {
                        // A drag throttles geometry writes, so capture the
                        // final position before hiding.
                        window::persist_state_now(window.app_handle());
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
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // Closing the last window is not a quit request while the tray
                // icon is live. An explicit `app.exit(0)` carries a code and
                // is always honoured.
                if code.is_none()
                    && config::Config::get()
                        .map(|c| c.close_to_tray)
                        .unwrap_or(true)
                {
                    // Still remember where the window was before we stay alive.
                    window::persist_state_now(_app);
                    api.prevent_exit();
                } else {
                    window::persist_state_now(_app);
                }
            }
        });
}
