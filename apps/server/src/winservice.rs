//! The server as a Windows service (V0.4.33): `repomemo-server service install|uninstall|run`.
//!
//! `install` registers this program with the service manager (starts at boot, as `NT AUTHORITY\LocalService`, restarts
//! after a failure); `run` is what the service manager starts: it answers the manager, reads the settings file, goes to
//! the working folder and runs the server until the manager says stop; `uninstall` stops and removes it. On any other
//! system the command only says to use systemd.

use std::path::PathBuf;

/// What the command line asked of `service`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ServiceCommand {
    Install(ServiceOptions),
    Uninstall(ServiceOptions),
    Run(ServiceOptions),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct ServiceOptions {
    pub name: String,
    /// The folder the server runs in; `workspace-data/` is written under it.
    pub workdir: Option<PathBuf>,
    /// A file of `NAME=value` lines read into the environment before the server starts.
    pub env_file: Option<PathBuf>,
    /// The account the service runs as (install only).
    pub account: String,
}

const DEFAULT_NAME: &str = "RepoMemo";
const DEFAULT_ACCOUNT: &str = "NT AUTHORITY\\LocalService";

/// Reads the words after `service`.
pub(crate) fn parse_command(words: &[String]) -> Result<ServiceCommand, String> {
    let (action, rest) = words
        .split_first()
        .ok_or("say what to do: repomemo-server service install | uninstall | run")?;
    let mut options = ServiceOptions {
        name: DEFAULT_NAME.to_owned(),
        workdir: None,
        env_file: None,
        account: DEFAULT_ACCOUNT.to_owned(),
    };
    let mut rest = rest.iter();
    while let Some(flag) = rest.next() {
        let mut value = |what: &str| rest.next().cloned().ok_or(format!("{flag} needs {what}"));
        match flag.as_str() {
            "--name" => options.name = value("a service name")?,
            "--workdir" => options.workdir = Some(PathBuf::from(value("a folder")?)),
            "--env-file" => options.env_file = Some(PathBuf::from(value("a file")?)),
            "--account" => options.account = value("an account")?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if options.name.is_empty()
        || !options
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("--name may hold only letters, digits, underscores and hyphens".to_owned());
    }
    match action.as_str() {
        "install" => Ok(ServiceCommand::Install(options)),
        "uninstall" => Ok(ServiceCommand::Uninstall(options)),
        "run" => Ok(ServiceCommand::Run(options)),
        other => Err(format!("unknown service action: {other} (install, uninstall or run)")),
    }
}

/// The settings of an env file: `NAME=value` lines; blank lines and `#` comments are skipped; the value is the rest of the
/// line (spaces inside are kept, a pair of quotes around it is dropped).
pub(crate) fn parse_env_file(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start_matches('\u{feff}').trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            let value = value.trim();
            let unquoted = value
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|inner| inner.strip_suffix('\'')))
                .unwrap_or(value);
            Some((name.to_owned(), unquoted.to_owned()))
        })
        .collect()
}

