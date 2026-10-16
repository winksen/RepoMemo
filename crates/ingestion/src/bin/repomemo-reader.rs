//! A reader process on its own, for the tests of the guards around it. The
//! server starts itself as a reader instead (`--document-reader`), so this is
//! never shipped.

use repomemo_ingestion::LimitedAllocator;

#[global_allocator]
static ALLOCATOR: LimitedAllocator = LimitedAllocator;

fn main() {
    repomemo_ingestion::serve_one_reading();
}
