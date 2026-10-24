//! Document readings in a process of their own (roadmap P10, review R2-4).
//!
//! A crafted PDF or workbook can make a parser run for ever, use all the
//! memory there is, or overflow its stack, and an overflow or an
//! out-of-memory abort in *any* thread ends the whole server. So a reading
//! can run in a child process instead: the server starts itself with
//! [`READER_ARGUMENT`], sends the file on the child's standard input and reads
//! the answer from its standard output. The parent then holds all the power:
//!
//! * **Time.** The child is killed when the reading takes longer than the
//!   limit (`REPOMEMO_DOCUMENT_TIMEOUT_SECONDS`), instead of being left
//!   running. Nothing is abandoned.
//! * **Memory.** The child counts what it allocates ([`LimitedAllocator`]) and
//!   exits with [`MEMORY_EXIT_CODE`] when it passes its limit
//!   (`REPOMEMO_DOCUMENT_MEMORY_MB`), long before the machine does.
//! * **Crashes.** A panic, stack overflow or abort ends the child only; the
//!   server reports that the file may be damaged and goes on.
//! * **Nothing to steal.** The child starts with an empty environment (no
//!   keys, no tokens) and reads one file, writes one answer and exits.
//!
//! Without a configured reader process (tests, or `REPOMEMO_DOCUMENT_READER=
//! thread`) readings run on a thread as before, with the weaker guarantees of
//! [`crate::budget`].
//!
//! Wire format. Request: `RMRD`, a kind byte, a big-endian `u16` name length,
//! the file name, a big-endian `u64` body length, the body. Response: a
//! status byte (0 answer, 1 error message), a big-endian `u64` length, the
//! payload.

use std::alloc::{GlobalAlloc, Layout, System};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

/// The first argument that makes a program act as a reader process.
pub const READER_ARGUMENT: &str = "--document-reader";
/// The environment variable that tells a reader process how much memory it may use, in MiB.
const MEMORY_ENV: &str = "REPOMEMO_READER_MEMORY_MB";
/// What a reader process exits with when it passes its memory limit.
pub const MEMORY_EXIT_CODE: i32 = 87;
const MAGIC: &[u8; 4] = b"RMRD";
/// The largest answer the server accepts from a reader process.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(10);

/// Default memory a reading may use, in MiB.
pub const DEFAULT_MEMORY_LIMIT_MB: u64 = 1024;

/// What a request asks the reader process to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestKind {
    Text = 1,
    Preview = 2,
    Probe = 255,
}

/// How to start a reader process.
#[derive(Debug, Clone)]
pub struct ReaderProcess {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
}

impl ReaderProcess {
    /// This very program, started as a reader.
    pub fn this_program() -> std::io::Result<Self> {
        Ok(Self {
            program: std::env::current_exe()?,
            arguments: vec![OsString::from(READER_ARGUMENT)],
        })
    }
}

static PROCESS: Mutex<Option<ReaderProcess>> = Mutex::new(None);
static MEMORY_LIMIT_MB: AtomicU64 = AtomicU64::new(DEFAULT_MEMORY_LIMIT_MB);
static KILLED: AtomicU64 = AtomicU64::new(0);

/// Chooses where readings run: in a child process started like this, or
/// (`None`) on a thread of this process.
pub fn set_reader_process(process: Option<ReaderProcess>) {
    if let Ok(mut current) = PROCESS.lock() {
        *current = process;
    }
}

/// Sets the memory a reading may use, in MiB; 0 removes the limit.
pub fn set_memory_limit_mb(megabytes: u64) {
    MEMORY_LIMIT_MB.store(megabytes, Ordering::Relaxed);
}

/// Readings that were stopped for running over their time or memory.
pub fn killed_readings() -> u64 {
    KILLED.load(Ordering::Relaxed)
}

/// Whether readings run in a child process.
pub fn reads_in_a_process() -> bool {
    PROCESS.lock().is_ok_and(|process| process.is_some())
}

