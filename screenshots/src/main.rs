//! AudioRef as a standalone program, with nih-plug's host for standalone plugins: the window
//! without a DAW, for utils/screenshots.sh.

use std::process::ExitCode;

fn main() -> ExitCode {
    if nih_plug::nih_export_standalone::<audioref::AudioRef>() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
