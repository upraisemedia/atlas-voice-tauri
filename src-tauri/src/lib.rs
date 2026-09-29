mod app;
mod commands;
mod error;

use tauri::RunEvent;

pub fn run() {
    let app = tauri::Builder::default()
        // Registered first so a second launch is forwarded to this instance
        // before any other plugin or window initialises.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app::windows::show_main(app);
        }))
        .plugin(app::logging::plugin())
        .setup(|app| {
            app::menu::install(app.handle())?;
            app::tray::install(app.handle())?;
            app::windows::attach(app.handle());
            // Phase 0 always opens the main window; later only when onboarding is needed.
            app::windows::show_main(app.handle());
            log::info!("Atlas Voice {} started", app.package_info().version);
            Ok(())
        })
        .on_menu_event(app::menu::handle_event)
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .build(tauri::generate_context!())
        .expect("failed to build the Tauri application");

    app.run(|handle, event| match event {
        // `code: None` means "last window closed": a menu bar app keeps running.
        // Explicit quits (`app.exit`) carry a code and are allowed through.
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        // Safety net: also reached on Cmd+Q from the system, which bypasses
        // `ExitRequested` (tauri#9198).
        RunEvent::Exit => app::lifecycle::shutdown(handle),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => app::windows::show_main(handle),
        _ => {}
    });
}
