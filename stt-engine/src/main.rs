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
            let config = ServerConfig::new("127.0.0.1", 8888, 5, 20, 2, 2, 2);
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
                    match client::run_with(wav_file).await {
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
            benchmark::run_benchmark().await;
        },
        _ => {
            println!("Unknown command: {}", &args[1]);
        }
    };
}