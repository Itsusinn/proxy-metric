use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::Instant,
};
use tokio_socks::tcp::Socks5Stream;

const PAYLOAD_SIZE: usize = 1024 * 1024;
const SEND_ROUND: usize = 256;

#[tokio::main]
async fn main() {
    println!("Test Direct");
    throughput(None).await;
    println!("Test Proxy");
    throughput(Some(&SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 27890).into())).await;
}

pub enum DirectOrProxy {
    Direct(TcpStream),
    ProxyStream(Socks5Stream<TcpStream>),
}

pub async fn throughput(proxy: Option<&SocketAddr>) {
    let start_time = Instant::now();
    upload_throughput(proxy).await;
    let connect_duration = start_time.elapsed().as_secs_f64();
    println!(
        "Upload speed: {:.2} MB/s",
        (PAYLOAD_SIZE * SEND_ROUND / (1024 * 1024)) as f64 / connect_duration
    );
    let start_time = Instant::now();
    download_throughput(proxy).await;
    let connect_duration = start_time.elapsed().as_secs_f64();
    println!(
        "Download speed: {:.2} MB/s",
        (PAYLOAD_SIZE * SEND_ROUND / (1024 * 1024)) as f64 / connect_duration
    );
}

async fn get_tcp_stream_pair(proxy: Option<&SocketAddr>) -> (DirectOrProxy, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        tx.send(stream).await.unwrap();
    });
    let client_stream = if let Some(proxy) = proxy {
        let client_stream = Socks5Stream::connect(proxy, local_addr).await.unwrap();
        DirectOrProxy::ProxyStream(client_stream)
    } else {
        let client_stream = TcpStream::connect(local_addr).await.unwrap();
        DirectOrProxy::Direct(client_stream)
    };
    let server_stream = rx.recv().await.unwrap();

    return (client_stream, server_stream);
}

async fn upload_throughput(proxy: Option<&SocketAddr>) {
    let (mut stream0, mut stream1) = get_tcp_stream_pair(proxy).await;

    tokio::spawn(async move {
        let mut buf: Vec<u8> = vec![];
        buf.resize(PAYLOAD_SIZE, 0);
        for _ in 0..SEND_ROUND {
            match &mut stream0 {
                DirectOrProxy::Direct(stream0) => stream0.write_all(&buf).await.unwrap(),
                DirectOrProxy::ProxyStream(stream0) => stream0.write_all(&buf).await.unwrap(),
            }
        }
    });

    let mut buf: Vec<u8> = vec![];
    buf.resize(PAYLOAD_SIZE, 0);
    for _ in 0..SEND_ROUND {
        stream1.read_exact(&mut buf).await.unwrap();
    }
}
async fn download_throughput(proxy: Option<&SocketAddr>) {
    let (mut stream0, mut stream1) = get_tcp_stream_pair(proxy).await;

    tokio::spawn(async move {
        let mut buf: Vec<u8> = vec![];
        buf.resize(PAYLOAD_SIZE, 0);
        for _ in 0..SEND_ROUND {
            stream1.write_all(&buf).await.unwrap();
        }
    });

    let mut buf: Vec<u8> = vec![];
    buf.resize(PAYLOAD_SIZE, 0);
    for _ in 0..SEND_ROUND {
        match &mut stream0 {
            DirectOrProxy::Direct(stream0) => stream0.read_exact(&mut buf).await.unwrap(),
            DirectOrProxy::ProxyStream(stream0) => stream0.read_exact(&mut buf).await.unwrap(),
        };
    }
}
