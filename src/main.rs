mod config;
mod discovery;
mod keys;
mod nav;
mod notify;
mod settings;
mod setup;
mod toolbar;
mod tray;
mod ublock;
mod window;

use tauri::Manager;

fn main() {
    #[cfg(target_os = "linux")]
    {
        use std::env;
        // Workaround for blank/white windows on some NVIDIA setups. Only set
        // when the user has not chosen a value, and keep compositing enabled
        // (disabling it is a large rendering-performance regression).
        if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }

        // WebKitGTK runs a large number of threads across its UI, web, network
        // and GPU processes. glibc gives each thread its own malloc arena (up
        // to 8 per core by default) and arenas rarely return freed memory to
        // the OS, which inflates the app's RSS. Capping arenas keeps that
        // overhead bounded. Only set it when the user has not chosen a value,
        // and allow a larger value for anyone who favours allocation speed.
        if env::var_os("MALLOC_ARENA_MAX").is_none() {
            env::set_var("MALLOC_ARENA_MAX", "2");
        }
    }

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
