use std::net::TcpListener;

/// Finds an available port in the specified range.
///
/// Scans ports from `range_start` to `range_end` (inclusive) and returns
/// the first available port. Returns `None` if no port is available in the range.
///
/// # Arguments
/// * `range_start` - Start of the port range (inclusive)
/// * `range_end` - End of the port range (inclusive)
///
/// # Examples
/// ```
/// let port = find_available_port(5600, 5900);
/// assert!(port.is_some());
/// ```
pub fn find_available_port(range_start: u16, range_end: u16) -> Option<u16> {
    (range_start..=range_end).find(|&port| is_port_available(port))
}

/// Checks if a specific port is available for binding.
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_port_in_range() {
        // Find any available port in a wide range
        let port = find_available_port(5600, 5900);
        assert!(port.is_some());
        if let Some(p) = port {
            assert!((5600..=5900).contains(&p));
        }
    }

    #[test]
    fn test_is_port_available_detects_used_port() {
        // Bind to an ephemeral port and verify it's detected as unavailable
        let listener = TcpListener::bind(("0.0.0.0", 0)).expect("Failed to bind ephemeral port");
        let port = listener.local_addr().unwrap().port();
        assert!(
            !is_port_available(port),
            "Port {port} should be detected as unavailable while listener is alive"
        );
    }

    #[test]
    fn test_find_available_port_returns_none_for_empty_range() {
        // An invalid range (start > end) should return None
        let port = find_available_port(5900, 5600);
        assert!(port.is_none());
    }

    #[test]
    fn test_find_available_port_single_port_range() {
        // Test with a single port range - port might be in use
        let port = find_available_port(0, 0);
        // Port 0 is special - OS will assign an available port
        assert!(port.is_some());
    }
}