// ---------------------------------------------------------------------------
// The server's side

fn encode_request(kind: RequestKind, name: &str, body: &[u8]) -> Vec<u8> {
    let name = &name.as_bytes()[..name.len().min(u16::MAX as usize)];
    let mut request = Vec::with_capacity(body.len() + name.len() + 16);
    request.extend_from_slice(MAGIC);
    request.push(kind as u8);
    request.extend_from_slice(&(name.len() as u16).to_be_bytes());
    request.extend_from_slice(name);
    request.extend_from_slice(&(body.len() as u64).to_be_bytes());
    request.extend_from_slice(body);
    request
}

/// Runs one reading in a child process. `None` when readings run on a thread
/// instead. `what` names the reader in messages ("document reader").
pub(crate) fn read_in_process(kind: RequestKind, what: &str, name: &str, body: &[u8]) -> Option<Result<Vec<u8>>> {
    let process = PROCESS.lock().ok()?.clone()?;
    Some(run_child(&process, kind, what, name, body))
}

fn run_child(process: &ReaderProcess, kind: RequestKind, what: &str, name: &str, body: &[u8]) -> Result<Vec<u8>> {
    let seconds = crate::budget::time_limit_seconds();
    let megabytes = MEMORY_LIMIT_MB.load(Ordering::Relaxed);
    // The child sees nothing of this server's environment.
    let mut command = Command::new(&process.program);
    command
        .args(&process.arguments)
        .env_clear()
        .env(MEMORY_ENV, megabytes.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| anyhow!("The {what} could not be started: {error}"))?;

    // The file goes in while the answer comes out, so neither pipe can fill up and block.
    let request = encode_request(kind, name, body);
    let mut stdin = child.stdin.take().context("the reader has no input")?;
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&request);
        // Dropping stdin tells the child the file is complete.
    });
    let mut stdout = child.stdout.take().context("the reader has no output")?;
    let reader = std::thread::spawn(move || {
        let mut answer = Vec::new();
        let _ = (&mut stdout).take(MAX_RESPONSE_BYTES + 16).read_to_end(&mut answer);
        answer
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                let _ = reader.join();
                bail!("The {what} could not be watched: {error}");
            }
        }
        if seconds > 0 && started.elapsed() >= Duration::from_secs(seconds) {
            break None;
        }
        std::thread::sleep(POLL);
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = writer.join();
        let _ = reader.join();
        KILLED.fetch_add(1, Ordering::Relaxed);
        bail!("The {what} took longer than {seconds} seconds, so the file was skipped.");
    };
    let _ = writer.join();
    let answer = reader.join().unwrap_or_default();

    if status.code() == Some(MEMORY_EXIT_CODE) {
        KILLED.fetch_add(1, Ordering::Relaxed);
        bail!("The {what} needed more than {megabytes} MB of memory, so the file was skipped.");
    }
    if !status.success() {
        bail!("The {what} stopped unexpectedly. The file may be damaged.");
    }
    decode_response(what, &answer)
}

fn decode_response(what: &str, answer: &[u8]) -> Result<Vec<u8>> {
    let broken = || anyhow!("The {what} stopped unexpectedly. The file may be damaged.");
    let (status, rest) = answer.split_first().ok_or_else(broken)?;
    let length = u64::from_be_bytes(rest.get(..8).ok_or_else(broken)?.try_into().map_err(|_| broken())?);
    let payload = rest.get(8..).ok_or_else(broken)?;
    if length != payload.len() as u64 {
        return Err(broken());
    }
    match status {
        0 => Ok(payload.to_vec()),
        1 => Err(anyhow!("{}", String::from_utf8_lossy(payload))),
        _ => Err(broken()),
    }
}

// ---------------------------------------------------------------------------
// The reader process's side

struct Request {
    kind: u8,
    name: String,
    body: Vec<u8>,
}

