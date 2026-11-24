mod cli;
mod daemon;
mod edition;
mod gui;
mod pages;

fn main() {
    let args = match cli::parser(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(m) => {
            eprintln!("{m}");
            std::process::exit(2);
        }
    };
    if args.en_ligne() {
        let code = cli::lancer(&args, &mut std::io::stdout().lock(), &mut std::io::stderr().lock());
        std::process::exit(code);
    }
    std::process::exit(gui::lancer(args.config, args.socket));
}
