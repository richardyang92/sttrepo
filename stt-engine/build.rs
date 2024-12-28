fn main() {
    println!("cargo:rustc-link-search=native=/Users/yangyang/Projects/sttrepo/stt-engine/lib/");
    println!("cargo:rustc-link-lib=dylib=sherpa-bridge");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/Users/yangyang/Projects/sttrepo/stt-engine/lib/");
}