mod opt;

use clap::Parser;
use colored::Colorize;
use common_lang_types::CurrentWorkingDirectory;
use graphql_network_protocol::GraphQLNetworkProtocol;
use intern::string_key::Intern;
use isograph_compiler::{compile_and_print, handle_watch_command};
use opt::{Command, CompileCommand, LspCommand, Opt};
use std::{io, iter};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    EnvFilter,
};

#[tokio::main]
async fn main() {
    let opt = Opt::parse();
    let command = opt.command.unwrap_or(Command::Compile(opt.compile));

    match command {
        Command::Compile(compile_command) => {
            start_compiler(compile_command, current_working_directory()).await;
        }
        Command::Lsp(lsp_command) => {
            start_language_server(lsp_command, current_working_directory()).await;
        }
    }
}

async fn start_compiler(
    compile_command: CompileCommand,
    current_working_directory: CurrentWorkingDirectory,
) {
    configure_logger(compile_command.log_level, &compile_command.log_target);
    let config_location = compile_command
        .config
        .unwrap_or("./isograph.config.json".into());

    if compile_command.watch {
        match handle_watch_command::<GraphQLNetworkProtocol>(
            &config_location,
            current_working_directory,
        )
        .await
        {
            Ok(_) => {
                info!("{}", "Successfully watched. Exiting.\n")
            }
            Err(err) => {
                error!("{}\n{:?}", "Error in watch process of some sort.\n", err);
                std::process::exit(1);
            }
        };
    } else if compile_and_print::<GraphQLNetworkProtocol>(
        &config_location,
        current_working_directory,
    )
    .is_err()
    {
        std::process::exit(1);
    }
}

async fn start_language_server(
    lsp_command: LspCommand,
    current_working_directory: CurrentWorkingDirectory,
) {
    let config_location = lsp_command
        .config
        .unwrap_or("./isograph.config.json".into());
    info!("Starting language server");
    if let Err(_e) = isograph_lsp::start_language_server::<GraphQLNetworkProtocol>(
        &config_location,
        current_working_directory,
    )
    .await
    {
        error!(
            "{}",
            "Error encountered when running language server.".bright_red(),
            // TODO derive Error and print e
        );
        std::process::exit(1);
    }
}

fn configure_logger(global_level: LevelFilter, target_overrides: &[String]) {
    let filter = iter::once(global_level.to_string())
        .chain(target_overrides.iter().cloned())
        .collect::<Vec<_>>()
        .join(",");
    let env_filter = EnvFilter::new(filter);
    let fmt_layer = fmt::layer()
        .pretty()
        .compact()
        .without_time()
        .with_writer(io::stderr);
    let fmt_layer = match global_level {
        LevelFilter::DEBUG | LevelFilter::TRACE => fmt_layer.with_span_events(FmtSpan::FULL),
        _ => fmt_layer
            .with_file(false)
            .with_line_number(false)
            .with_target(false),
    };
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

fn current_working_directory() -> CurrentWorkingDirectory {
    std::env::current_dir()
        .expect("Expected current working to exist")
        .to_str()
        .expect("Expected current working directory to be able to be stringified.")
        .intern()
        .into()
}
