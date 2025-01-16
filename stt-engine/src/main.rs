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
                    .long("workers")
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
                .arg(Arg::new("wav_dir")
                    .short('d')
                    .long("dir")
                    .required(true)
                    .value_name("WAV_DIR"))
                .arg(Arg::new("client_count")
                    .short('c')
                    .long("clients")
                    .default_value("1")
                    .value_name("CLIENT_COUNT"))
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
                let wav_dir = sub_m.get_one::<String>("wav_dir").unwrap();
                let client_count = sub_m.get_one::<String>("client_count")
                    .map(|w| {
                        match w.parse::<usize>() {
                            Ok(w) => w,
                            Err(_) => 1,
                        }
                    }).unwrap();
                println!("Starting {} clients on {}:{}, wav dir: {}",client_count, ip, port, wav_dir);

                let mut clients = vec![];
                for i in 0..client_count {
                    let ip = ip.clone();
                    let wav_dir = wav_dir.clone();
                    let client = tokio::spawn(async move {
                        let wav_file = format!("{}/split_part_{}.wav", wav_dir, i + 1);
                        if let Some(client) = TcpClientEndpoint::init(TcpClientConfig::new(ip, port, wav_file)).await {
                            client.execute_async().await;
                        }
                    });
                    clients.push(client);
                }

                for client in clients {
                    client.await.unwrap();
                }
            },
            _ => {},
        }
}