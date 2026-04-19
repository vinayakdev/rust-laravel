fn main() {
    if let Err(error) = rust_php::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
