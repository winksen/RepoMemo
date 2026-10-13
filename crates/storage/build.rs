// Migrations are embedded at compile time; rebuild when one is added or changed.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
