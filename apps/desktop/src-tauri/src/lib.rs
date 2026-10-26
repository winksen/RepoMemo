mod config;
mod connection;

use tauri::Manager;

use crate::connection::Connection;

/// The desktop shell is a client of a server (V0.2.49): it holds the chosen server's address, the session in
/// the operating system's credential store and the window that may call that server and no other. Everything
/// else is the web client's. The local workspace and the commands that served it were removed in V0.3.6.
pub fn run() {
    tauri::Builder::default()
        // The window may call no server until one is chosen; `connection` adds the chosen one (V0.2.49).
        .plugin(tauri_plugin_http::init())
        .manage(Connection::default())
        .setup(|app| {
            // The administrator's file is read once, first: it decides the server and the name.
            let file = config::load(app.handle());
            // The uninstaller (V0.4.50) starts the program to forget the saved session; it opens no window.
            if connection::asked_to_forget(std::env::args()) {
                connection::forget_saved(app.handle(), &file);
                std::process::exit(0);
            }
            connection::start(app.handle(), &app.state::<Connection>(), &file)?;
            let name = file.config.name.clone();
            app.manage(file);

            // The window is made here, not by the configuration, so it can refuse to leave the bundled client.
            let window = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or("the configuration has no window")?;
            let mut builder =
                tauri::WebviewWindowBuilder::from_config(app, &window)?.on_navigation(connection::is_app_url);
            if let Some(name) = name {
                builder = builder.title(name);
            }
            builder.build()?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config::desktop_settings,
            connection::current_server,
            connection::server_locked,
            connection::prepare_server,
            connection::remember_server,
            connection::forget_server,
            connection::save_session,
            connection::load_session,
            connection::clear_session,
            connection::open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running RepoMemo");
}
