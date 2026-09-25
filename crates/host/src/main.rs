//! Bundled Windows SCM host.
#[cfg(any(windows, test))]
mod quoting;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
fn main() {
    if let Err(e) = windows::run() {
        eprintln!("service-kit-host: {e}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("service-kit-host is only supported on Windows SCM");
    std::process::exit(1);
}
