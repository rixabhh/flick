// Flick - main.rs
// Thin wrapper per PRD §7.2.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    flick_lib::run();
}
