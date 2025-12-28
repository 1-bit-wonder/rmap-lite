use clap::Parser;
use dns_lookup::lookup_host;
use futures::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    net::{Ipv4Addr, SocketAddr},
    process,
    str::FromStr,
    time::{Duration, Instant},
};
use tokio::{net::TcpStream, time::timeout};

/// A simple concurrent portscanner in Rust.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, value_parser, default_value = "127.0.0.1")]
    target: String,
    #[clap(long, value_parser, default_value = "1")]
    port_from: u16,
    #[clap(long, value_parser, default_value = "65535")]
    port_to: u16,
    #[clap(long, value_parser, default_value = "1000")]
    concurrency: usize,
    #[clap(long, value_parser, default_value = "1500")]
    timeout: u64,
}

async fn is_open(target_ip: Ipv4Addr, port: u16, timeout_ms: u64) -> Option<u16> {
    let addr = SocketAddr::new(target_ip.into(), port);
    let timeout_duration = Duration::from_millis(timeout_ms);

    if let Ok(Ok(_)) = timeout(timeout_duration, TcpStream::connect(&addr)).await {
        Some(port)
    } else {
        None
    }
}

async fn scan(
    target_ip: Ipv4Addr,
    port_from: u16,
    port_to: u16,
    concurrency: usize,
    timeout_val: u64,
) -> Vec<u16> {
    let ports_to_scan: Vec<u16> = (port_from..=port_to).collect();
    let total_ports = ports_to_scan.len();

    let bar = ProgressBar::new(total_ports as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{elapsed_precise} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap() // This is safe because the template is static and valid.
            .progress_chars("##-"),
    );

    let open_ports: Vec<u16> = stream::iter(ports_to_scan)
        .map(|port| {
            let bar_clone = bar.clone();
            async move {
                let result = is_open(target_ip, port, timeout_val).await;
                bar_clone.inc(1);
                result
            }
        })
        .buffer_unordered(concurrency)
        .filter_map(|p| async move { p })
        .collect()
        .await;

    bar.finish();

    open_ports
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let ips = match lookup_host(&args.target) {
        Ok(ips) => ips,
        Err(e) => {
            eprintln!("DNS resolution failed for target '{}': {}", args.target, e);
            process::exit(1);
        }
    };

    let ip_addr_str = if let Some(ip) = ips.into_iter().find(|ip| ip.is_ipv4()) {
        ip.to_string()
    } else {
        eprintln!("No IPv4 address found for target '{}'", args.target);
        process::exit(1);
    };

    let target_ip = match Ipv4Addr::from_str(&ip_addr_str) {
        Ok(ip) => ip,
        Err(_) => {
            // This should ideally not happen since we filtered for IPv4, but for robustness:
            eprintln!("Failed to parse IP address '{}'", ip_addr_str);
            process::exit(1);
        }
    };

    let timer = Instant::now();
    let open_ports = scan(
        target_ip,
        args.port_from,
        args.port_to,
        args.concurrency,
        args.timeout,
    )
    .await;

    println!(); // Newline after progress bar
    println!(
        "Found {} open ports in {:.2} seconds:",
        open_ports.len(),
        timer.elapsed().as_secs_f32()
    );
    if !open_ports.is_empty() {
        let mut sorted_ports = open_ports;
        sorted_ports.sort_unstable();
        println!("{:?}", sorted_ports);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_is_open_with_open_port() {
        // Bind a listener to a random available port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        let ip = Ipv4Addr::new(127, 0, 0, 1);

        let result = is_open(ip, port, 1000).await;
        assert_eq!(result, Some(port));
    }

    #[tokio::test]
    async fn test_is_open_with_closed_port() {
        // Bind a listener to get an ephemeral port, then close it
        // to make sure we test a port that is not open.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        drop(listener); // Drop the listener to close the port.

        let ip = Ipv4Addr::new(127, 0, 0, 1);

        let result = is_open(ip, port, 1000).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_is_open_timeout() {
        // 192.0.2.1 is a TEST-NET-1 address, reserved for documentation and should not be reachable.
        let ip = Ipv4Addr::new(192, 0, 2, 1);
        let port = 80; // A common port, but the IP is the key here.
        let start = Instant::now();
        let result = is_open(ip, port, 100).await; // 100ms timeout
        let duration = start.elapsed();

        assert_eq!(result, None);
        // Check that it actually timed out and didn't return immediately.
        // It should take at least 100ms, but we'll allow for some buffer.
        assert!(duration >= Duration::from_millis(100));
        // And it shouldn't take too long, e.g. the default TCP timeout.
        assert!(duration < Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_scan_finds_open_ports() {
        // Bind a couple of listeners
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let port1 = addr1.port();

        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let port2 = addr2.port();

        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let min_port = std::cmp::min(port1, port2);
        let max_port = std::cmp::max(port1, port2);

        let mut open_ports = scan(ip, min_port, max_port, 100, 1000).await;
        open_ports.sort_unstable();

        let mut expected_ports = vec![port1, port2];
        expected_ports.sort_unstable();

        assert_eq!(open_ports, expected_ports);
    }
}
