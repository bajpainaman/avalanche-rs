#[cfg(test)]
mod tests;

pub fn get_endpoint() -> (String, bool) {
    match std::env::var("AVALANCHEGO_CONFORMANCE_SERVER_RPC_ENDPOINT") {
        Ok(s) => (s, true),
        _ => (String::new(), false),
    }
}

/// Helper macro to skip test if conformance server endpoint is not configured.
/// Use at the start of each test: `skip_if_no_server!();`
#[macro_export]
macro_rules! skip_if_no_server {
    () => {
        let (ep, is_set) = $crate::get_endpoint();
        if !is_set {
            eprintln!("SKIPPING: AVALANCHEGO_CONFORMANCE_SERVER_RPC_ENDPOINT not set");
            return;
        }
        let _ = ep; // suppress unused warning
    };
}
