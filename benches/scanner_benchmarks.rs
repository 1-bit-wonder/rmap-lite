//! Performance benchmarks for rmap-lite port scanner.
//!
//! Run with: cargo bench
//!
//! These benchmarks measure real network I/O latency and produce
//! p50/p90/p95/p99 percentile statistics suitable for resume/documentation.

use futures::{stream, StreamExt};
use std::{
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};
use tokio::{net::TcpListener, net::TcpStream, runtime::Runtime, time::timeout};

/// Port check function (mirrors main.rs implementation)
async fn is_open(target_ip: Ipv4Addr, port: u16, timeout_ms: u64) -> Option<u16> {
    let addr = SocketAddr::new(target_ip.into(), port);
    let timeout_duration = Duration::from_millis(timeout_ms);

    if let Ok(Ok(_)) = timeout(timeout_duration, TcpStream::connect(addr)).await {
        Some(port)
    } else {
        None
    }
}

/// Scan function (mirrors main.rs implementation)
async fn scan(
    target_ip: Ipv4Addr,
    port_from: u16,
    port_to: u16,
    concurrency: usize,
    timeout_val: u64,
) -> Vec<u16> {
    let ports_to_scan: Vec<u16> = (port_from..=port_to).collect();

    let mut open_ports: Vec<u16> = stream::iter(ports_to_scan)
        .map(|port| async move { is_open(target_ip, port, timeout_val).await })
        .buffer_unordered(concurrency)
        .filter_map(std::future::ready)
        .collect()
        .await;

    open_ports.sort_unstable();
    open_ports
}

/// Calculate percentile from sorted slice
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Print latency statistics
fn print_stats(name: &str, latencies_us: &mut [u64], total_elapsed: Duration) {
    latencies_us.sort_unstable();
    let n = latencies_us.len();

    if n == 0 {
        println!("{}: No samples collected", name);
        return;
    }

    let sum: u64 = latencies_us.iter().sum();
    let mean = sum as f64 / n as f64;
    let throughput = n as f64 / total_elapsed.as_secs_f64();

    println!("\n{} ({} iterations)", name, n);
    println!("  Min:        {:>8.2} ms", latencies_us[0] as f64 / 1000.0);
    println!(
        "  Max:        {:>8.2} ms",
        latencies_us[n - 1] as f64 / 1000.0
    );
    println!("  Mean:       {:>8.2} ms", mean / 1000.0);
    println!(
        "  p50:        {:>8.2} ms",
        percentile(latencies_us, 50.0) as f64 / 1000.0
    );
    println!(
        "  p90:        {:>8.2} ms",
        percentile(latencies_us, 90.0) as f64 / 1000.0
    );
    println!(
        "  p95:        {:>8.2} ms",
        percentile(latencies_us, 95.0) as f64 / 1000.0
    );
    println!(
        "  p99:        {:>8.2} ms",
        percentile(latencies_us, 99.0) as f64 / 1000.0
    );
    println!("  Throughput: {:>8.0} ops/sec", throughput);
}

fn main() {
    println!("=== rmap-lite Performance Benchmarks ===\n");

    let rt = Runtime::new().unwrap();

    // Benchmark 1: Open port connection latency
    {
        let listener = rt.block_on(async { TcpListener::bind("127.0.0.1:0").await.unwrap() });
        let port = listener.local_addr().unwrap().port();
        let ip = Ipv4Addr::new(127, 0, 0, 1);

        let mut latencies = Vec::with_capacity(50);
        let start = Instant::now();

        rt.block_on(async {
            for _ in 0..50 {
                let t = Instant::now();
                let _ = is_open(ip, port, 1000).await;
                latencies.push(t.elapsed().as_micros() as u64);
            }
        });

        let elapsed = start.elapsed();
        print_stats("Open Port Connection Latency", &mut latencies, elapsed);
        drop(listener);
    }

    // Benchmark 2: Closed port rejection latency
    {
        let port = rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            port
        });
        let ip = Ipv4Addr::new(127, 0, 0, 1);

        let mut latencies = Vec::with_capacity(50);
        let start = Instant::now();

        rt.block_on(async {
            for _ in 0..50 {
                let t = Instant::now();
                let _ = is_open(ip, port, 50).await;
                latencies.push(t.elapsed().as_micros() as u64);
            }
        });

        let elapsed = start.elapsed();
        print_stats("Closed Port Rejection Latency", &mut latencies, elapsed);
    }

    // Benchmark 3: Concurrent connections
    {
        let listeners: Vec<_> = rt.block_on(async {
            let mut listeners = Vec::new();
            for _ in 0..20 {
                listeners.push(TcpListener::bind("127.0.0.1:0").await.unwrap());
            }
            listeners
        });

        let ports: Vec<u16> = listeners
            .iter()
            .map(|l| l.local_addr().unwrap().port())
            .collect();

        let ip = Ipv4Addr::new(127, 0, 0, 1);

        let mut latencies = Vec::with_capacity(20);
        let start = Instant::now();

        rt.block_on(async {
            for _ in 0..20 {
                let t = Instant::now();
                let _: Vec<_> = stream::iter(ports.clone())
                    .map(|port| async move { is_open(ip, port, 500).await })
                    .buffer_unordered(20)
                    .collect()
                    .await;
                latencies.push(t.elapsed().as_micros() as u64);
            }
        });

        let elapsed = start.elapsed();
        print_stats("20 Concurrent Connections (batch)", &mut latencies, elapsed);

        let ports_per_sec = (20 * 20) as f64 / elapsed.as_secs_f64();
        println!("  Effective:  {:>8.0} ports/sec", ports_per_sec);

        drop(listeners);
    }

    // Benchmark 4: Port range scan
    {
        let listeners: Vec<_> = rt.block_on(async {
            let mut listeners = Vec::new();
            for _ in 0..5 {
                listeners.push(TcpListener::bind("127.0.0.1:0").await.unwrap());
            }
            listeners
        });

        let ports: Vec<u16> = listeners
            .iter()
            .map(|l| l.local_addr().unwrap().port())
            .collect();

        let min_port = *ports.iter().min().unwrap();
        let max_port = *ports.iter().max().unwrap();
        let port_count = max_port - min_port + 1;
        let ip = Ipv4Addr::new(127, 0, 0, 1);

        let mut latencies = Vec::with_capacity(10);
        let start = Instant::now();

        rt.block_on(async {
            for _ in 0..10 {
                let t = Instant::now();
                let _ = scan(ip, min_port, max_port, 100, 100).await;
                latencies.push(t.elapsed().as_micros() as u64);
            }
        });

        let elapsed = start.elapsed();
        print_stats(
            &format!("Port Range Scan ({} ports)", port_count),
            &mut latencies,
            elapsed,
        );

        let ports_per_sec = (port_count as usize * 10) as f64 / elapsed.as_secs_f64();
        println!("  Scan rate:  {:>8.0} ports/sec", ports_per_sec);

        drop(listeners);
    }

    println!("\n=== Benchmarks Complete ===");
}
