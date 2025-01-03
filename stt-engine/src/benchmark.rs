use std::sync::{Arc, Mutex};

use tokio::{signal::ctrl_c, sync::mpsc, task::JoinHandle};

use crate::client;

pub async fn run_benchmark(ip: String, port: u16, max_clients: usize) {
    tokio::select! {
        _ = async move {
            let (tx, mut rx) = mpsc::channel::<JoinHandle<()>>(max_clients);
            tokio::spawn(async move {
                let handles: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
                let max_support = max_clients;
                while let Some(handle) = rx.recv().await {
                    // println!("Received handle");
                    match handles.lock() {
                        Ok(mut handles) => {
                            // 如果handles长度max_support, 则等待其中一个完成
                            while handles.len() >= max_support {
                                for i in 0..handles.len() {
                                    if handles[i].is_finished() {
                                        handles.remove(i);
                                        break;
                                    }
                                }
                            }
                            handles.push(handle);
                        }
                        Err(_) => {}
                    }
                }
            });
            let mut i = 1;
            loop {
                let wav_file = format!("./data/segment/split_part_{}.wav", i);
                if i > 100 {
                    i = 1;
                }
                let ip = ip.clone();
                let handle = tokio::spawn(async move {
                    match client::run_with(ip, port, wav_file, false).await {
                        Ok(res) => {
                            println!("Received response: {:?}", res);
                        },
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                });
                tx.send(handle).await.unwrap();
                i += 1;
            }
        } => {},
        _ = ctrl_c() => {},
    }
}