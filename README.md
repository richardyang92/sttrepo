# Stt engine
## 1. Installing & building dependencies
### 1.1 Downloading third-party dependencies
Firstly, init submodule [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) by running:
```
cd ${workspace_folder}
git submodule update --init
```
this will clone the sherpa-onnx repo into `third-party/sherpa-onnx` folder.

then, donwload [libevent](https://libevent.org) (optional, for c-api build) by running:
```
cd third-party
wget "https://github.com/libevent/libevent/releases/download/release-2.1.12-stable/libevent-2.1.12-stable.tar.gz"
tar -xvf libevent-2.1.12-stable.tar.gz
```
### 1.2 Building third-party dependencies
We will build sherpa-onnx and libevent (optional) in `sherpa/third-party/` folder.

Firslty, build sherpa-onnx by running:
```
cd ${workspace_folder}/third_party/sherpa-onnx
mkdir build-shared
cd build-shared
cmake \
  -DSHERPA_ONNX_ENABLE_C_API=ON \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=ON \
  -DCMAKE_INSTALL_PREFIX=${workspace_folder}/sherpa/third_party/sherpa-onnx \
  -Wno-dev \
  ..
  make -j8
  make install
  ```
  Then, build libevent (optional) by running:
  ```
  cd ${workspace_folder}/third-party/libevent-2.1.12-stable
  ./configure --prefix=${workspace_folder}/sherpa/third_party/libevent
  make -j8
  mak install
  ```
  If your build is failed with error message like ` aclocal-1.15: 未找到命令`, you can try the following command to fix it:
  ```
  autoreconf  -ivf 
  ```
  ## 2. Building stt engine
  ### 2.1 Building sherpa-bridge
  Just run follwoing command to build `libsherpa-bridge.so` for unix-like system, or `libsherpa-bridge.dylib` for macOS:
  ```
  cd ${workspace_folder}/sherpa/sherpa-bridge
  make clean & make
  ```
  If you want to test whether the build is successful, you can run:
  ```
  make test
  ./sherpa_test
  ```
  ### 2.2 Building stt-engine excuable
 We now support both rust and c++ build.
 #### 2.2.1 Rust build
 Executing following command to build stt-engine:
 ```
 cd ${workspace_folder}/sherpa/stt-engine
 cargo build --release
 ```
 #### 2.2.2 C++ build
 Executing following command to build stt-engine:
 ```
 cd ${workspace_folder}/sherpa/stt-c-api
 make clean & make
 ```
## 3. Running stt-engine
### 3.1 Downloading models
We need download models from [sherpa-onnx models](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/online-transducer/zipformer-transducer-models.html#sherpa-onnx-streaming-zipformer-multi-zh-hans-2023-12-12-chinese) repo before we can run stt-engine. By running following command to download models:
```
cd ${workspace_folder}/sherpa/sherpa-models
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-multi-zh-hans-2023-12-12.tar.bz2

tar xf sherpa-onnx-streaming-zipformer-multi-zh-hans-2023-12-12.tar.bz2
rm sherpa-onnx-streaming-zipformer-multi-zh-hans-2023-12-12.tar.bz2
ls -lh sherpa-onnx-streaming-zipformer-multi-zh-hans-2023-12-12
```
### 3.2 Running rust build
Executing following command to run stt-engine:
1. run as server
```
cd ${workspace_folder}/sherpa/stt-engine
cargo run server
```
2. run as client
```
cd ${workspace_folder}/sherpa/stt-engine
cargo run client
```
### 3.3 Running c++ build
Executing following command to run stt-engine:
```
cd ${workspace_folder}/sherpa/stt-c-api
./sherpa-c-api
```
## 4.Benchmark
All tests are run on a MacBook Air(RAM 16G/Storage 256G) with Apple M1 Max chip.
### 4.1 Speed
The server contains 10 sherpa handle and client request 10 times in parallel. To run this test, we need to run the server and client in two different terminals.
Step 1: Open one terminal and run a server:
For rust build:
```
cd ${workspace_folder}/stt-engine
cargo run server
```
And for c++ build:
```
cd ${workspace_folder}/sherpa/stt-c-api
./sherpa-c-api
```
Step 2: Open another terminal and run a client for 5 times and calculate the average time:
```
cd ${workspace_folder}/stt-engine
cargo run client
```

| Build Type | Test1 | Test2 | Test3 | Test4 | Test5 | Average |
| --- | --- | --- | --- | --- | --- | --- |
| Rust Build | 6.56123050s | 7.35451825s | 6.756349833s | 6.650826125s | 5.537515375s | 6.57208802s |
| C++ Build | 9.330376833s | 10.400389667s | 10.289438333s | 10.269550458s | 10.33128725s | 10.12420851 |

For now, the C++ build is slower than the Rust build. I guess it's because the libevent library is not making full use of the multi-core CPU's performance. The CPU usage for the C++ build is about 100%, whereas the Rust build exceeds 400% CPU usage.
### 4.2 Stabliity
The server contains 10 sherpa handle and client continuesly request to server. To run this test, we need to run the server and client in two different terminals.
Step 1: Open a termianl and run a server:
For rust build:
```
cd ${workspace_folder}/stt-engine
cargo run server
```
And for c++ build:
```
cd ${workspace_folder}/sherpa/stt-c-api
./sherpa-c-api
```
Step 2: Open another terminal and run a client:
```
cd ${workspace_folder}/stt-engine
cargo run benchmark
```

| Running Time | Rust Build | C++ Build |
| --- | --- | --- |
| 30min | &#10004; Pass | &#10004; Pass |
| 1hour | &#10004; Pass | &#10004; Pass |
| 2hour | &#10006; Not test yet | &#10006;  Not test yet |
| ... | ... | ... |
| 1day | &#10006; Not test yet | &#10006;  Not test yet |
## 5. Reference
The following open-source projects are used in this repository:
1. `sherpa-onnx`: https://github.com/k2-fsa/sherpa-onnx
2. `libvevent`: https://libevent.org

The model used in this repository is from:
1. `sherpa-onnx`: https://github.com/k2-fsa/sherpa-onnx