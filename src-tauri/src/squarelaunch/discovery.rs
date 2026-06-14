use std::io::{BufRead, BufReader, ErrorKind};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const SQUARELAUNCH_WS_SERVICE: &str = "_openlaunch-ws._tcp.local.";
const M_DNS_ADDR: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353);
const DNS_SD_BIN: &str = "/usr/bin/dns-sd";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSquareLaunchEndpoint {
    pub host: String,
    pub port: u16,
}

pub fn discover_squarelaunch_ws_endpoint(
    timeout: Duration,
) -> Result<DiscoveredSquareLaunchEndpoint, String> {
    let udp_timeout = timeout.min(Duration::from_secs(2));
    if let Some(endpoint) = discover_squarelaunch_ws_endpoint_udp(udp_timeout) {
        return Ok(endpoint);
    }

    let dns_sd_timeout = timeout
        .saturating_sub(udp_timeout)
        .max(Duration::from_secs(1));
    match discover_squarelaunch_ws_endpoint_dns_sd(dns_sd_timeout) {
        Ok(Some(endpoint)) => Ok(endpoint),
        Ok(None) => Err(format!(
            "SquareLaunch WebSocket discovery timed out; no {SQUARELAUNCH_WS_SERVICE} service found"
        )),
        Err(err) => Err(format!("SquareLaunch WebSocket discovery failed: {err}")),
    }
}

fn discover_squarelaunch_ws_endpoint_udp(
    timeout: Duration,
) -> Option<DiscoveredSquareLaunchEndpoint> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok()?;
    let query = build_mdns_ptr_query(SQUARELAUNCH_WS_SERVICE);
    let _ = socket.send_to(&query, SocketAddr::V4(M_DNS_ADDR));
    let start = Instant::now();
    let mut buf = [0_u8; 9000];
    let mut records = MdnsRecords::default();
    while start.elapsed() < timeout {
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                parse_mdns_packet(&buf[..n], &mut records);
                if let Some(endpoint) = records.endpoint() {
                    return Some(endpoint);
                }
            }
            Err(err) if timeout_or_would_block(&err) => {
                let _ = socket.send_to(&query, SocketAddr::V4(M_DNS_ADDR));
            }
            Err(_) => break,
        }
    }
    None
}

fn discover_squarelaunch_ws_endpoint_dns_sd(
    timeout: Duration,
) -> Result<Option<DiscoveredSquareLaunchEndpoint>, String> {
    let Some(instance) = browse_dns_sd_instance(timeout.min(Duration::from_secs(2)))? else {
        return Ok(None);
    };
    let remaining = timeout
        .saturating_sub(Duration::from_secs(2))
        .max(Duration::from_secs(1));
    resolve_dns_sd_instance(&instance, remaining)
}

fn browse_dns_sd_instance(timeout: Duration) -> Result<Option<String>, String> {
    run_dns_sd_until(
        &["-B", "_openlaunch-ws._tcp", "local"],
        timeout,
        parse_dns_sd_browse_instance,
    )
}

fn resolve_dns_sd_instance(
    instance: &str,
    timeout: Duration,
) -> Result<Option<DiscoveredSquareLaunchEndpoint>, String> {
    run_dns_sd_until(
        &["-L", instance, "_openlaunch-ws._tcp", "local"],
        timeout,
        parse_dns_sd_resolve_endpoint,
    )
}

