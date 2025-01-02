use std::sync::{Arc, Mutex};

use tokio::{signal::ctrl_c, sync::mpsc, task::JoinHandle};

use crate::client;

pub async fn run_benchmark() {
    tokio::select! {
        _ = async move {
            let (tx, mut rx) = mpsc::channel::<JoinHandle<()>>(10);
            tokio::spawn(async move {
                let handles: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
                let max_support = 5;
                while let Some(handle) = rx.recv().await {
                    println!("Received handle");
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
                let handle = tokio::spawn(async move {
                    match client::run_with(wav_file).await {
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