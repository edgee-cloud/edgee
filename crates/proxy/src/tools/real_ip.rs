use http::HeaderMap;
use ipnetwork::IpNetwork;
use std::net::SocketAddr;
use std::str::FromStr;

pub struct Realip {
    cidrs: Vec<IpNetwork>,
}

impl Realip {
    /// Constructs a new `Realip` instance.
    ///
    /// # Returns
    ///
    /// * `Self` - A new `Realip` instance with its `cidrs` field populated with CIDR blocks representing private IP address ranges.
    ///
    /// # Logic
    ///
    /// The function first defines a vector `cidr_blocks` containing strings representing CIDR blocks for private IP address ranges.
    /// These ranges include localhost, 24-bit block, 20-bit block, 16-bit block, link local address, localhost IPv6, unique local address IPv6, and link local address IPv6.
    /// The function then iterates over `cidr_blocks`, attempts to parse each CIDR block string into an `IpNetwork` instance, and collects the successful results into a new vector `cidrs`.
    /// Finally, the function constructs a new `Realip` instance with `cidrs` as its `cidrs` field and returns it.
    pub fn new() -> Self {
        let cidr_blocks = [
            "127.0.0.1/8",    // localhost
            "10.0.0.0/8",     // 24-bit block
            "172.16.0.0/12",  // 20-bit block
            "192.168.0.0/16", // 16-bit block
            "169.254.0.0/16", // link local address
            "::1/128",        // localhost IPv6
            "fc00::/7",       // unique local address IPv6
            "fe80::/10",
        ];

        let cidrs = cidr_blocks
            .iter()
            .filter_map(|&cidr| IpNetwork::from_str(cidr).ok())
            .collect();

        Self { cidrs }
    }

    /// Retrieves the client's real IP address from the request headers or falls back to the remote address.
    ///
    /// # Arguments
    ///
    /// * `remote_addr` - A `SocketAddr` representing the remote address of the client.
    /// * `request_headers` - A reference to the request headers.
    ///
    /// # Returns
    ///
    /// * `String` - The client's real IP address as a string. If no special or forwarded headers are found, the remote address is returned.
    pub fn get_from_request(
        &self,
        remote_addr: &SocketAddr,
        request_headers: &HeaderMap,
    ) -> String {
        if let Some(ip) = self.get_from_special_headers(request_headers) {
            return ip;
        }

        if let Some(ip) = self.get_from_forwarded_headers(request_headers) {
            return ip;
        }

        // if no special headers, use the remote_addr, but remove the port
        remote_addr.ip().to_string()
    }

    /// Checks if the given IP address is a private address.
    ///
    /// # Arguments
    ///
    /// * `address` - A string slice that holds the IP address to be checked.
    ///
    /// # Returns
    ///
    /// * `Result<bool, &'static str>` - A `Result` containing `true` if the IP address is private, `false` if it is not, or an error message if the IP address is not valid.
    ///
    /// # Logic
    ///
    /// The function attempts to parse the given IP address string into an `IpAddr` instance.
    /// If parsing fails, it returns an error message indicating that the address is not valid.
    /// It then iterates over the CIDR blocks stored in the `cidrs` field of the `Realip` struct.
    /// If the IP address is contained within any of the CIDR blocks, the function returns `true`.
    /// If the IP address is not contained within any of the CIDR blocks, the function returns `false`.
    fn is_private_address(&self, address: &str) -> Result<bool, &'static str> {
        let ip_address = address.parse().map_err(|_| "address is not valid")?;