fn run_dns_sd_until<T>(
    args: &[&str],
    timeout: Duration,
    mut parse: impl FnMut(&str) -> Option<T>,
) -> Result<Option<T>, String> {
    let mut child = Command::new(DNS_SD_BIN)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn {DNS_SD_BIN}: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture dns-sd stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture dns-sd stderr".to_string())?;
    let (tx, rx) = mpsc::channel();
    let stdout_tx = tx.clone();
    let stdout_reader = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if stdout_tx.send(line).is_err() {
                break;
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + timeout;
    let mut found = None;
    let mut failure = None;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(line) => {
                if let Some(err) = parse_dns_sd_error(&line) {
                    failure = Some(err);
                    break;
                }
                if let Some(value) = parse(&line) {
                    found = Some(value);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    if let Some(failure) = failure {
        Err(failure)
    } else {
        Ok(found)
    }
}

fn parse_dns_sd_browse_instance(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    while let Some(part) = parts.next() {
        if part.trim_end_matches('.') == "_openlaunch-ws._tcp" {
            let instance = parts.collect::<Vec<_>>().join(" ");
            return (!instance.trim().is_empty()).then(|| instance.trim().to_string());
        }
    }
    None
}

fn parse_dns_sd_resolve_endpoint(line: &str) -> Option<DiscoveredSquareLaunchEndpoint> {
    let (_, rest) = line.split_once(" can be reached at ")?;
    let token = rest.split_whitespace().next()?;
    let (host, port) = token.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    let host = host.trim_end_matches('.').to_string();
    (!host.is_empty()).then_some(DiscoveredSquareLaunchEndpoint { host, port })
}

fn parse_dns_sd_error(line: &str) -> Option<String> {
    let trimmed = line.trim();
    (trimmed.starts_with("DNSServiceBrowse failed")
        || trimmed.starts_with("DNSServiceResolve failed")
        || trimmed.starts_with("DNSServiceQueryRecord failed"))
    .then(|| trimmed.to_string())
}

fn build_mdns_ptr_query(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0_u16.to_be_bytes());
    out.extend_from_slice(&0_u16.to_be_bytes());
    out.extend_from_slice(&1_u16.to_be_bytes());
    out.extend_from_slice(&0_u16.to_be_bytes());
    out.extend_from_slice(&0_u16.to_be_bytes());
    out.extend_from_slice(&0_u16.to_be_bytes());
    encode_dns_name(name, &mut out);
    out.extend_from_slice(&12_u16.to_be_bytes());
    out.extend_from_slice(&0x8001_u16.to_be_bytes());
    out
}

fn encode_dns_name(name: &str, out: &mut Vec<u8>) {
    for label in name.trim_end_matches('.').split('.') {
        out.push(label.len().min(63) as u8);
        out.extend_from_slice(
            label
                .as_bytes()
                .get(..label.len().min(63))
                .unwrap_or_default(),
        );
    }
    out.push(0);
}

#[derive(Default)]
struct MdnsRecords {
    instances: Vec<String>,
    srvs: Vec<(String, String, u16)>,
    addrs: Vec<(String, Ipv4Addr)>,
}

impl MdnsRecords {
    fn endpoint(&self) -> Option<DiscoveredSquareLaunchEndpoint> {
        self.srvs
            .iter()
            .find(|(name, _, _)| self.instances.is_empty() || self.instances.contains(name))
            .map(|(_, target, port)| {
                let host = self
                    .addrs
                    .iter()
                    .find(|(name, _)| dns_names_equal(name, target))
                    .map(|(_, ip)| ip.to_string())
                    .unwrap_or_else(|| target.trim_end_matches('.').to_string());
                DiscoveredSquareLaunchEndpoint { host, port: *port }
            })
    }
}

fn parse_mdns_packet(packet: &[u8], records: &mut MdnsRecords) {
    if packet.len() < 12 {
        return;
    }
    let qd = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    let an = u16::from_be_bytes([packet[6], packet[7]]) as usize;
    let ns = u16::from_be_bytes([packet[8], packet[9]]) as usize;
    let ar = u16::from_be_bytes([packet[10], packet[11]]) as usize;
    let mut offset = 12usize;
    for _ in 0..qd {
        let Some((_, next)) = decode_dns_name(packet, offset) else {
            return;
        };
        offset = next.saturating_add(4);
        if offset > packet.len() {
            return;
        }
    }
    for _ in 0..(an + ns + ar) {
        let Some((name, next)) = decode_dns_name(packet, offset) else {
            return;
        };
        offset = next;
        if offset + 10 > packet.len() {
            return;
        }
        let rr_type = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
        let rd_len = u16::from_be_bytes([packet[offset + 8], packet[offset + 9]]) as usize;
        offset += 10;
        if offset + rd_len > packet.len() {
            return;
        }
        let rdata = offset;
        match rr_type {
            1 if rd_len == 4 => {
                records.addrs.push((
                    name,
                    Ipv4Addr::new(
                        packet[rdata],
                        packet[rdata + 1],
                        packet[rdata + 2],
                        packet[rdata + 3],
                    ),
                ));
            }
            12 => {
                if let Some((ptr, _)) = decode_dns_name(packet, rdata) {
                    records.instances.push(ptr);
                }
            }
            33 if rd_len >= 6 => {
                let port = u16::from_be_bytes([packet[rdata + 4], packet[rdata + 5]]);
                if let Some((target, _)) = decode_dns_name(packet, rdata + 6) {
                    records.srvs.push((name, target, port));
                }
            }
            _ => {}
        }
        offset += rd_len;
    }
}

fn decode_dns_name(packet: &[u8], mut offset: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut next = offset;
    let mut jumps = 0usize;
    loop {
        let len = *packet.get(offset)?;
        if len & 0xc0 == 0xc0 {
            let b2 = *packet.get(offset + 1)?;
            let ptr = (usize::from(len & 0x3f) << 8) | usize::from(b2);
            if !jumped {
                next = offset + 2;
            }
            offset = ptr;
            jumped = true;
            jumps += 1;
            if jumps > 16 {
                return None;
            }
            continue;
        }
        offset += 1;
        if len == 0 {
            if !jumped {
                next = offset;
            }
            break;
        }
        let end = offset.checked_add(usize::from(len))?;
        let label = std::str::from_utf8(packet.get(offset..end)?).ok()?;
        labels.push(label.to_string());
        offset = end;
    }
    Some((format!("{}.", labels.join(".")), next))
}

fn dns_names_equal(a: &str, b: &str) -> bool {
    a.trim_end_matches('.')
        .eq_ignore_ascii_case(b.trim_end_matches('.'))
}

fn timeout_or_would_block(err: &std::io::Error) -> bool {
    matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mdns_endpoint_uses_srv_port_and_a_record_address() {
        let mut records = MdnsRecords::default();
        records
            .instances
            .push("SQUARELAUNCH B0100SIM._openlaunch-ws._tcp.local.".to_string());
        records.srvs.push((
            "SQUARELAUNCH B0100SIM._openlaunch-ws._tcp.local.".to_string(),
            "squarelaunch-sim.local.".to_string(),
            3030,
        ));
        records.addrs.push((
            "squarelaunch-sim.local.".to_string(),
            Ipv4Addr::new(192, 168, 1, 50),
        ));

        let endpoint = records.endpoint().expect("mdns endpoint");

        assert_eq!(endpoint.host, "192.168.1.50");
        assert_eq!(endpoint.port, 3030);
    }

    #[test]
    fn mdns_parser_accepts_real_openlaunch_simulator_packet() {
        let packet = hex_bytes(
            "0000840000010001000000040e5f6f70656e6c61756e63682d7773045f746370056c6f63616c00000c0001c00c000c00010000119400100d4e4f564120423031303053494dc00cc03700210001000000780018000000000b680f5368616e65732d4d61632d6d696e69c020c037002f0001000000780008c037000400000008c05900010001000000780004c0a8443ac0370010000100001194006b0a6d6f64656c3d4e4f5641186d616e7566616374757265723d4f70656e204c61756e63680f73657269616c3d423031303053494d18686f73746e616d653d5368616e65732d4d61632d6d696e691d76657273696f6e3d73696d756c61746f722d323032352d4465632d3032",
        );
        let mut records = MdnsRecords::default();

        parse_mdns_packet(&packet, &mut records);
        let endpoint = records.endpoint().expect("simulator endpoint");

        assert_eq!(endpoint.host, "192.168.68.58");
        assert_eq!(endpoint.port, 2920);
    }

    #[test]
    #[ignore = "requires a live SquareLaunch/OpenLaunch device advertising _openlaunch-ws._tcp.local."]
    fn live_discovers_squarelaunch_ws_endpoint() {
        let endpoint =
            discover_squarelaunch_ws_endpoint(Duration::from_secs(5)).expect("live discovery");

        assert_eq!(endpoint.port, 2920);
    }

    #[test]
    fn dns_sd_browse_output_parses_instance_name() {
        let line = "18:41:09.123  Add     3  14 local. _openlaunch-ws._tcp. SQUARELAUNCH B0100SIM";

        let instance = parse_dns_sd_browse_instance(line).expect("dns-sd browse instance");

        assert_eq!(instance, "SQUARELAUNCH B0100SIM");
    }

    #[test]
    fn dns_sd_resolve_output_parses_host_and_port() {
        let line = "18:41:11.456  SQUARELAUNCH B0100SIM._openlaunch-ws._tcp.local. can be reached at squarelaunch-sim.local.:3030 (interface 14)";

        let endpoint = parse_dns_sd_resolve_endpoint(line).expect("dns-sd resolve endpoint");

        assert_eq!(endpoint.host, "squarelaunch-sim.local");
        assert_eq!(endpoint.port, 3030);
    }

    #[test]
    fn dns_sd_failure_line_is_reported() {
        let line = "DNSServiceBrowse failed -65563 (Service Not Running)";

        let error = parse_dns_sd_error(line).expect("dns-sd failure");

        assert_eq!(
            error,
            "DNSServiceBrowse failed -65563 (Service Not Running)"
        );
    }

    fn hex_bytes(hex: &str) -> Vec<u8> {
        hex.as_bytes()
            .chunks(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).expect("hex utf8");
                u8::from_str_radix(text, 16).expect("hex byte")
            })
            .collect()
    }
}
