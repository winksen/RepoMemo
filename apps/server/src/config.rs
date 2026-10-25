use crate::*;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub service_name: String,
    pub bind_address: SocketAddr,
    /// The environment folder to attach at startup (verified, and always
    /// inside `workspace-data/` when it comes from `REPOMEMO_SERVER_DATA_DIR`).
    /// `None` starts the server detached: an administrator picks the
    /// environment in the web app.
    pub data_dir: Option<PathBuf>,
    /// Where the database file lives (`REPOMEMO_DB_DIR`); `None` keeps it in the data folder.
    pub db_dir: Option<PathBuf>,
    /// Where the stored files live (`REPOMEMO_BLOB_DIR`); `None` keeps them in `<data folder>/blobs`.
    pub blob_dir: Option<PathBuf>,
    /// Where backups are made (`REPOMEMO_BACKUP_DIR`); `None` keeps them in `<data folder>/backups`.
    pub backup_dir: Option<PathBuf>,
    pub jwt_secret: String,
    /// Browser origins allowed to call the API (CORS). Empty: no CORS headers.
    pub allowed_origins: Vec<HeaderValue>,
    /// The refresh cookie's `SameSite` attribute (`REPOMEMO_COOKIE_SAMESITE`): `Strict` (the default; the web app and the
    /// API on one site), `Lax`, or `None` for a web app and an API on different sites (needs an `https://` public address).
    pub cookie_same_site: &'static str,
    /// Whether LibreOffice converts uploaded Office files for layout-accurate
    /// previews (`REPOMEMO_OFFICE_PREVIEWS`). Off by default: LibreOffice parses
    /// files strangers uploaded, so an operator turns it on on purpose, and
    /// only a release at or past the fixed ones is used.
    pub office_previews: bool,
    /// LibreOffice executable used for Office previews. When unset the server
    /// looks for it; without it the extracted previews are used.
    pub soffice: Option<PathBuf>,
    /// The folder that holds the `previews` folder (the converted PDFs, the conversions' work folders and
    /// LibreOffice's profile). Unset: the data folder. The container layout puts it on a volume of its own,
    /// which is all the converter container sees (`REPOMEMO_PREVIEW_ROOT`).
    pub preview_root: Option<PathBuf>,
    /// A command LibreOffice is started through (`REPOMEMO_SOFFICE_WRAPPER`),
    /// split on spaces; `{work}` and `{profile}` stand for the folders it needs.
    pub soffice_wrapper: Vec<String>,
    /// Key material for encrypting stored secrets (AI provider API keys).
    /// When unset a random key is kept in `secret.key` in the data directory.
    pub secret_key: Option<String>,
    /// Seals backups when set (`REPOMEMO_BACKUP_KEY`, at least 16 characters).
    pub backup_key: Option<String>,
    /// The key backups were sealed with before the last change of `REPOMEMO_BACKUP_KEY`
    /// (`REPOMEMO_BACKUP_KEY_PREVIOUS`): it opens those backups, and they can be sealed again with the new key.
    pub backup_previous_key: Option<String>,
    /// Another folder (a network share, say) every backup is copied to and checked in
    /// (`REPOMEMO_BACKUP_COPY_DIR`).
    pub backup_copy_dir: Option<std::path::PathBuf>,
    /// A folder of workspace template files added to the built-in ones (`REPOMEMO_TEMPLATES_DIR`, V0.2.189); read
    /// once, when the server starts.
    pub templates_dir: Option<std::path::PathBuf>,
    /// Anyone may create an account. When false, only the very first account
    /// can be registered; everyone else is added by an administrator after
    /// registering while it was open, or by the operator.
    pub allow_registration: bool,
    /// Who may create an organization: `administrators` (system and app administrators, the default)
    /// or `everyone` (any verified account). A system setting, changed in System › Settings.
    pub organization_creators: String,
    /// Who may link a repository that lives on this machine's disk: `system_admins` (the default)
    /// or `workspace_admins`. A system setting, changed in System › Settings.
    pub local_repository_linkers: String,
    /// Take the client address from `X-Forwarded-For` / `X-Real-IP`. Only for
    /// a server reachable solely through a reverse proxy that sets them.
    pub trust_proxy: bool,
    /// Proxy addresses besides loopback whose forwarding headers are read
    /// (`REPOMEMO_TRUSTED_PROXIES`).
    pub trusted_proxies: Vec<IpAddr>,
    /// The address people use, such as `https://memo.example.com`
    /// (`REPOMEMO_PUBLIC_URL`). HTTPS turns HSTS on.
    pub public_url: Option<String>,
    /// Allows binding beyond loopback over plain HTTP without a proxy
    /// (`REPOMEMO_ALLOW_INSECURE_HTTP`), for a trusted network only.
    pub allow_insecure_http: bool,
    pub max_upload_bytes: usize,
    /// How many bytes of uploads may be in flight at once, all senders together
    /// (`REPOMEMO_UPLOAD_INFLIGHT_MB`). Past it a new upload is answered `503`.
    pub upload_inflight_bytes: u64,
    /// The largest workspace archive that can be imported (`REPOMEMO_MAX_IMPORT_MB`).
    pub max_import_bytes: u64,
    /// How long a request may take, event streams excepted
    /// (`REPOMEMO_REQUEST_TIMEOUT_SECONDS`, 0 turns it off).
    pub request_timeout: Duration,
    /// How long a client may take to send its request headers
    /// (`REPOMEMO_HEADER_TIMEOUT_SECONDS`, 0 turns it off).
    pub header_read_timeout: Duration,
    /// How long running requests get to finish once the server is asked to stop (`REPOMEMO_SHUTDOWN_GRACE_SECONDS`).
    pub shutdown_grace: Duration,
    /// Event streams one person may hold open at once
    /// (`REPOMEMO_MAX_EVENT_STREAMS_PER_USER`).
    pub max_event_streams_per_user: usize,
    /// Artifacts indexed at once (`REPOMEMO_INDEX_CONCURRENCY`).
    pub index_concurrency: usize,
    /// Workspaces embedded at once (`REPOMEMO_EMBEDDING_CONCURRENCY`).
    pub embedding_concurrency: usize,
    /// Database connections that serve reads (`REPOMEMO_DB_READ_CONNECTIONS`).
    pub db_read_connections: u32,
    /// Requests served at once, all accounts together (`REPOMEMO_MAX_IN_FLIGHT`, 0 for no limit).
    /// Past it a request is refused at once with `503`.
    pub max_in_flight: usize,
    /// Requests served at once for one account (`REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT`, 0 for no
    /// limit). Past it a request is refused at once with `429`.
    pub max_in_flight_per_account: usize,
    /// The page cache of each read connection, in MiB (`REPOMEMO_SQLITE_CACHE_MB`, 0 keeps SQLite's own).
    pub db_read_cache_mb: u32,
    /// Memory-mapped reads of each read connection, in MiB (`REPOMEMO_SQLITE_MMAP_MB`, 0 turns them off).
    pub db_read_mmap_mb: u32,
    /// Memory for in-memory vector indexes, in MiB (`REPOMEMO_VECTOR_INDEX_MB`,
    /// 0 turns them off).
    pub vector_index_mb: u32,
    /// How long reading one document may take (`REPOMEMO_DOCUMENT_TIMEOUT_SECONDS`,
    /// 0 for no limit).
    pub document_time_limit: Duration,
    /// Memory one document reading may use, in MiB; 0 is no limit (`REPOMEMO_DOCUMENT_MEMORY_MB`).
    pub document_memory_mb: u64,
    /// Whether readings run in a child process (`REPOMEMO_DOCUMENT_READER` = `process`, the default)
    /// or on a thread (`thread`).
    pub document_reader_process: bool,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    /// Sign-ins and registrations per client address; token refreshes get
    /// four times as many.
    pub auth_quota: Quota,
    /// Consecutive failed sign-ins before an account is locked out; 0 never.
    pub login_max_failures: u32,
    pub login_lockout: Duration,
    /// AI requests per user per hour; a limit of 0 is unlimited.
    pub ai_quota: Quota,
    /// Requests to the heavy read endpoints (search, map, health, metrics,
    /// artifact query) per user per minute; 0 is unlimited.
    pub heavy_quota: Quota,
    /// Vision and embedding calls made in the background per workspace per
    /// day; 0 is unlimited.
    pub background_ai_per_day: u32,
    /// Storage one workspace may hold, in MiB (`REPOMEMO_WORKSPACE_STORAGE_MB`);
    /// 0 is unlimited.
    pub workspace_storage_mb: u64,
    /// Files one person may upload per hour (`REPOMEMO_UPLOADS_PER_HOUR`);
    /// 0 is unlimited.
    pub upload_quota: Quota,
    /// Workspaces one person may own (`REPOMEMO_WORKSPACES_PER_PERSON`);
    /// 0 is unlimited.
    pub workspaces_per_person: u32,
    /// Uploads are refused when less than this is free on the data disk, in
    /// MiB (`REPOMEMO_DISK_FLOOR_MB`); 0 turns the check off.
    pub disk_floor_mb: u64,
    /// Hours between automatic backups (`REPOMEMO_BACKUP_INTERVAL_HOURS`);
    /// 0 turns them off.
    pub backup_interval_hours: u64,
    /// Backups kept; older ones are deleted after a new one (`REPOMEMO_BACKUP_KEEP`).
    pub backup_keep: u32,
    /// Hosts AI providers may be configured at; `None` allows any host except
    /// link-local and cloud-metadata addresses.
    pub ai_allowed_hosts: Option<Vec<String>>,
    /// Hosts the mail server may be at although they resolve to private
    /// addresses (`REPOMEMO_SMTP_ALLOWED_HOSTS`, `*` for any).
    pub smtp_allowed_hosts: Vec<String>,
    /// Certificates (DER) trusted for the mail server besides the public
    /// roots (`REPOMEMO_SMTP_CA_FILE`, a PEM bundle).
    pub smtp_extra_roots: Vec<Vec<u8>>,
    /// Folders repository links must sit inside; empty allows any local folder.
    pub repo_roots: Vec<PathBuf>,
    /// Hosts remote repository links may use (`REPOMEMO_GIT_ALLOWED_HOSTS`:
    /// `host`, `host:port`, `*.suffix` or `*`). Empty turns remote links off.
    pub git_allowed_hosts: Vec<String>,
    /// Folders git may read although another account owns them
    /// (`REPOMEMO_GIT_SAFE_DIRECTORIES`).
    pub git_safe_directories: Vec<PathBuf>,
    /// How often linked repositories are checked for new commits; `None`
    /// turns automatic syncing off.
    pub repo_poll_interval: Option<Duration>,
    /// The most a remote repository's copy may grow to, in MiB (0: no cap).
    pub repo_clone_max_mb: u64,
    /// The longest one clone or fetch may run, in minutes.
    pub repo_clone_timeout_minutes: u64,
    pub maintenance: MaintenanceSettings,
    /// Accounts made system administrators at startup and when they
    /// register. The first account on a new server always becomes one.
    pub system_admin_emails: Vec<String>,
    /// System audit events are kept this many days; 0 keeps them forever.
    pub system_audit_retention_days: i64,
    /// What is logged, how it is printed and whether it is kept in files.
    pub logging: LogSettings,
    /// A new server shows an onboarding that creates its first system
    /// administrator. When off, the first account to register becomes one.
    pub setup_wizard: bool,
    /// The one-time code the onboarding asks for; generated and printed on
    /// the console at startup when unset.
    pub setup_code: Option<String>,
    /// The built web app the server serves itself (`REPOMEMO_WEB_DIR`, V0.4.20); `None` serves none.
    pub web_dir: Option<PathBuf>,
    /// The name `/config.json` gives the page when the server serves it (`REPOMEMO_PRODUCT_NAME`).
    pub product_name: String,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let jwt_secret = env_secret("REPOMEMO_JWT_SECRET")?.context(
            "REPOMEMO_JWT_SECRET (or REPOMEMO_JWT_SECRET_FILE) is required; use at least 32 random characters",
        )?;
        if jwt_secret.len() < 32 {
            bail!("REPOMEMO_JWT_SECRET must contain at least 32 characters");
        }
        if jwt_secret.chars().collect::<BTreeSet<_>>().len() < 8 {
            bail!("REPOMEMO_JWT_SECRET is too repetitive; use at least 32 random characters");
        }

        let bind_address = std::env::var("REPOMEMO_SERVER_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3020".to_owned())
            .parse()
            .context("REPOMEMO_SERVER_ADDR must be a valid socket address")?;
        let data_dir = match std::env::var("REPOMEMO_SERVER_DATA_DIR") {
            Ok(value) if !value.trim().is_empty() => Some(environment::resolve_setting(
                &environment::environments_root()?,
                &value,
            )?),
            _ => None,
        };
        let db_dir = env_folder("REPOMEMO_DB_DIR")?;
        let blob_dir = env_folder("REPOMEMO_BLOB_DIR")?;
        let backup_dir = env_folder("REPOMEMO_BACKUP_DIR")?;
        let web_dir = env_folder("REPOMEMO_WEB_DIR")?;
        if let Some(dir) = &web_dir {
            if !dir.join("index.html").is_file() {
                bail!("REPOMEMO_WEB_DIR ({}) has no index.html; build the web app (npm run build) and give the dist folder", dir.display());
            }
        }
        let product_name = env_text("REPOMEMO_PRODUCT_NAME").unwrap_or_else(|| "RepoMemo".to_owned());
        if product_name.chars().count() > 60
            || product_name
                .chars()
                .any(|character| character.is_control() || character == '"' || character == '\\')
        {
            bail!("REPOMEMO_PRODUCT_NAME must be one line of 60 characters or fewer, with no quote or backslash");
        }
        if data_dir.is_none() && (db_dir.is_some() || blob_dir.is_some() || backup_dir.is_some()) {
            bail!("REPOMEMO_DB_DIR, REPOMEMO_BLOB_DIR and REPOMEMO_BACKUP_DIR belong to one environment, so REPOMEMO_SERVER_DATA_DIR must be set too");
        }
        let allowed_origins = std::env::var("REPOMEMO_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| "http://127.0.0.1:3021".to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(|origin| {
                if origin == "*" {
                    bail!("REPOMEMO_ALLOWED_ORIGIN must list origins; \"*\" is not allowed");
                }
                origin
                    .trim_end_matches('/')
                    .parse::<HeaderValue>()
                    .with_context(|| format!("REPOMEMO_ALLOWED_ORIGIN has an invalid origin: {origin}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let secret_key = env_secret("REPOMEMO_SECRET_KEY")?;
        if secret_key
            .as_ref()
            .is_some_and(|key| key.chars().count() < repomemo_storage::MIN_MASTER_KEY_CHARS)
        {
            bail!(
                "REPOMEMO_SECRET_KEY must contain at least {} characters",
                repomemo_storage::MIN_MASTER_KEY_CHARS
            );
        }
        let backup_key = env_secret("REPOMEMO_BACKUP_KEY")?;
        if let Some(Err(message)) = backup_key.as_deref().map(crate::backup_crypto::validate_passphrase) {
            bail!("{message}");
        }
        let backup_previous_key = env_secret("REPOMEMO_BACKUP_KEY_PREVIOUS")?;
        if let Some(previous) = backup_previous_key.as_deref() {
            if backup_key.is_none() {
                bail!("REPOMEMO_BACKUP_KEY_PREVIOUS is the key before a change, so REPOMEMO_BACKUP_KEY must be set to the new one");
            }
            if crate::backup_crypto::validate_passphrase(previous).is_err() {
                bail!(
                    "REPOMEMO_BACKUP_KEY_PREVIOUS must contain at least {} characters.",
                    crate::backup_crypto::MIN_KEY_CHARS
                );
            }
            if backup_key.as_deref() == Some(previous) {
                bail!("REPOMEMO_BACKUP_KEY_PREVIOUS is the same as REPOMEMO_BACKUP_KEY; leave it unset until the key changes");
            }
        }
        // Unset: this machine's Ollama (its port) and OpenRouter only. `*` opens every host
        // (link-local and cloud-metadata addresses stay refused). An entry may end in `:port`.
        let ai_allowed_hosts = match env_text("REPOMEMO_AI_ALLOWED_HOSTS") {
            None => Some(
                repomemo_ai::DEFAULT_ALLOWED_HOSTS
                    .iter()
                    .map(|host| (*host).to_owned())
                    .collect(),
            ),
            Some(hosts) if hosts.trim() == "*" => None,
            Some(hosts) => {
                let hosts = hosts
                    .split(',')
                    .map(|host| host.trim().to_ascii_lowercase())
                    .filter(|host| !host.is_empty())
                    .collect::<Vec<_>>();
                for host in &hosts {
                    repomemo_ai::validate_allowed_host(host).context("REPOMEMO_AI_ALLOWED_HOSTS")?;
                }
                Some(hosts)
            }
        };
        let smtp_allowed_hosts = env_text("REPOMEMO_SMTP_ALLOWED_HOSTS")
            .map(|list| {
                list.split(',')
                    .map(|entry| entry.trim().to_ascii_lowercase())
                    .filter(|entry| !entry.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let smtp_extra_roots = match env_text("REPOMEMO_SMTP_CA_FILE") {
            Some(path) => {
                let pem = std::fs::read(&path)
                    .with_context(|| format!("REPOMEMO_SMTP_CA_FILE ({path}) could not be read"))?;
                repomemo_mail::parse_pem_certificates(&pem)
                    .map_err(|error| anyhow::anyhow!("REPOMEMO_SMTP_CA_FILE ({path}): {error}"))?
            }
            None => Vec::new(),
        };
        let git_allowed_hosts = env_text("REPOMEMO_GIT_ALLOWED_HOSTS")
            .map(|list| {
                list.split(',')
                    .map(|entry| entry.trim().to_ascii_lowercase())
                    .filter(|entry| !entry.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let repo_roots = std::env::var_os("REPOMEMO_REPO_ROOTS")
            .map(|roots| {
                std::env::split_paths(&roots)
                    .filter(|root| !root.as_os_str().is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let trusted_proxies = env_text("REPOMEMO_TRUSTED_PROXIES")
            .map(|list| {
                list.split(',')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                    .map(|entry| {
                        entry
                            .parse::<IpAddr>()
                            .with_context(|| format!("REPOMEMO_TRUSTED_PROXIES has an invalid address: {entry}"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let public_url = env_text("REPOMEMO_PUBLIC_URL").map(|url| url.trim_end_matches('/').to_owned());
        if public_url
            .as_deref()
            .is_some_and(|url| !url.starts_with("https://") && !url.starts_with("http://"))
        {
            bail!("REPOMEMO_PUBLIC_URL must start with https:// (or http://)");
        }
        let trust_proxy = env_flag("REPOMEMO_TRUST_PROXY", false)?;
        let allow_insecure_http = env_flag("REPOMEMO_ALLOW_INSECURE_HTTP", false)?;
        check_transport(bind_address, public_url.as_deref(), trust_proxy, allow_insecure_http)?;
        let cookie_same_site = parse_same_site(env_text("REPOMEMO_COOKIE_SAMESITE").as_deref(), public_url.as_deref())?;
        let repo_poll_seconds: u64 = env_parse("REPOMEMO_REPO_POLL_SECONDS", 300)?;
        let maintenance_minutes: u64 = env_parse("REPOMEMO_MAINTENANCE_INTERVAL_MINUTES", 60)?;
        let index_retry_hours: u64 = env_parse("REPOMEMO_INDEX_RETRY_HOURS", 6)?;

        Ok(Self {
            service_name: std::env::var("REPOMEMO_SERVICE_NAME").unwrap_or_else(|_| "repomemo-server".to_owned()),
            bind_address,
            data_dir,
            db_dir,
            blob_dir,
            backup_dir,
            jwt_secret,
            allowed_origins,
            cookie_same_site,
            office_previews: env_flag("REPOMEMO_OFFICE_PREVIEWS", false)?,
            preview_root: env_text("REPOMEMO_PREVIEW_ROOT").map(PathBuf::from),
            soffice: std::env::var_os("REPOMEMO_SOFFICE").map(PathBuf::from),
            soffice_wrapper: env_text("REPOMEMO_SOFFICE_WRAPPER")
                .map(|command| command.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default(),
            secret_key,
            backup_key,
            backup_previous_key,
            backup_copy_dir: env_text("REPOMEMO_BACKUP_COPY_DIR").map(std::path::PathBuf::from),
            templates_dir: env_text("REPOMEMO_TEMPLATES_DIR").map(std::path::PathBuf::from),
            // Closed by default: people join with invitation links (review M1).
            allow_registration: env_flag("REPOMEMO_ALLOW_REGISTRATION", false)?,
            organization_creators: "administrators".to_owned(),
            local_repository_linkers: "system_admins".to_owned(),
            trust_proxy,
            trusted_proxies,
            public_url,
            allow_insecure_http,
            max_upload_bytes: env_parse::<usize>("REPOMEMO_MAX_UPLOAD_MB", 10)?.clamp(1, 1024) * 1024 * 1024,
            upload_inflight_bytes: env_parse::<u64>("REPOMEMO_UPLOAD_INFLIGHT_MB", 512)?.clamp(1, 1_048_576)
                * 1024
                * 1024,
            max_import_bytes: env_parse::<u64>("REPOMEMO_MAX_IMPORT_MB", 2048)?.clamp(1, 1_048_576) * 1024 * 1024,
            request_timeout: Duration::from_secs(env_parse("REPOMEMO_REQUEST_TIMEOUT_SECONDS", 120)?),
            header_read_timeout: Duration::from_secs(env_parse("REPOMEMO_HEADER_TIMEOUT_SECONDS", 30)?),
            shutdown_grace: Duration::from_secs(env_parse("REPOMEMO_SHUTDOWN_GRACE_SECONDS", 30)?),
            max_event_streams_per_user: env_parse::<usize>("REPOMEMO_MAX_EVENT_STREAMS_PER_USER", 10)?.clamp(1, 1000),
            index_concurrency: env_parse::<usize>("REPOMEMO_INDEX_CONCURRENCY", 2)?.clamp(1, 16),
            embedding_concurrency: env_parse::<usize>("REPOMEMO_EMBEDDING_CONCURRENCY", 1)?.clamp(1, 8),
            db_read_connections: env_parse::<u32>("REPOMEMO_DB_READ_CONNECTIONS", 4)?.clamp(1, 64),
            max_in_flight: env_parse::<usize>(
                "REPOMEMO_MAX_IN_FLIGHT",
                (env_parse::<usize>("REPOMEMO_DB_READ_CONNECTIONS", 4)?.clamp(1, 64) * 16).max(32),
            )?
            .min(100_000),
            max_in_flight_per_account: env_parse::<usize>(
                "REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT",
                (env_parse::<usize>("REPOMEMO_DB_READ_CONNECTIONS", 4)?.clamp(1, 64) * 8).max(16),
            )?
            .min(100_000),
            db_read_cache_mb: env_parse::<u32>("REPOMEMO_SQLITE_CACHE_MB", repomemo_storage::DEFAULT_READ_CACHE_MB)?
                .min(4 * 1024),
            db_read_mmap_mb: env_parse::<u32>("REPOMEMO_SQLITE_MMAP_MB", repomemo_storage::DEFAULT_READ_MMAP_MB)?
                .min(64 * 1024),
            vector_index_mb: env_parse::<u32>("REPOMEMO_VECTOR_INDEX_MB", repomemo_storage::DEFAULT_VECTOR_INDEX_MB)?
                .min(64 * 1024),
            document_time_limit: Duration::from_secs(env_parse("REPOMEMO_DOCUMENT_TIMEOUT_SECONDS", 60)?),
            document_memory_mb: env_parse("REPOMEMO_DOCUMENT_MEMORY_MB", repomemo_api::DEFAULT_MEMORY_LIMIT_MB)?,
            document_reader_process: match env_text("REPOMEMO_DOCUMENT_READER").as_deref() {
                None | Some("process") => true,
                Some("thread") => false,
                Some(other) => bail!("REPOMEMO_DOCUMENT_READER must be \"process\" or \"thread\", not \"{other}\""),
            },
            access_token_ttl_minutes: env_parse::<i64>("REPOMEMO_ACCESS_TOKEN_TTL_MINUTES", 60)?.clamp(5, 24 * 60),
            refresh_token_ttl_days: env_parse::<i64>("REPOMEMO_REFRESH_TOKEN_TTL_DAYS", 30)?.clamp(1, 365),
            auth_quota: Quota::per_minute(env_parse("REPOMEMO_AUTH_REQUESTS_PER_MINUTE", 30)?),
            login_max_failures: env_parse("REPOMEMO_LOGIN_MAX_FAILURES", 10)?,
            login_lockout: Duration::from_secs(
                60 * env_parse::<u64>("REPOMEMO_LOGIN_LOCKOUT_MINUTES", 15)?.clamp(1, 24 * 60),
            ),
            ai_quota: Quota::per_hour(env_parse("REPOMEMO_AI_REQUESTS_PER_HOUR", 120)?),
            heavy_quota: Quota::per_minute(env_parse("REPOMEMO_HEAVY_REQUESTS_PER_MINUTE", 240)?),
            background_ai_per_day: env_parse("REPOMEMO_BACKGROUND_AI_PER_DAY", 2000)?,
            workspace_storage_mb: env_parse("REPOMEMO_WORKSPACE_STORAGE_MB", 20 * 1024)?,
            upload_quota: Quota::per_hour(env_parse("REPOMEMO_UPLOADS_PER_HOUR", 600)?),
            workspaces_per_person: env_parse("REPOMEMO_WORKSPACES_PER_PERSON", 25)?,
            disk_floor_mb: env_parse("REPOMEMO_DISK_FLOOR_MB", 1024)?,
            backup_interval_hours: env_parse("REPOMEMO_BACKUP_INTERVAL_HOURS", 0)?,
            backup_keep: env_parse::<u32>("REPOMEMO_BACKUP_KEEP", 7)?.max(1),
            ai_allowed_hosts,
            smtp_allowed_hosts,
            smtp_extra_roots,
            repo_roots,
            git_allowed_hosts,
            git_safe_directories: std::env::var_os("REPOMEMO_GIT_SAFE_DIRECTORIES")
                .map(|folders| {
                    std::env::split_paths(&folders)
                        .filter(|folder| !folder.as_os_str().is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            repo_poll_interval: (repo_poll_seconds > 0).then(|| Duration::from_secs(repo_poll_seconds.max(30))),
            repo_clone_max_mb: env_parse("REPOMEMO_REPO_CLONE_MAX_MB", 2048)?,
            repo_clone_timeout_minutes: env_parse::<u64>("REPOMEMO_REPO_CLONE_TIMEOUT_MINUTES", 30)?.max(1),
            maintenance: MaintenanceSettings {
                interval: Duration::from_secs(60 * maintenance_minutes),
                blob_gc: env_flag("REPOMEMO_BLOB_GC", true)?,
                job_retention_days: env_parse("REPOMEMO_JOB_RETENTION_DAYS", 90)?,
                index_retry_interval: Duration::from_secs(3600 * index_retry_hours.max(1)),
            },
            system_admin_emails: env_text("REPOMEMO_SYSTEM_ADMIN_EMAILS")
                .map(|emails| {
                    emails
                        .split(',')
                        .map(|email| email.trim().to_lowercase())
                        .filter(|email| !email.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            system_audit_retention_days: env_parse("REPOMEMO_SYSTEM_AUDIT_RETENTION_DAYS", 365)?,
            logging: LogSettings::from_env()?,
            setup_wizard: env_flag("REPOMEMO_SETUP_WIZARD", true)?,
            web_dir,
            product_name,
            setup_code: match env_secret("REPOMEMO_SETUP_CODE")? {
                Some(code) if code.chars().filter(char::is_ascii_alphanumeric).count() < 12 => {
                    bail!("REPOMEMO_SETUP_CODE must contain at least 12 letters or digits")
                }
                code => code,
            },
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(data_dir: PathBuf) -> Self {
        Self {
            service_name: "repomemo-server-test".to_owned(),
            bind_address: "127.0.0.1:0".parse().unwrap(),
            data_dir: Some(data_dir),
            db_dir: None,
            blob_dir: None,
            backup_dir: None,
            jwt_secret: "test-secret-that-is-long-enough-for-jwt-signing".to_owned(),
            allowed_origins: Vec::new(),
            cookie_same_site: "Strict",
            office_previews: false,
            preview_root: None,
            soffice: None,
            soffice_wrapper: Vec::new(),
            secret_key: None,
            backup_key: None,
            backup_previous_key: None,
            backup_copy_dir: None,
            templates_dir: None,
            allow_registration: true,
            // The tests of everything else sign in as ordinary people who create their own organizations
            // and link folders; the tests of these two settings set them to the server's defaults.
            organization_creators: "everyone".to_owned(),
            local_repository_linkers: "workspace_admins".to_owned(),
            trust_proxy: false,
            trusted_proxies: Vec::new(),
            public_url: None,
            allow_insecure_http: false,
            max_upload_bytes: 10 * 1024 * 1024,
            upload_inflight_bytes: 256 * 1024 * 1024,
            max_import_bytes: 64 * 1024 * 1024,
            request_timeout: Duration::from_secs(120),
            header_read_timeout: Duration::from_secs(30),
            shutdown_grace: Duration::from_secs(5),
            max_event_streams_per_user: 10,
            index_concurrency: 2,
            embedding_concurrency: 1,
            db_read_connections: 4,
            max_in_flight: 256,
            max_in_flight_per_account: 128,
            db_read_cache_mb: repomemo_storage::DEFAULT_READ_CACHE_MB,
            db_read_mmap_mb: repomemo_storage::DEFAULT_READ_MMAP_MB,
            vector_index_mb: 64,
            document_time_limit: Duration::from_secs(60),
            document_memory_mb: repomemo_api::DEFAULT_MEMORY_LIMIT_MB,
            // The test program cannot act as a reader process.
            document_reader_process: false,
            access_token_ttl_minutes: 60,
            refresh_token_ttl_days: 30,
            // Tests register many accounts from one address.
            auth_quota: Quota::per_minute(0),
            login_max_failures: 10,
            login_lockout: Duration::from_secs(15 * 60),
            ai_quota: Quota::per_hour(0),
            heavy_quota: Quota::per_minute(0),
            background_ai_per_day: 0,
            workspace_storage_mb: 0,
            upload_quota: Quota::per_hour(0),
            workspaces_per_person: 0,
            disk_floor_mb: 0,
            backup_interval_hours: 0,
            backup_keep: 7,
            ai_allowed_hosts: None,
            smtp_allowed_hosts: Vec::new(),
            smtp_extra_roots: Vec::new(),
            repo_roots: Vec::new(),
            git_allowed_hosts: Vec::new(),
            git_safe_directories: Vec::new(),
            repo_poll_interval: None,
            repo_clone_max_mb: 2048,
            repo_clone_timeout_minutes: 30,
            maintenance: MaintenanceSettings {
                interval: Duration::ZERO,
                ..MaintenanceSettings::default()
            },
            system_admin_emails: Vec::new(),
            system_audit_retention_days: 365,
            logging: LogSettings {
                to_file: false,
                ..LogSettings::default()
            },
            // Tests register accounts directly; the onboarding has its own tests.
            setup_wizard: false,
            setup_code: None,
            web_dir: None,
            product_name: "RepoMemo".to_owned(),
        }
    }
}

pub(crate) fn env_text(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// A secret given as the variable itself (`NAME`) or as the path of a file that holds it
/// (`NAME_FILE`, what Docker and Kubernetes secrets mount). Giving both is refused, so a
/// stale variable can never hide the file that was meant to replace it.
pub(crate) fn env_secret(name: &str) -> Result<Option<String>> {
    secret_from(name, env_text(name), env_text(&format!("{name}_FILE")))
}

fn secret_from(name: &str, value: Option<String>, file: Option<String>) -> Result<Option<String>> {
    let Some(path) = file else {
        return Ok(value);
    };
    if value.is_some() {
        bail!("{name} and {name}_FILE are both set; give the secret one way only");
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("{name}_FILE ({path}) could not be read"))?;
    let text = text.trim();
    if text.is_empty() {
        bail!("{name}_FILE ({path}) is empty");
    }
    Ok(Some(text.to_owned()))
}

#[cfg(test)]
mod secret_tests {
    use super::secret_from;

    fn secret_file(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("repomemo-secret-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn a_variable_alone_is_the_secret() {
        let secret = secret_from("REPOMEMO_BACKUP_KEY", Some("abc".to_owned()), None).unwrap();
        assert_eq!(secret.as_deref(), Some("abc"));
        assert_eq!(secret_from("REPOMEMO_BACKUP_KEY", None, None).unwrap(), None);
    }

    #[test]
    fn a_file_alone_is_the_secret_without_its_line_ending() {
        let path = secret_file("from-a-file\r\n");
        let secret = secret_from("REPOMEMO_BACKUP_KEY", None, Some(path.display().to_string())).unwrap();
        assert_eq!(secret.as_deref(), Some("from-a-file"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_variable_and_a_file_for_the_same_secret_are_refused() {
        let path = secret_file("from-a-file");
        let error = secret_from(
            "REPOMEMO_BACKUP_KEY",
            Some("abc".to_owned()),
            Some(path.display().to_string()),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("REPOMEMO_BACKUP_KEY and REPOMEMO_BACKUP_KEY_FILE are both set"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_missing_or_empty_file_is_named() {
        let missing = secret_from(
            "REPOMEMO_BACKUP_KEY",
            None,
            Some("/nonexistent/repomemo-secret".to_owned()),
        )
        .unwrap_err();
        assert!(missing
            .to_string()
            .contains("REPOMEMO_BACKUP_KEY_FILE (/nonexistent/repomemo-secret) could not be read"));
        let path = secret_file(" \n");
        let empty = secret_from("REPOMEMO_BACKUP_KEY", None, Some(path.display().to_string())).unwrap_err();
        assert!(empty.to_string().contains("is empty"));
        std::fs::remove_file(path).unwrap();
    }
}

/// The refresh cookie's `SameSite` attribute from `REPOMEMO_COOKIE_SAMESITE` (V0.4.17). `None` is only accepted
/// with an `https://` public address, since a browser takes `SameSite=None` only together with `Secure`.
pub(crate) fn parse_same_site(value: Option<&str>, public_url: Option<&str>) -> Result<&'static str> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        None | Some("strict") => Ok("Strict"),
        Some("lax") => Ok("Lax"),
        Some("none") => {
            if public_url.is_some_and(|url| url.starts_with("https://")) {
                Ok("None")
            } else {
                bail!("REPOMEMO_COOKIE_SAMESITE=none needs REPOMEMO_PUBLIC_URL to be an https:// address, since a browser takes SameSite=None only with Secure")
            }
        }
        Some(other) => bail!("REPOMEMO_COOKIE_SAMESITE must be strict, lax or none, not {other}"),
    }
}

/// A folder given by an environment variable: an absolute path, or unset.
fn env_folder(name: &str) -> Result<Option<PathBuf>> {
    let Some(value) = env_text(name) else {
        return Ok(None);
    };
    let path = PathBuf::from(&value);
    if !path.is_absolute() {
        bail!("{name} must be an absolute path, not {value}");
    }
    Ok(Some(path))
}

/// Parses an optional environment variable, with a clear error for a value
/// that is set but invalid.
pub(crate) fn env_parse<T: std::str::FromStr>(name: &str, default: T) -> Result<T> {
    let Some(raw) = env_text(name) else {
        return Ok(default);
    };
    raw.parse::<T>()
        .map_err(|_| anyhow::anyhow!("{name} has an invalid value: {raw}"))
}

/// Refuses to serve plain HTTP beyond this machine unless a TLS proxy is in
/// front (an `https://` public address, or proxy trust) or the operator opted
/// in for a trusted network (review M9).
pub(crate) fn check_transport(
    bind: SocketAddr,
    public_url: Option<&str>,
    trust_proxy: bool,
    allow_insecure_http: bool,
) -> Result<()> {
    let behind_tls = public_url.is_some_and(|url| url.starts_with("https://")) || trust_proxy;
    if !bind.ip().is_loopback() && !behind_tls && !allow_insecure_http {
        bail!(
            "REPOMEMO_SERVER_ADDR ({bind}) is reachable from other machines over plain HTTP. Put a TLS reverse proxy in front and set REPOMEMO_PUBLIC_URL=https://… (or REPOMEMO_TRUST_PROXY=true), or set REPOMEMO_ALLOW_INSECURE_HTTP=true for a trusted network only."
        );
    }
    Ok(())
}

impl ServerConfig {
    /// Whose forwarding headers name the client (see `security::client_ip`).
    pub fn proxy_trust(&self) -> security::ProxyTrust {
        security::ProxyTrust {
            enabled: self.trust_proxy,
            proxies: self.trusted_proxies.clone(),
        }
    }
}

/// An optional on/off environment variable: true/false, 1/0, yes/no, on/off.
pub(crate) fn env_flag(name: &str, default: bool) -> Result<bool> {
    let Some(raw) = env_text(name) else {
        return Ok(default);
    };
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("{name} must be true or false, not {raw}"),
    }
}

/// The configuration requests need at run time.
#[derive(Debug, Clone)]
pub(crate) struct RuntimeSettings {
    pub(crate) service_name: String,
    pub(crate) allow_registration: bool,
    /// `administrators` or `everyone` (see [`ServerConfig::organization_creators`]).
    pub(crate) organization_creators: String,
    /// `system_admins` or `workspace_admins` (see [`ServerConfig::local_repository_linkers`]).
    pub(crate) local_repository_linkers: String,
    pub(crate) proxy: security::ProxyTrust,
    /// The refresh cookie carries `Secure` (the public address is HTTPS).
    pub(crate) secure_cookies: bool,
    /// `Strict`, `Lax` or `None` (see [`ServerConfig::cookie_same_site`]).
    pub(crate) cookie_same_site: &'static str,
    /// The origins that may call the API, as text; a request that presents the refresh cookie from another one is
    /// refused when the cookie is not `Strict`.
    pub(crate) allowed_origins: Vec<String>,
    pub(crate) max_event_streams_per_user: usize,
    pub(crate) access_token_ttl_minutes: i64,
    pub(crate) refresh_token_ttl_days: i64,
    pub(crate) auth_quota: Quota,
    pub(crate) login_max_failures: u32,
    pub(crate) login_lockout: Duration,
    pub(crate) ai_quota: Quota,
    pub(crate) heavy_quota: Quota,
    pub(crate) background_ai_per_day: u32,
    pub(crate) workspace_storage_mb: u64,
    pub(crate) upload_quota: Quota,
    pub(crate) workspaces_per_person: u32,
    pub(crate) disk_floor_mb: u64,
    pub(crate) backup_interval_hours: u64,
    pub(crate) backup_keep: u32,
    pub(crate) repo_poll_interval: Option<Duration>,
    pub(crate) repo_clone_max_mb: u64,
    pub(crate) repo_clone_timeout_minutes: u64,
    pub(crate) maintenance: MaintenanceSettings,
    pub(crate) system_audit_retention_days: i64,
    pub(crate) system_admin_emails: Vec<String>,
    pub(crate) logging: LogSettings,
}

impl RuntimeSettings {
    /// The settings the environment gives, before system administrators'
    /// run-time changes.
    pub(crate) fn from_config(config: &ServerConfig) -> Self {
        Self {
            service_name: config.service_name.clone(),
            allow_registration: config.allow_registration,
            organization_creators: config.organization_creators.clone(),
            local_repository_linkers: config.local_repository_linkers.clone(),
            proxy: config.proxy_trust(),
            secure_cookies: config
                .public_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://")),
            cookie_same_site: config.cookie_same_site,
            allowed_origins: config
                .allowed_origins
                .iter()
                .filter_map(|origin| origin.to_str().ok())
                .map(|origin| origin.trim_end_matches('/').to_ascii_lowercase())
                .collect(),
            max_event_streams_per_user: config.max_event_streams_per_user,
            access_token_ttl_minutes: config.access_token_ttl_minutes,
            refresh_token_ttl_days: config.refresh_token_ttl_days,
            auth_quota: config.auth_quota,
            login_max_failures: config.login_max_failures,
            login_lockout: config.login_lockout,
            ai_quota: config.ai_quota,
            heavy_quota: config.heavy_quota,
            background_ai_per_day: config.background_ai_per_day,
            workspace_storage_mb: config.workspace_storage_mb,
            upload_quota: config.upload_quota,
            workspaces_per_person: config.workspaces_per_person,
            disk_floor_mb: config.disk_floor_mb,
            backup_interval_hours: config.backup_interval_hours,
            backup_keep: config.backup_keep,
            repo_poll_interval: config.repo_poll_interval,
            repo_clone_max_mb: config.repo_clone_max_mb,
            repo_clone_timeout_minutes: config.repo_clone_timeout_minutes,
            maintenance: config.maintenance.clone(),
            system_audit_retention_days: config.system_audit_retention_days,
            system_admin_emails: config.system_admin_emails.clone(),
            logging: config.logging.clone(),
        }
    }
}
#[cfg(test)]
mod same_site_tests {
    use super::parse_same_site;

    #[test]
    fn strict_is_the_default_and_the_others_are_named() {
        assert_eq!(parse_same_site(None, None).unwrap(), "Strict");
        assert_eq!(parse_same_site(Some("STRICT"), None).unwrap(), "Strict");
        assert_eq!(parse_same_site(Some("lax"), None).unwrap(), "Lax");
        assert!(parse_same_site(Some("sometimes"), None).is_err());
    }

    #[test]
    fn none_needs_an_https_public_address() {
        assert_eq!(
            parse_same_site(Some("none"), Some("https://memo.example.com")).unwrap(),
            "None"
        );
        let error = parse_same_site(Some("none"), Some("http://memo.example.com")).unwrap_err();
        assert!(error.to_string().contains("https://"));
        assert!(parse_same_site(Some("none"), None).is_err());
    }
}
