/// Print a human-facing progress line without polluting the headless JSON contract.
/// In headless mode the line goes to **stderr** so stdout stays reserved
/// exclusively for the final JSON envelope; outside headless mode it goes to stdout,
/// preserving the existing interactive output verbatim.
///
/// This is the `cyancoordinator` crate-local twin of `cyanprint`'s `hprogress!` — the
/// `hprogress!` macro lives in `cyanprint` and cannot cross the crate boundary, so each
/// crate that needs the behavior carries its own copy with identical semantics. Use this
/// for any progress/status message reachable from a headless command path in this crate.
/// Usage mirrors `println!`/`eprintln!`: `cprogress!(headless, "doing {x}")`.
#[macro_export]
macro_rules! cprogress {
    ($headless:expr, $($arg:tt)*) => {
        if $headless {
            eprintln!($($arg)*);
        } else {
            println!($($arg)*);
        }
    };
}
