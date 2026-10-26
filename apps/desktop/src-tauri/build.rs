fn main() {
    // The server an app is packaged for is read when it is compiled (V0.4.46).
    println!("cargo:rerun-if-env-changed=REPOMEMO_PRESET_SERVER");
    tauri_build::build()
}
