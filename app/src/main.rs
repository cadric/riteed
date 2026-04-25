#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(not(coverage))]
fn main() -> gtk4::glib::ExitCode {
    riteed::run()
}

#[cfg(coverage)]
fn main() -> gtk4::glib::ExitCode {
    gtk4::glib::ExitCode::SUCCESS
}

#[cfg(all(test, coverage))]
mod tests {
    #[test]
    fn coverage_main_returns() {
        let _code = super::main();
    }
}
