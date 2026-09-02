fn main() {
    let src = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    match valafmt::format_source(&src, &valafmt::Options::default()) {
        Ok(out) => print!("{out}"),
        Err(e) => eprintln!("ERROR:\n{e}"),
    }
}
