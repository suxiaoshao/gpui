use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    BundleAiChat,
    BundleAiChatWindows(BundleAiChatWindowsArgs),
}

#[derive(Args)]
pub struct BundleAiChatWindowsArgs {
    #[arg(short = 'i', long)]
    pub install: bool,
    #[arg(short = 'a', long, alias = "architecture")]
    pub arch: Option<String>,
    #[arg(short = 't', long)]
    pub target: Option<String>,
}
