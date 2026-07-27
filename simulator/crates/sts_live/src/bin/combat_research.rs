fn main() {
    if let Err(error) = sts_live::combat_research::run(std::env::args().skip(1)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
