#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(windows)]
mod updater;
#[cfg(windows)]
mod windows;

fn main() {
    #[cfg(windows)]
    if let Err(error) = windows::run() {
        windows::show_error(&error);
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    eprintln!("Pimpit requires Windows 10/11.");
}
