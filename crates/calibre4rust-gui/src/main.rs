fn main() -> anyhow::Result<()> {
    let arguments: Vec<String> = std::env::args().collect();
    if calibre4rust_gui::cli::run(&arguments)? {
        return Ok(());
    }
    calibre4rust_gui::app::run()
}
