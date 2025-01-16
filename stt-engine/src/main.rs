use clap::{Arg, Command};
use stt_engine::{client::{TcpClientConfig, TcpClientEndpoint}, endpoint::{AsyncExecute, Endpoint}, server::{proxy::{TcpListenerConfig, TcpListenerEndpoint}, worker::{TcpWorkerConfig, TcpWorkerEndpoint}}};

#[tokio::main]
async fn main() {
    let matches = Command::new("SttEngine")
        .version("0.1.2-alpha")
        .about("A tiny speech to tex tool written by Rust")
        .subcommand(
            Command::new("proxy")
                .about("Start a proxy endpoint")
                .arg(Arg::new("listen")
                    .short('l')
                    .long("listen")
                    .default_value("0.0.0.0")
                    .value_name("LISTENER_IPADDR"))
                .arg(Arg::new("port")
                    .short('p')
                    .long("port")
                    .default_value("8888")
                    .value_name("LISTENER_PORT"))
        )
        .subcommand(
            Command::new("worker")
                .about("Start a worker endpoint")
                .arg(Arg::new("ip")
                    .short('i')
                    .long("ip")
                    .default_value("127.0.0.1")
                    .value_name("PROXY_IPADDR"))
                .arg(Arg::new("port")
                    .short('p')
                    .long("port")
                    .default_value("8888")
                    .value_name("PROXY_PORT"))
                .arg(Arg::new("woker_count")
                    .short('w')
                    .long("wokers")
                    .default_value("1")
                    .value_name("WORKER_COUNT"))
            
        )
        .subcommand(
            Command::new("client")
                .about("Start a client endpoint")
                .arg(Arg::new("ip")
                    .short('i')
                    .long("ip")
                    .default_value("127.0.0.1")
                    .value_name("PROXY_IPADDR"))
                .arg(Arg::new("port")
                    .short('p')
                    .long("port")
                    .default_value("8888")
                    .value_name("PROXY_PORT"))
                .arg(Arg::new("file")
                    .short('f')
                    .long("file")
                    .required(true)
                    .value_name("WAV_FILE_PATH"))
        ).get_matches();

        match matches.subcommand() {
            Some(("proxy", sub_m)) => {
                let ip = sub_m.get_one::<String>("listen").unwrap();
                let port = sub_m.get_one::<String>("port")
                    .map(|p| {
                        match p.parse::<u16>() {
                            Ok(p) => p,
                            Err(_) => 8888,
                        }
                    }).unwrap();
                println!("Starting proxy on {}:{}",ip, port);
                if let Some(listener) = TcpListenerEndpoint::init(
                    TcpListenerConfig::new(String::from(ip), port.clone())).await {
                    listener.execute_async().await;
                }
            }
            Some(("worker", sub_m)) => {
                let ip = sub_m.get_one::<String>("ip").unwrap();
                let port = sub_m.get_one::<String>("port")
                    .map(|p| {
                        match p.parse::<u16>() {
                            Ok(p) => p,
                            Err(_) => 8888,
                        }
                    }).unwrap();
                let worker_count = sub_m.get_one::<String>("woker_count")
                    .map(|w| {
                        match w.parse::<usize>() {
                            Ok(w) => w,
                            Err(_) => 1,
                        }
                    }).unwrap();
                println!("Starting {} workers on {}:{}",worker_count, ip, port);

                let mut workers = vec![];
                for _ in 0..worker_count {
                    let ip = ip.clone();
                    let worker = tokio::spawn(async move {
                        if let Some(worker) = TcpWorkerEndpoint::init(TcpWorkerConfig::new(ip, port)).await {
                            worker.execute_async().await;
                        }
                    });
                    workers.push(worker);
                }
                for worker in workers {
                    worker.await.unwrap();
                }
            },
            Some(("client", sub_m)) => {
                let ip = sub_m.get_one::<String>("ip").unwrap();
                let port = sub_m.get_one::<String>("port")
                    .map(|p| {
                        match p.parse::<u16>() {
                            Ok(p) => p,
                            Err(_) => 8888,
                        }
                    }).unwrap();
                let file_path = sub_m.get_one::<String>("file").unwrap();
                println!("Starting client on {}:{}, processing file: {}",ip, port, file_path);

                if let Some(client) = TcpClientEndpoint::init(TcpClientConfig::new(ip.to_string(), port, file_path.to_string())).await {
                    client.execute_async().await;
                }
            },
            _ => {},
        }
}