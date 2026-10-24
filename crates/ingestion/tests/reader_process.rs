//! Readings in a process of their own: a reading that runs over its time or
//! its memory is killed, a crash costs only the reader, and the reader sees
//! none of the server's environment. The statics these tests set are
//! process-wide, so they run as one test.

use std::time::{Duration, Instant};

use repomemo_domain::DocumentPreview;
use repomemo_ingestion::{
    abandoned_readings, document_preview, extract_document_text, killed_readings, probe_reader, reads_in_a_process,
    set_document_memory_limit_mb, set_document_time_limit, set_reader_process, Probe, ReaderProcess,
};

const EMAIL: &[u8] = b"From: Ada <ada@example.com>\r\nTo: grace@example.com\r\nSubject: Quarterly plan\r\n\r\nShip the release on Friday.\r\n";

#[test]
fn readings_run_in_a_process_that_is_killed_when_it_overruns_and_that_crashes_alone() {
    set_reader_process(Some(ReaderProcess {
        program: env!("CARGO_BIN_EXE_repomemo-reader").into(),
        arguments: Vec::new(),
    }));
    set_document_time_limit(Duration::from_secs(20));
    set_document_memory_limit_mb(64);
    assert!(reads_in_a_process());

    // A real reading goes through the process and comes back whole.
    assert_eq!(probe_reader(Probe::Echo).unwrap(), "echo");
    let text = extract_document_text(std::path::Path::new("plan.eml"), EMAIL)
        .unwrap()
        .unwrap();
    assert!(
        text.contains("Quarterly plan") && text.contains("Ship the release on Friday"),
        "{text}"
    );
    assert!(matches!(
        document_preview(std::path::Path::new("plan.eml"), EMAIL),
        DocumentPreview::Email { .. }
    ));
    assert!(extract_document_text(std::path::Path::new("notes.txt"), b"plain")
        .unwrap()
        .is_none());
    // A file that is not what its name says is an error, not a crash.
    assert!(extract_document_text(std::path::Path::new("broken.pdf"), b"not a pdf").is_err());
    assert!(matches!(
        document_preview(std::path::Path::new("broken.pdf"), b"not a pdf"),
        DocumentPreview::Unavailable { .. }
    ));

    // Within its memory it finishes; past it the reader is stopped and says why.
    assert_eq!(probe_reader(Probe::Allocate(8)).unwrap(), "allocated 8");
    let before = killed_readings();
    let hungry = probe_reader(Probe::Allocate(400)).unwrap_err().to_string();
    assert!(hungry.contains("needed more than 64 MB of memory"), "{hungry}");
    assert_eq!(killed_readings(), before + 1);

    // Without a limit, the same reading is allowed.
    set_document_memory_limit_mb(0);
    assert_eq!(probe_reader(Probe::Allocate(100)).unwrap(), "allocated 100");
    set_document_memory_limit_mb(64);

    // A reading that never ends is killed, not abandoned.
    set_document_time_limit(Duration::from_secs(1));
    let started = Instant::now();
    let slow = probe_reader(Probe::Sleep).unwrap_err().to_string();
    assert!(slow.contains("took longer than 1 seconds"), "{slow}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the reader was killed at once"
    );
    assert_eq!(killed_readings(), before + 2);
    assert_eq!(abandoned_readings(), 0, "nothing is left running");
    set_document_time_limit(Duration::from_secs(20));

    // A panic and an abort (what a stack overflow does) end the reader only.
    for crash in [Probe::Panic, Probe::Abort] {
        let message = probe_reader(crash).unwrap_err().to_string();
        assert!(message.contains("stopped unexpectedly"), "{message}");
    }
    assert_eq!(
        probe_reader(Probe::Echo).unwrap(),
        "echo",
        "the next reading is unharmed"
    );

    // The reader has none of the server's secrets.
    std::env::set_var("REPOMEMO_SECRET_KEY", "a secret that must not reach the reader process");
    std::env::set_var("REPOMEMO_BACKUP_KEY", "another secret that stays here");
    let seen = probe_reader(Probe::Environment).unwrap();
    assert!(
        !seen.contains("REPOMEMO_SECRET_KEY") && !seen.contains("REPOMEMO_BACKUP_KEY"),
        "{seen}"
    );
    assert!(seen.contains("REPOMEMO_READER_MEMORY_MB"), "{seen}");

    // With no reader process, readings are on a thread again.
    set_reader_process(None);
    assert!(!reads_in_a_process());
    assert!(probe_reader(Probe::Echo).is_err());
    assert!(extract_document_text(std::path::Path::new("plan.eml"), EMAIL)
        .unwrap()
        .is_some());
}
