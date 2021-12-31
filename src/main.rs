use clap::{crate_authors, crate_version};

fn main() {
    println!("{}",crate_authors!());
    println!("{}",crate_version!());
}
