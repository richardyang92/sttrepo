use stt_engine::{benchmark, client, endpoint::{server::{Server, ServerConfig}, Endpoint}};

#[tokio::main]
// 主函数，解析参数如果是server则启动服务端，如果是client则启动客户端
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} [server|client] [options]", &args[0]);
        return;
    }
    match &*args[1] {
        "server" => {
            let config = ServerConfig::new("127.0.0.1", 8888, 10, 20, false, 2, 2, 2);
            if let Some(server) = Server::init(config).await {
                println!("Server started on 127.0.0.1:8888");
                server.run().await;
            }
        }, 
        "client" => {
            let mut joints = Vec::new();
            for i in 0..20 {
                let joint = tokio::spawn(async move {
                    let wav_file = format!("./data/segment/split_part_{}.wav", i + 1);
                    println!("Sending file: {}", wav_file);
                    match client::run_with("127.0.0.1".to_string(), 8888, wav_file, true).await {
                        Ok(res) => {
                            println!("Received response: {:?}", res);
                        },
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                });
                joints.push(joint);
            }

            for joint in joints {
                joint.await.unwrap();
            }
        },
        "benchmark" => {
            // 如果args长度为3，则第三个参数为服务器地址，否则默认为127.0.0.1
            let server_addr = if args.len() == 3 {
                args[2].clone()
            } else {
                "127.0.0.1".to_string()
            };
            benchmark::run_benchmark(server_addr, 8888, 15).await;
        },
        _ => {
            println!("Unknown command: {}", &args[1]);
        }
    };
}