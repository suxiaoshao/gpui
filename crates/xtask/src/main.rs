mod bundle;
mod cli;
mod cmd;
mod context;
mod error;
mod manifest;

use clap::Parser;
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .event_format(fmt::format().pretty())
                .with_filter(LevelFilter::INFO),
        )
        .init();

    let cli = cli::Cli::parse();
    let result = match cli.command {
        cli::Commands::BundleAiChat => bundle::ai_chat::run(),
        cli::Commands::BundleAiChatWindows(args) => bundle::windows::run(args),
    };

    if let Err(err) = result {
        event!(Level::ERROR, "{err}");
        std::process::exit(1);
    }
}
