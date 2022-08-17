use clap::Parser;
use indicatif::ProgressBar;
use tokio::{net::{TcpStream, ToSocketAddrs}};
use futures::{StreamExt, stream::FuturesUnordered};
use std::{vec::Vec, time::Instant, net::Ipv4Addr, str::FromStr, ops::Range};

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
}

async fn is_open<A: ToSocketAddrs>(target: A, port: u16) -> (u16, bool) {
    let tcp = TcpStream::connect(target).await;
    (port, tcp.is_ok())
}

async fn scan(target: Ipv4Addr, range: Range<u16>) -> Vec<u16> {
    let mut scanning = FuturesUnordered::new();
    let mut open_ports: Vec<u16> = Vec::new();
    let bar = ProgressBar::new(range.end.into());

    for port in range {
        scanning.push(is_open((target, port), port));
    }

    while let Some((port, open)) = scanning.next().await {
        if open {
            open_ports.push(port);
        }

        bar.inc(1);
    }

    bar.finish();

    return open_ports;
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let target = Ipv4Addr::from_str(&args.target).unwrap();
    let range = Range {start: args.port_from, end: args.port_to};
    let timer = Instant::now();
    let open_ports = scan(target, range).await;

    println!("Found {:?} open ports in {} seconds:", open_ports.len(), timer.elapsed().as_secs());
    println!("{:?}", open_ports);
}