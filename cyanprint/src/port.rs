use std::collections::HashSet;
use std::net::TcpListener;

use rand::Rng;

/// Port range for `cyanprint try` command template containers.
pub const TEMPLATE_TRY: u16 = 49152;
/// End of port range for `cyanprint try` command template containers.
pub const TEMPLATE_TRY_END: u16 = 49351;

/// Port range for `cyanprint test template` containers.
pub const TEMPLATE_TEST: u16 = 49352;
/// End of port range for `cyanprint test template` containers.
pub const TEMPLATE_TEST_END: u16 = 49551;

/// Port range for processor test containers.
pub const PROCESSOR_TEST: u16 = 49552;
/// End of port range for processor test containers.
pub const PROCESSOR_TEST_END: u16 = 49751;

/// Port range for plugin test containers.
pub const PLUGIN_TEST: u16 = 49752;
/// End of port range for plugin test containers.
pub const PLUGIN_TEST_END: u16 = 49951;

/// Port range for resolver test containers.
pub const RESOLVER_TEST: u16 = 49952;
/// End of port range for resolver test containers.
pub const RESOLVER_TEST_END: u16 = 50151;

/// Represents an allocated port held open by a `TcpListener`.
///
/// The port is reserved as long as this struct exists. Call [`release`](Self::release)
/// to free the port by dropping the listener and returning the port number.
pub struct PortAllocation {
    /// The allocated port number.
    pub port: u16,
    _listener: TcpListener,
}

impl PortAllocation {
    /// Releases the port allocation, dropping the listener and returning the port number.
    ///
    /// The caller should use the returned port immediately (e.g., for a Docker bind)
    /// to minimize the race window.
    pub fn release(self) -> u16 {
        self.port
    }
}

/// Allocates an available port in the given range by binding a `TcpListener`.
///
/// Uses random selection (up to 50 attempts) with a sequential fallback to avoid
/// starvation. Returns `None` if no port is available in the range.
///
/// # Arguments
///
/// * `range_start` - Start of the port range (inclusive)
/// * `range_end` - End of the port range (inclusive)
///
/// # Examples
///
/// ```
/// let alloc = allocate_port(49352, 49551);
/// assert!(alloc.is_some());
/// ```
pub fn allocate_port(range_start: u16, range_end: u16) -> Option<PortAllocation> {
    if range_start == 0 || range_start > range_end {
        return None;
    }

    let range_size = usize::from(range_end) - usize::from(range_start) + 1;
    let mut tried: HashSet<u16> = HashSet::new();
    let mut rng = rand::rng();

    // Random phase: try up to min(range_size, 50) random ports
    let random_attempts = range_size.min(50);
    for _ in 0..random_attempts {
        let port = rng.random_range(range_start..=range_end);
        if tried.insert(port) {
            if let Some(alloc) = try_bind_port(port) {
                return Some(alloc);
            }
        }
    }

    // Sequential fallback: iterate the full range
    for port in range_start..=range_end {
        if !tried.contains(&port) {
            if let Some(alloc) = try_bind_port(port) {
                return Some(alloc);
            }
        }
    }

    None
}

/// Attempts to bind a `TcpListener` on the given port.
fn try_bind_port(port: u16) -> Option<PortAllocation> {
    TcpListener::bind(("0.0.0.0", port))
        .ok()
        .map(|_listener| PortAllocation { port, _listener })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_port_in_range() {
        let alloc = allocate_port(TEMPLATE_TEST, TEMPLATE_TEST_END);
        assert!(alloc.is_some());
        if let Some(a) = alloc {
            assert!((TEMPLATE_TEST..=TEMPLATE_TEST_END).contains(&a.port));
            // Port is still held
            assert!(TcpListener::bind(("0.0.0.0", a.port)).is_err());
        }
    }

    #[test]
    fn test_release_frees_port() {
        // Use a wide range with a PID-based offset to minimize collisions between
        // parallel test processes. A 100-port range makes it practically impossible
        // for all ports to be occupied after release.
        let pid = std::process::id() as u16;
        let base: u16 = 42000 + (pid % 3000);
        let range_end = base + 99;

        // First allocation should succeed (many ports available in 100-port range)
        let alloc1 = allocate_port(base, range_end)
            .expect("First allocation should succeed in a 100-port range");

        // Verify the port is held while PortAllocation exists
        let port = alloc1.port;
        assert!(
            TcpListener::bind(("0.0.0.0", port)).is_err(),
            "Port should be held while PortAllocation exists"
        );

        // Release the port — consumes self, drops listener
        let freed = alloc1.release();
        assert_eq!(
            freed, port,
            "release() should return the allocated port number"
        );

        // After release, the port is free. Even if another process grabbed it,
        // there are 99 other ports in the range, so a second allocation must succeed.
        let alloc2 = allocate_port(base, range_end);
        assert!(
            alloc2.is_some(),
            "Second allocation should succeed after release — the released port or another should be available"
        );
    }

    #[test]
    fn test_allocate_port_empty_range() {
        let alloc = allocate_port(50152, 49152);
        assert!(alloc.is_none());
    }

    #[test]
    fn test_allocate_port_random_then_sequential_fallback() {
        // Bind all ports in a small range except one to force sequential fallback.
        // Use a PID-based offset to minimize collisions between parallel test processes.
        let pid = std::process::id() as u16;
        let base: u16 = 42000 + (pid % 3000);
        let start: u16 = base;
        let end: u16 = base + 9;
        let skip_port: u16 = base + 5; // leave this port free
        let mut listeners = Vec::new();

        for port in start..=end {
            if port == skip_port {
                continue; // skip the one we want to be free
            }
            if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
                listeners.push(listener);
            }
            // If bind fails, the port was already in use — that's fine for our test
        }

        let alloc = allocate_port(start, end);
        assert!(
            alloc.is_some(),
            "Should find the free port {skip_port} in range {start}-{end}"
        );
        assert_eq!(alloc.unwrap().port, skip_port);

        // Clean up listeners
        drop(listeners);
    }
}
