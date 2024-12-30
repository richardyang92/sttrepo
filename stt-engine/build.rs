use std::env;

fn main() {
    // 获取当前工作目录
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let lib_dir = current_dir.join("lib");

    let lib_path = lib_dir.to_str().unwrap();

    println!("cargo:rustc-link-search=native={}/", lib_path);
    println!("cargo:rustc-link-lib=dylib=sherpa-bridge");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}/", lib_path);
}