        for cidr in &self.cidrs {
            if cidr.contains(ip_address) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Retrieves the client's real IP address from special headers in the request.
    ///
    /// # Arguments
    ///
    /// * `request_headers` - A reference to the request headers.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - An `Option` containing the client's real IP address as a string if found in the special headers, or `None` if not found.
    ///
    /// # Logic
    ///
    /// The function defines a list of headers that may contain the client's real IP address.
    /// It iterates over these headers and checks if any of them are present in the request headers.
    /// If a header is found, the function attempts to convert its value to a string and returns it.
    fn get_from_special_headers(&self, request_headers: &HeaderMap) -> Option<String> {
        let headers = [
            "X-Client-IP",
            "CF-Connecting-IP",
            "Fastly-Client-Ip",
            "True-Client-Ip",
            "X-Real-IP",
        ];

        headers.iter().find_map(|&header| {
            request_headers
                .get(header)
                .and_then(|h| h.to_str().ok())
                .map(|value| value.to_string())
                .or(None)
        })
    }

    /// Retrieves the client's real IP address from forwarded headers in the request.
    ///
    /// # Arguments
    ///
    /// * `request_headers` - A reference to the request headers.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - An `Option` containing the client's real IP address as a string if found in the forwarded headers, or `None` if not found.
    ///
    /// # Logic
    ///
    /// The function defines a list of headers that may contain the client's real IP address.
    /// It iterates over these headers and checks if any of them are present in the request headers.
    /// If a header is found, the function splits its value by commas to handle multiple IP addresses and trims each IP address.
    /// It then checks if each IP address is a private address by calling the `is_private_address` method.
    /// If an IP address is not private, it is returned as the client's real IP address.
    fn get_from_forwarded_headers(&self, request_headers: &HeaderMap) -> Option<String> {
        let headers = [
            "X-Forwarded-For",
            "X-Original-Forwarded-For",
            "Forwarded-For",
            "X-Forwarded",
            "Forwarded",
        ];

        for &header in headers.iter() {
            if let Some(value) = request_headers.get(header).and_then(|h| h.to_str().ok()) {
                for ip in value.split(',').map(str::trim) {
                    if let Ok(is_private) = self.is_private_address(ip) {
                        if !is_private {
                            return Some(ip.to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;
    use pretty_assertions::assert_eq;
    use std::net::SocketAddr;

    #[test]
    fn new_realip_initializes_with_private_cidrs() {
        let realip = Realip::new();
        assert_eq!(realip.cidrs.len(), 8);
    }

    #[test]
    fn get_from_request_returns_ip_from_special_headers() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", "203.0.113.195".parse().unwrap());
        let remote_addr: SocketAddr = "192.0.2.1:12345".parse().unwrap();
        let ip = realip.get_from_request(&remote_addr, &headers);
        assert_eq!(ip, "203.0.113.195");
    }

    #[test]
    fn get_from_request_returns_ip_from_forwarded_headers() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "203.0.113.195".parse().unwrap());
        let remote_addr: SocketAddr = "192.0.2.1:12345".parse().unwrap();
        let ip = realip.get_from_request(&remote_addr, &headers);
        assert_eq!(ip, "203.0.113.195");
    }

    #[test]
    fn get_from_request_returns_remote_addr_if_no_headers() {
        let realip = Realip::new();
        let headers = HeaderMap::new();
        let remote_addr: SocketAddr = "192.0.2.1:12345".parse().unwrap();
        let ip = realip.get_from_request(&remote_addr, &headers);
        assert_eq!(ip, "192.0.2.1");
    }

    #[test]
    fn is_private_address_returns_true_for_private_ip() {
        let realip = Realip::new();
        let result = realip.is_private_address("192.168.1.1");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn is_private_address_returns_false_for_public_ip() {
        let realip = Realip::new();
        let result = realip.is_private_address("8.8.8.8");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn is_private_address_returns_error_for_invalid_ip() {
        let realip = Realip::new();
        let result = realip.is_private_address("invalid_ip");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "address is not valid");
    }

    #[test]
    fn get_from_special_headers_returns_ip_if_present() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", "203.0.113.195".parse().unwrap());
        let ip = realip.get_from_special_headers(&headers);
        assert_eq!(ip, Some("203.0.113.195".to_string()));
    }

    #[test]
    fn get_from_special_headers_returns_none_if_not_present() {
        let realip = Realip::new();
        let headers = HeaderMap::new();
        let ip = realip.get_from_special_headers(&headers);
        assert_eq!(ip, None);
    }

    #[test]
    fn get_from_forwarded_headers_returns_public_ip() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            "203.0.113.195, 192.168.1.1".parse().unwrap(),
        );
        let ip = realip.get_from_forwarded_headers(&headers);
        assert_eq!(ip, Some("203.0.113.195".to_string()));
    }

    #[test]
    fn get_from_forwarded_headers_skips_private_ip() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            "192.168.1.1, 203.0.113.195".parse().unwrap(),
        );
        let ip = realip.get_from_forwarded_headers(&headers);
        assert_eq!(ip, Some("203.0.113.195".to_string()));
    }

    #[test]
    fn get_from_forwarded_headers_doesnt_fail_with_invalid_ip() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "192.168, 203.0.113.195".parse().unwrap());
        let ip = realip.get_from_forwarded_headers(&headers);
        assert_eq!(ip, Some("203.0.113.195".to_string()));
    }

    #[test]
    fn get_from_forwarded_headers_returns_none_if_all_private() {
        let realip = Realip::new();
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "192.168.1.1, 10.0.0.1".parse().unwrap());
        let ip = realip.get_from_forwarded_headers(&headers);
        assert_eq!(ip, None);
    }
}
