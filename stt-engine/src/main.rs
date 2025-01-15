use stt_engine::{client::{TcpClientConfig, TcpClientEndpoint}, endpoint::{AsyncExecute, Endpoint}, server::{proxy::{TcpListenerConfig, TcpListenerEndpoint}, worker::{TcpWorkerConfig, TcpWorkerEndpoint}}};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} [server|client] [options]", &args[0]);
        return;
    }
    match &*args[1] {
        "proxy" => {
            if let Some(listener) = TcpListenerEndpoint::init(TcpListenerConfig::new("0.0.0.0".to_string(), 8888)).await {
                listener.execute_async().await
            }
        }, 
        "worker" => {
            let worker = TcpWorkerEndpoint::init(TcpWorkerConfig::new("127.0.0.1".to_string(), 8888)).await.expect("Failed to init worker");
            worker.execute_async().await;
        },
        "client" => {
            let client = TcpClientEndpoint::init(TcpClientConfig::new("127.0.0.1".to_string(), 8888)).await.expect("Failed to init client");
            client.execute_async().await;
        },
        _ => {
            println!("Unknown command: {}", &args[1]);
        }
    };
}