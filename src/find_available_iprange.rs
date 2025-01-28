use std::collections::HashSet;
use std::fs;
use std::net::Ipv4Addr;


/// Parse /proc/net/tcp to retrieve a set of active IPv4 addresses.
fn get_active_networks() -> HashSet<Ipv4Addr> {
    let mut active_nets: HashSet<Ipv4Addr> = HashSet::new();

    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        for line in content.lines().skip(1) { // Skip the header line
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                // Parse the local address (e.g., 0100007F:0016)
                if let Some(local_address) = parts.get(1) {
                    if let Some(ip) = ip_str_to_net(local_address) {
                        active_nets.insert(ip);
                    }
                }
            }
        }
    }
    active_nets
}

/// Parse a hexadecimal IP address from /proc/net/tcp (e.g., "0100007F:0016").
fn ip_str_to_net(hex_ip: &str) -> Option<Ipv4Addr> {
    let ip_port: Vec<&str> = hex_ip.split(':').collect();
    if ip_port.len() == 2 {
        if let Ok(ip) = u32::from_str_radix(ip_port[0], 16) {
            return Some(Ipv4Addr::new(
                (ip & 0xFF) as u8,
                ((ip >> 8) & 0xFF) as u8,
                ((ip >> 16) & 0xFF) as u8,
                0,
            ));
        }
    }
    None
}

/// Find free IP ranges of 255 addresses each, starting from 127.0.1.0 to 127.255.255.255.
fn find_available_iprange() -> Result<Ipv4Addr, String> {
    let active_nets = get_active_networks();

    for i in 0..=255 {
        for j in 0..=255 {
            if j == 0 && i == 0 {
                continue;
            }
            let net = Ipv4Addr::new(127, i, j, 0);
            if !active_nets.contains(&net) {
                return Ok(net)
            }
        }
    }
    Err("No free IP ranges found".to_string())
}


#[cfg(test)]
mod tests {
    use crate::find_available_iprange::find_available_iprange;

    #[test]
    fn test_find_available_range() {
        println!("{:?}", find_available_iprange())
    }
}