fn read_request(input: &mut impl Read) -> Result<Request> {
    let mut header = [0u8; 7];
    input.read_exact(&mut header).context("no request")?;
    if &header[..4] != MAGIC {
        bail!("not a reading request");
    }
    let kind = header[4];
    let name_length = u16::from_be_bytes([header[5], header[6]]) as usize;
    let mut name = vec![0u8; name_length];
    input.read_exact(&mut name)?;
    let mut length = [0u8; 8];
    input.read_exact(&mut length)?;
    let length = u64::from_be_bytes(length);
    let mut body = Vec::new();
    input.take(length).read_to_end(&mut body)?;
    if body.len() as u64 != length {
        bail!("the request ends early");
    }
    Ok(Request {
        kind,
        name: String::from_utf8_lossy(&name).into_owned(),
        body,
    })
}

fn write_response(output: &mut impl Write, outcome: Result<Vec<u8>>) -> std::io::Result<()> {
    let (status, payload) = match outcome {
        Ok(payload) => (0u8, payload),
        Err(error) => (1u8, format!("{error:#}").into_bytes()),
    };
    output.write_all(&[status])?;
    output.write_all(&(payload.len() as u64).to_be_bytes())?;
    output.write_all(&payload)?;
    output.flush()
}

/// If this program was started as a reader process, does the one reading and
/// returns `true` after exiting; otherwise returns `false` at once. Called
/// first thing in `main`.
pub fn run_as_document_reader_if_requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some(READER_ARGUMENT) {
        return false;
    }
    serve_one_reading();
    true
}

/// The reader process: one request in, one answer out.
pub fn serve_one_reading() {
    if let Some(megabytes) = std::env::var(MEMORY_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        if megabytes > 0 {
            LimitedAllocator::limit_to(megabytes.saturating_mul(1024 * 1024));
        }
    }
    let outcome = read_request(&mut std::io::stdin().lock()).and_then(|request| {
        let name = std::path::Path::new(&request.name);
        match request.kind {
            1 => crate::documents::serve_text(name, &request.body),
            2 => crate::documents::serve_preview(name, &request.body),
            255 => serve_probe(&request.body),
            _ => bail!("unknown reading"),
        }
    });
    let mut stdout = std::io::stdout().lock();
    if write_response(&mut stdout, outcome).is_err() {
        std::process::exit(2);
    }
    drop(stdout);
    std::process::exit(0);
}

// ---------------------------------------------------------------------------
// Probes: ways a reading can go wrong, for the tests of the guards above.

/// A deliberate misbehaviour for a reader process, to prove the guards hold.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum Probe {
    Echo,
    /// Never finishes.
    Sleep,
    /// Allocates this many MiB and holds them.
    Allocate(u64),
    /// Panics.
    Panic,
    /// Aborts the process, as a stack overflow does.
    Abort,
    /// The names of the environment variables the process can see.
    Environment,
}

/// Runs a probe in the reader process; the answer is a text.
#[doc(hidden)]
pub fn probe_reader(probe: Probe) -> Result<String> {
    let body = match probe {
        Probe::Echo => vec![0],
        Probe::Sleep => vec![1],
        Probe::Allocate(megabytes) => {
            let mut body = vec![2];
            body.extend_from_slice(&megabytes.to_be_bytes());
            body
        }
        Probe::Panic => vec![3],
        Probe::Abort => vec![4],
        Probe::Environment => vec![5],
    };
    match read_in_process(RequestKind::Probe, "probe reader", "probe", &body) {
        Some(answer) => answer.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        None => bail!("no reader process is set up"),
    }
}

