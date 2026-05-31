export interface Workspace {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
  settings: Record<string, unknown>;
}

export interface AppSettings {
  data_dir: string;
  ai_enabled: boolean;
  active_provider: string | null;
}
