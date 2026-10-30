use tracing::info;

/// Print minimalist ASCII banner and runtime metadata to the console on boot.
pub fn print_banner(version: &str, port: u16, db_path: &str) {
    let green = "\x1b[38;2;48;209;88m";
    let blue = "\x1b[38;2;10;132;255m";
    let gray = "\x1b[38;2;140;140;150m";
    let white = "\x1b[38;2;245;245;250m";
    let bold = "\x1b[1m";
    let reset = "\x1b[0m";

    println!(
        r#"{green}
  ██╗  ██╗███████╗███████╗██████╗ ██╗   ██╗██████╗ 
  ██║ ██╔╝██╔════╝██╔════╝██╔══██╗██║   ██║██╔══██╗
  █████╔╝ █████╗  █████╗  ██████╔╝██║   ██║██████╔╝
  ██╔═██╗ ██╔══╝  ██╔══╝  ██╔═══╝ ██║   ██║██╔═══╝ 
  ██║  ██╗███████╗███████╗██║     ╚██████╔╝██║     
  ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝      ╚═════╝ ╚═╝     {reset}
  {gray}Ultra-lightweight Uptime & Telemetry Engine{reset}

  {blue}Version  :{reset} {bold}{white}v{version}{reset}
  {blue}Service  :{reset} {white}http://localhost:{port}{reset}
  {blue}Database :{reset} {white}{db_path}{reset}
  {blue}Engine   :{reset} {white}Tokio Async + Axum + SQLite (WAL Mode){reset}
"#
    );

    info!("KeepUp engine initialized successfully.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_banner_does_not_panic() {
        print_banner("0.1.0", 3000, "data/keepup.db");
    }
}
