use repomemo_server::{run_server, shutdown_signal};

// Counts memory only in a document reader process, which asks it to; the
// server itself pays for one flag check per allocation.
#[global_allocator]
static ALLOCATOR: repomemo_api::LimitedAllocator = repomemo_api::LimitedAllocator;

fn main() {
    // A document reading runs in a child process: this program, started with
    // an argument. It reads one file, answers and exits.
    if repomemo_api::run_as_document_reader_if_requested() {
        return;
    }
    // git runs this program as its credential helper when it fetches a remote
    // repository with a token: answer it and leave, without starting a server.
    if repomemo_git::remote::run_as_askpass_if_requested() {
        return;
    }
    // `--health` is the container's health check: ask this machine's server, exit 0 or 1.
    if std::env::args().any(|argument| argument == "--health") {
        std::process::exit(repomemo_server::health_probe());
    }
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("{}", repomemo_server::BUILD.line());
        return;
    }
    // `--check-config` reads the environment as a start would, reports it and exits.
    if std::env::args().any(|argument| argument == "--check-config") {
        std::process::exit(repomemo_server::check_config());
    }
    // `service install|uninstall|run` is the Windows service (V0.4.33).
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("service") {
        std::process::exit(repomemo_server::service_command(&arguments[1..]));
    }
    run();
}

#[tokio::main]
async fn run() {
    let code = run_server(shutdown_signal()).await;
    if code != 0 {
        std::process::exit(code);
    }
}
