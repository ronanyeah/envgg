mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    envgg_core::run(Some(ui::open_secrets_viewer))
}
