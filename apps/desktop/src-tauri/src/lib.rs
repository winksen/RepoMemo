use std::path::PathBuf;

use repomemo_api::RepoMemoCore;
use repomemo_domain::{
    AppSettings, ArtifactDetail, ArtifactSummary, AskAnswer, AskRequest, ImportReport,
    ImportRequest, IndexingJobStatus, ProviderSettings, ProviderTestResult, SearchRequest,
    SearchResult, SummaryResult, Symbol, SymbolSearchResult, Workspace, WorkspaceOverview,
};
use serde::Deserialize;
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
async fn create_workspace(state: State<'_, AppState>, name: String) -> Result<Workspace, String> {
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
async fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.core.app_settings().await.map_err(to_command_error)
}

#[tauri::command]
async fn import_paths(
    state: State<'_, AppState>,
    workspace_id: String,
    paths: Vec<String>,
) -> Result<ImportReport, String> {
    state
        .core
        .import_paths(ImportRequest {
            workspace_id,
            paths,
        })
        .await
        .map_err(to_command_error)
}

#[derive(Debug, Deserialize)]
struct ImportTextRequest {
    workspace_id: String,
    title: String,
    content: String,
    language: Option<String>,
}

#[tauri::command]
async fn import_text(
    state: State<'_, AppState>,
    request: ImportTextRequest,
) -> Result<ArtifactSummary, String> {
    state
        .core
        .import_text(
            request.workspace_id,
            request.title,
            request.content,
            request.language,
        )
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn list_artifacts(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<Vec<ArtifactSummary>, String> {
    state
        .core
        .list_artifacts(workspace_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn get_artifact(
    state: State<'_, AppState>,
    artifact_id: String,
) -> Result<ArtifactDetail, String> {
    state
        .core
        .get_artifact(artifact_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn index_artifact(
    state: State<'_, AppState>,
    artifact_id: String,
) -> Result<IndexingJobStatus, String> {
    state
        .core
        .index_artifact(artifact_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn index_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<IndexingJobStatus, String> {
    state
        .core
        .index_workspace(workspace_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn get_workspace_overview(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<WorkspaceOverview, String> {
    state
        .core
        .workspace_overview(workspace_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn search_workspace(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<Vec<SearchResult>, String> {
    state
        .core
        .search_workspace(request)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn list_symbols(
    state: State<'_, AppState>,
    artifact_id: String,
) -> Result<Vec<Symbol>, String> {
    state
        .core
        .list_symbols(artifact_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn search_symbols(
    state: State<'_, AppState>,
    workspace_id: String,
    query: String,
) -> Result<Vec<SymbolSearchResult>, String> {
    state
        .core
        .search_symbols(workspace_id, query)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn list_provider_settings(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<Vec<ProviderSettings>, String> {
    state
        .core
        .list_provider_settings(workspace_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn save_provider_settings(
    state: State<'_, AppState>,
    settings: ProviderSettings,
) -> Result<ProviderSettings, String> {
    state
        .core
        .save_provider_settings(settings)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn test_provider(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ProviderTestResult, String> {
    state
        .core
        .test_provider(provider_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn summarize_artifact(
    state: State<'_, AppState>,
    artifact_id: String,
    provider_id: String,
) -> Result<SummaryResult, String> {
    state
        .core
        .summarize_artifact(artifact_id, provider_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn summarize_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    provider_id: String,
) -> Result<SummaryResult, String> {
    state
        .core
        .summarize_workspace(workspace_id, provider_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn embed_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    provider_id: String,
) -> Result<IndexingJobStatus, String> {
    state
        .core
        .embed_workspace(workspace_id, provider_id)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn ask_workspace(
    state: State<'_, AppState>,
    request: AskRequest,
) -> Result<AskAnswer, String> {
    state
        .core
        .ask_workspace(request)
        .await
        .map_err(to_command_error)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            get_app_settings,
            import_paths,
            import_text,
            list_artifacts,
            get_artifact,
            index_artifact,
            index_workspace,
            get_workspace_overview,
            search_workspace,
            list_symbols,
            search_symbols,
            list_provider_settings,
            save_provider_settings,
            test_provider,
            summarize_artifact,
            summarize_workspace,
            embed_workspace,
            ask_workspace
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
