fn main() {
    if let Err(err) = dlp::run() {
        eprintln!("\n❌ Error: {err}\n");
        std::process::exit(1);
    }
}
