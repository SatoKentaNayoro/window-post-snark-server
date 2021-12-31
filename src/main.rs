mod utils;
use clap::{crate_version, App, Arg};
use std::env;

fn main() {
    utils::set_commit_env();
    let cmds = App::new("window-post-server")
        .author(utils::author())
        .version(utils::version())
        .subcommands(vec![run_cmd(), stop_cmd()]);
    let cmd_matched = cmds.get_matches();
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
