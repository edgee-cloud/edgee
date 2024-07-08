use std::{net::SocketAddr, str::FromStr};

use http::HeaderMap;
use ipnetwork::IpNetwork;

pub fn get(remote_addr: SocketAddr, request_headers: &HeaderMap) -> String {
    let cidr_blocks = vec![
        "127.0.0.1/8",    // localhost
        "10.0.0.0/8",     // 24-bit block
        "172.16.0.0/12",  // 20-bit block
        "192.168.0.0/16", // 16-bit block
        "169.254.0.0/16", // link local address
        "::1/128",        // localhost IPv6
        "fc00::/7",       // unique local address IPv6
        "fe80::/10",      // link local address IPv6
    ];

    let cidrs = cidr_blocks
        .iter()
        .filter_map(|&cidr| IpNetwork::from_str(cidr).ok())
        .collect();

    let headers = [
        "X-Client-IP",
        "CF-Connecting-IP",
        "Fastly-Client-Ip",
        "True-Client-Ip",
        "X-Real-IP",
    ];

    if let Some(ip) = headers.iter().find_map(|&header| {
        request_headers
            .get(header)
            .and_then(|h| h.to_str().ok())
            .map(|value| value.to_string())
    }) {
        return ip;
    }

    let headers = [
        "X-Forwarded-For",
        "X-Original-Forwarded-For",
        "Forwarded-For",
        "X-Forwarded",
        "Forwarded",
    ];

    for header in headers {
        if let Some(ip) = request_headers
            .get(header)
            .and_then(|h| h.to_str().ok())
            .and_then(|value| {
                let mut public_ip: Option<String> = None;
                for ip in value.split(',').map(str::trim) {
                    if !is_private_address(&cidrs, ip) {
                        public_ip = Some(ip.to_string());
                        break;
                    }
                }
                public_ip
            })
        {
            return ip;
        }
    }

    // if no special headers, use the fastly client ip as the real ip
    return remote_addr.to_string();
}

fn is_private_address(cidrs: &Vec<IpNetwork>, address: &str) -> bool {
    if let Some(ip_address) = address.parse().ok() {
        for cidr in cidrs {
            if cidr.contains(ip_address) {
                return true;
            }
        }
    }

    return false;
}
