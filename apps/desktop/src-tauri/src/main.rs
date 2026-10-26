// No console window behind the app in a release build (V0.4.50); a debug build keeps it for the logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    repomemo_desktop_lib::run();
}
