use std::path::PathBuf;

use repomemo_api::RepoMemoCore;
use repomemo_domain::{AppSettings, Workspace};
use tauri::{Manager, State};

#[derive(Clone)]
struct AppState {
    core: RepoMemoCore,
}

#[tauri::command]
async fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<Workspace>, String> {
    state.core.list_workspaces().await.map_err(to_command_error)
}

#[tauri::command]
async fn create_workspace(
    state: State<'_, AppState>,
    name: String,
) -> Result<Workspace, String> {
    if name.trim().is_empty() {
        return Err("Workspace name is required.".to_owned());
    }

    state
        .core
        .create_workspace(name)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
fn get_app_settings(state: State<'_, AppState>) -> AppSettings {
    state.core.app_settings()
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = resolve_data_dir(app);
            let core = tauri::async_runtime::block_on(RepoMemoCore::boot(data_dir))
                .map_err(|error| format!("Failed to boot RepoMemo core: {error}"))?;

            app.manage(AppState { core });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_workspaces,
            create_workspace,
            get_app_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running RepoMemo");
}

fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| fallback_data_dir())
}

fn fallback_data_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".repomemo")
}

fn to_command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
