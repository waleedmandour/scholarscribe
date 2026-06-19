// ScholarScribe — binary entry point. Defers to the library.
//
// The `windows_subsystem = "windows"` attribute MUST be on the binary crate
// (this file), not on the library crate. If it's on the lib, Windows still
// allocates a console window for the binary — which is exactly the bug we
// shipped in v0.1.0-pre (closing the console killed the app). Fixed in v0.1.1.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    scholarscribe_lib::run();
}