/// Handles `service ...` and returns the exit code.
pub(crate) fn run(words: &[String]) -> i32 {
    match parse_command(words) {
        Ok(command) => platform::execute(command),
        Err(message) => {
            eprintln!("service: {message}");
            2
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::ServiceCommand;

    pub(super) fn execute(_command: ServiceCommand) -> i32 {
        eprintln!(
            "service: Windows services are for Windows. On Linux use systemd (service/install.sh in the archive)."
        );
        2
    }
}

#[cfg(windows)]
mod platform {
    use std::{ffi::OsString, sync::Mutex, time::Duration};

    use windows_service::{
        define_windows_service,
        service::{
            ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceInfo,
            ServiceStartType, ServiceState, ServiceStatus, ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    use super::{parse_env_file, ServiceCommand, ServiceOptions};

    const DISPLAY_NAME: &str = "RepoMemo server";
    const DESCRIPTION: &str = "RepoMemo: the team's technical memory. https://github.com/repomemo/repomemo";

    /// What the service entry point needs, handed over from `run` (the dispatcher takes no arguments of ours).
    static RUNNING: Mutex<Option<ServiceOptions>> = Mutex::new(None);

    pub(super) fn execute(command: ServiceCommand) -> i32 {
        let result = match command {
            ServiceCommand::Install(options) => install(&options),
            ServiceCommand::Uninstall(options) => uninstall(&options),
            ServiceCommand::Run(options) => dispatch(options),
        };
        match result {
            Ok(()) => 0,
            Err(message) => {
                eprintln!("service: {message}");
                1
            }
        }
    }

    fn install(options: &ServiceOptions) -> Result<(), String> {
        let executable = std::env::current_exe().map_err(|error| format!("cannot find this program: {error}"))?;
        let mut arguments: Vec<OsString> = vec![
            "service".into(),
            "run".into(),
            "--name".into(),
            options.name.clone().into(),
        ];
        if let Some(workdir) = &options.workdir {
            arguments.extend(["--workdir".into(), workdir.clone().into_os_string()]);
        }
        if let Some(env_file) = &options.env_file {
            arguments.extend(["--env-file".into(), env_file.clone().into_os_string()]);
        }
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)
            .map_err(|error| format!("cannot open the service manager (run as an administrator): {error}"))?;
        let info = ServiceInfo {
            name: options.name.clone().into(),
            display_name: DISPLAY_NAME.into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: executable,
            launch_arguments: arguments,
            dependencies: vec![],
            account_name: Some(options.account.clone().into()),
            account_password: None,
        };
        let service = manager
            .create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::QUERY_STATUS)
            .map_err(|error| {
                format!(
                    "cannot install the service {} ({error}); if it exists, run uninstall first",
                    options.name
                )
            })?;
        service
            .set_description(DESCRIPTION)
            .map_err(|error| format!("the service is installed but its description could not be set: {error}"))?;
        // Restart after a failure: after 5 seconds twice, then after 30; the count starts again after a day.
        let _ = std::process::Command::new("sc.exe")
            .args([
                "failure",
                &options.name,
                "reset=",
                "86400",
                "actions=",
                "restart/5000/restart/5000/restart/30000",
            ])
            .output();
        println!(
            "service: {} is installed; it starts when Windows does. Start it now with: Start-Service {}",
            options.name, options.name
        );
        Ok(())
    }

    fn uninstall(options: &ServiceOptions) -> Result<(), String> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|error| format!("cannot open the service manager (run as an administrator): {error}"))?;
        let service = manager
            .open_service(
                &options.name,
                ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
            )
            .map_err(|error| format!("the service {} is not installed ({error})", options.name))?;
        if service.query_status().map_err(|error| error.to_string())?.current_state != ServiceState::Stopped {
            let _ = service.stop();
            for _ in 0..60 {
                if service
                    .query_status()
                    .map(|status| status.current_state == ServiceState::Stopped)
                    .unwrap_or(true)
                {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        service
            .delete()
            .map_err(|error| format!("cannot remove the service: {error}"))?;
        println!("service: {} is removed (its data was not touched)", options.name);
        Ok(())
    }

    fn dispatch(options: ServiceOptions) -> Result<(), String> {
        let name = options.name.clone();
        *RUNNING.lock().map_err(|_| "internal error".to_owned())? = Some(options);
        service_dispatcher::start(name, ffi_service_main)
            .map_err(|error| format!("this command is for the Windows service manager to start ({error}); to run the server in a window, start repomemo-server with no arguments"))
    }

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_arguments: Vec<OsString>) {
        let Some(options) = RUNNING.lock().ok().and_then(|mut slot| slot.take()) else {
            return;
        };
        let _ = run_service(&options);
    }

    fn run_service(options: &ServiceOptions) -> windows_service::Result<()> {
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let stop = Mutex::new(Some(stop));
        let status_handle = service_control_handler::register(options.name.clone(), move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                if let Some(stop) = stop.lock().ok().and_then(|mut slot| slot.take()) {
                    let _ = stop.send(());
                }
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;
        let report = |state: ServiceState, accepted: ServiceControlAccept, code: u32, wait: Duration| {
            status_handle.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: accepted,
                exit_code: ServiceExitCode::Win32(code),
                checkpoint: 0,
                wait_hint: wait,
                process_id: None,
            })
        };
        report(
            ServiceState::StartPending,
            ServiceControlAccept::empty(),
            0,
            Duration::from_secs(20),
        )?;

        // The manager starts a service in System32 with an empty environment: go to the working folder and read the settings.
        if let Some(workdir) = &options.workdir {
            let _ = std::env::set_current_dir(workdir);
        }
        if let Some(file) = &options.env_file {
            match std::fs::read_to_string(file) {
                Ok(text) => {
                    for (name, value) in parse_env_file(&text) {
                        std::env::set_var(name, value);
                    }
                }
                Err(error) => {
                    eprintln!("service: cannot read {}: {error}", file.display());
                    report(
                        ServiceState::Stopped,
                        ServiceControlAccept::empty(),
                        1,
                        Duration::default(),
                    )?;
                    return Ok(());
                }
            }
        }

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(_) => {
                report(
                    ServiceState::Stopped,
                    ServiceControlAccept::empty(),
                    1,
                    Duration::default(),
                )?;
                return Ok(());
            }
        };
        report(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            0,
            Duration::default(),
        )?;
        let code = runtime.block_on(crate::runner::run_server(async {
            let _ = stopped.await;
        }));
        report(
            ServiceState::Stopped,
            ServiceControlAccept::empty(),
            u32::from(code != 0),
            Duration::default(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_command_line_names_an_action_and_its_options() {
        let ServiceCommand::Install(options) = parse_command(&words(
            "install --workdir C:\\RepoMemo --env-file C:\\RepoMemo\\repomemo.env",
        ))
        .unwrap() else {
            panic!("install expected");
        };
        assert_eq!(options.name, "RepoMemo");
        assert_eq!(options.workdir.as_deref(), Some(std::path::Path::new("C:\\RepoMemo")));
        assert_eq!(options.account, "NT AUTHORITY\\LocalService");
        assert!(matches!(
            parse_command(&words("uninstall")).unwrap(),
            ServiceCommand::Uninstall(_)
        ));
        assert!(
            matches!(parse_command(&words("run --name Memo2")).unwrap(), ServiceCommand::Run(options) if options.name == "Memo2")
        );
    }

    #[test]
    fn a_bad_command_line_says_what_is_wrong() {
        assert!(parse_command(&[]).unwrap_err().contains("say what to do"));
        assert!(parse_command(&words("restart"))
            .unwrap_err()
            .contains("unknown service action"));
        assert!(parse_command(&words("install --bogus"))
            .unwrap_err()
            .contains("unknown argument"));
        assert!(parse_command(&words("install --workdir"))
            .unwrap_err()
            .contains("needs a folder"));
        assert!(parse_command(&words("install --name a;b"))
            .unwrap_err()
            .contains("letters, digits"));
    }

    #[test]
    fn an_env_file_gives_names_and_values() {
        let pairs = parse_env_file(
            "\u{feff}# the settings\nREPOMEMO_SERVER_ADDR=127.0.0.1:3020\n\nREPOMEMO_WEB_DIR=C:\\Program Files\\RepoMemo\\current\\web\nREPOMEMO_PRODUCT_NAME=\"Acme Memo\"\nnot a setting\nBAD NAME=x\nEMPTY=\n",
        );
        assert_eq!(
            pairs,
            vec![
                ("REPOMEMO_SERVER_ADDR".to_owned(), "127.0.0.1:3020".to_owned()),
                (
                    "REPOMEMO_WEB_DIR".to_owned(),
                    "C:\\Program Files\\RepoMemo\\current\\web".to_owned()
                ),
                ("REPOMEMO_PRODUCT_NAME".to_owned(), "Acme Memo".to_owned()),
                ("EMPTY".to_owned(), String::new()),
            ]
        );
    }
}
