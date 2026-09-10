// Flick - main.rs
// Thin wrapper per PRD §7.2.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if flick_lib::transcription_worker::run_if_requested() {
        return;
    }
    flick_lib::run();
}
