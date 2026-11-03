fn main() {
    match agentic_desktop_cli::run(std::env::args_os()) {
        Ok(envelope) => {
            match serde_json::to_string(&envelope) {
                Ok(text) => println!("{text}"),
                Err(error) => eprintln!("failed to encode output: {error}"),
            }
            std::process::exit(envelope.exit_code());
        }
        Err(error) => error.exit(),
    }
}
