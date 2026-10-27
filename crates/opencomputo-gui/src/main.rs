//! Avvio dell'applicazione desktop. Un argomento facoltativo è il file di progetto da
//! aprire.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    opencomputo_gui::run();
}