fn serve_probe(body: &[u8]) -> Result<Vec<u8>> {
    match body.first() {
        Some(0) => Ok(b"echo".to_vec()),
        Some(1) => loop {
            std::thread::sleep(Duration::from_secs(3600));
        },
        Some(2) => {
            let megabytes = u64::from_be_bytes(body.get(1..9).context("no size")?.try_into()?) as usize;
            let mut held: Vec<Vec<u8>> = Vec::new();
            for _ in 0..megabytes {
                // Touched, so the memory is really used.
                held.push(vec![1u8; 1024 * 1024]);
            }
            Ok(format!("allocated {}", held.len()).into_bytes())
        }
        Some(3) => panic!("probe panic"),
        Some(4) => std::process::abort(),
        Some(5) => {
            let mut names = std::env::vars().map(|(name, _)| name).collect::<Vec<_>>();
            names.sort();
            Ok(names.join(",").into_bytes())
        }
        _ => bail!("unknown probe"),
    }
}

// ---------------------------------------------------------------------------
// The memory limit

static COUNTING: AtomicBool = AtomicBool::new(false);
static LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);
/// Bytes handed out since counting began. Signed: blocks allocated before it began are freed after.
static USED: AtomicIsize = AtomicIsize::new(0);

/// A global allocator that does nothing different until a reader process asks
/// it to enforce a limit; then it counts what the program holds and ends the
/// process (exit code [`MEMORY_EXIT_CODE`]) when that passes the limit.
pub struct LimitedAllocator;

impl LimitedAllocator {
    /// Starts counting, and ends the process past `bytes`.
    pub fn limit_to(bytes: u64) {
        LIMIT.store(bytes.min(isize::MAX as u64) as usize, Ordering::Relaxed);
        COUNTING.store(true, Ordering::Release);
    }

    #[inline]
    fn add(size: usize) {
        if COUNTING.load(Ordering::Relaxed) {
            let used = USED.fetch_add(size as isize, Ordering::Relaxed) + size as isize;
            if used > 0 && used as usize > LIMIT.load(Ordering::Relaxed) {
                out_of_memory();
            }
        }
    }

    #[inline]
    fn remove(size: usize) {
        if COUNTING.load(Ordering::Relaxed) {
            USED.fetch_sub(size as isize, Ordering::Relaxed);
        }
    }
}

#[cold]
fn out_of_memory() -> ! {
    // Nothing here may allocate.
    let _ = std::io::stderr().write_all(b"document reader: memory limit reached\n");
    std::process::exit(MEMORY_EXIT_CODE)
}

// SAFETY: every call is passed straight to the system allocator; the counting
// around it only reads and writes atomics.
unsafe impl GlobalAlloc for LimitedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::add(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::add(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::remove(layout.size());
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > layout.size() {
            Self::add(new_size - layout.size());
        } else {
            Self::remove(layout.size() - new_size);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_survives_the_wire() {
        let bytes = encode_request(RequestKind::Text, "report.docx", b"file bytes");
        let request = read_request(&mut bytes.as_slice()).unwrap();
        assert_eq!(
            (request.kind, request.name.as_str(), request.body.as_slice()),
            (1, "report.docx", &b"file bytes"[..])
        );
        assert!(
            read_request(&mut &bytes[..bytes.len() - 3]).is_err(),
            "a cut request is refused"
        );
        assert!(read_request(&mut &b"XXXX\x01\x00\x00"[..]).is_err(), "not a request");
    }

    #[test]
    fn answers_and_errors_come_back_as_what_they_are() {
        let mut ok = Vec::new();
        write_response(&mut ok, Ok(b"text".to_vec())).unwrap();
        assert_eq!(decode_response("reader", &ok).unwrap(), b"text");
        let mut failed = Vec::new();
        write_response(&mut failed, Err(anyhow!("not a document"))).unwrap();
        assert_eq!(
            decode_response("reader", &failed).unwrap_err().to_string(),
            "not a document"
        );
        assert!(decode_response("reader", b"").is_err());
        assert!(
            decode_response("reader", &ok[..ok.len() - 1]).is_err(),
            "a short answer is a crash"
        );
        assert!(decode_response("reader", &[9, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
