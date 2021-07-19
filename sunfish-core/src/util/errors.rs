use log::error;
use std::ops::Deref;
use std::panic;

// https://stackoverflow.com/a/42457596
pub fn setup_panic_handling() {
    log::info!("Setting up panic handler");
    panic::set_hook(Box::new(|panic_info| {
        log::info!("A panic occurred!");
        let (filename, line) = panic_info
            .location()
            .map(|loc| (loc.file(), loc.line()))
            .unwrap_or(("<unknown>", 0));

        let cause = panic_info
            .payload()
            .downcast_ref::<String>()
            .map(String::deref);

        let cause = cause.unwrap_or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| *s)
                .unwrap_or("<cause unknown>")
        });

        error!("A panic occurred at {}:{}: {}", filename, line, cause);
    }));
}
