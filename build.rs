fn main() {
    println!("cargo::rerun-if-changed=proto/stt.proto");
    tonic_build::configure()
        .compile(&["proto/stt.proto"], &[".", "googleapis"])
        .unwrap();
}
