use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from("src");
    tonic_build::configure()
        .out_dir(out_dir)
        .compile(&["src/snark_proof_grpc.proto"], &["src"])
        .unwrap();
}