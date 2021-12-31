mod utils;

use clap::{crate_version, App, Arg};
use std::env;
use std::process::exit;

fn main() {
    utils::set_commit_env();
    let cmds = App::new("window-post-server")
        .author(utils::author())
        .version(utils::version())
        .subcommands(vec![run_cmd(), stop_cmd()]);
    let mut c = cmds.clone();
    let matches = cmds.get_matches();
    match matches.subcommand_name() {
        Some("run") => {
            env::set_var("RUST_BACKTRACE", "full");
            let run_matched = matches.subcommand_matches("run").unwrap();
            if run_matched.is_present("debug") {
                env::set_var("RUST_LOG", "debug");
            } else {
                env::set_var("RUST_LOG", "info");
            }

            fil_logger::init();
            let port = run_matched.value_of("port").unwrap().to_string();
            if run_matched.is_present("force") {
                run(port, true)
            } else {
                run(port, false)
            }
        }
        Some("stop") => {
            let stop_matched = cmd_matches.subcommand_matches("stop").unwrap();
            let pid = stop_matched.value_of("pid").unwrap().to_string();
            stop(pid)
        }
        _ => {
            c.print_help().unwrap();
            exit(1)
        }
    };
}

fn run_cmd() -> App<'static, 'static> {
    App::new("run").about("run window-post-server").args(&[
        Arg::from_usage("-d, --debug 'print debug log'").required(false),
        Arg::from_usage("-f, --force 'force run process without num limit'").required(false),
        Arg::from_usage("-p, --port=[PORT] 'specify server port'")
            .default_value("50051")
            .required(false),
    ])
}

fn stop_cmd() -> App<'static, 'static> {
    App::new("stop").about("stop window-post-server").arg(
        Arg::from_usage("-p, --pid=[PID] 'specify server pid'")
            .default_value("")
            .required(false),
    )
}


#[tokio::main]
async fn run(port: String,is_force: bool) {

}

fn stop(p: String) {

}