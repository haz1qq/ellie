#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = ellie_lib::run() {
        // Errors are typed and deliberately exclude paths, input values and source errors.
        eprintln!("Ellie could not start: {error}");
        std::process::exit(1);
    }
}
