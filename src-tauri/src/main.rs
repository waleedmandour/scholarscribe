// ScholarScribe — binary entry point. Defers to the library.
// This split lets `tauri-build` generate mobile targets (future) cleanly.

fn main() {
    scholarscribe_lib::run();
}
