fn main() {
    let path = std::env::args().nth(1).unwrap();
    let no_check = std::env::args().any(|a| a == "--no-check");
    let src = std::fs::read_to_string(path).unwrap();
    if no_check {
        let lexed = valafmt::lexer::lex(&src);
        let file = valafmt::parser::parse(lexed.tokens).unwrap();
        let doc = valafmt::format::format_file(&file);
        let out = valafmt::doc::printer::print(&doc, &valafmt::Options::default());
        print!("{out}");
        return;
    }
    match valafmt::format_source(&src, &valafmt::Options::default()) {
        Ok(out) => print!("{out}"),
        Err(e) => eprintln!("ERROR:\n{e}"),
    }
}
