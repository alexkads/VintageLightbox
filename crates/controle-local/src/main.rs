fn main() {
    std::process::exit(vlb_controle::cli::run(std::env::args().skip(1).collect()));
}
