use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "xtask")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Bundle(BundleArgs),
}

#[derive(Args)]
pub struct BundleArgs {
    pub app: BundleApp,
    #[arg(short = 'i', long)]
    pub install: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum BundleApp {
    Jaco,
    Feiwen,
    HttpClient,
    NovelDownload,
}

impl BundleApp {
    pub fn package_name(self) -> &'static str {
        match self {
            Self::Jaco => "jaco",
            Self::Feiwen => "feiwen",
            Self::HttpClient => "http-client",
            Self::NovelDownload => "novel-download",
        }
    }

    pub fn app_dir_name(self) -> &'static str {
        self.package_name()
    }
}

#[cfg(test)]
mod tests {
    use super::{BundleApp, Cli, Commands};
    use clap::Parser;

    #[test]
    fn parses_bundle_app_argument() {
        let cli = Cli::try_parse_from(["xtask", "bundle", "http-client"])
            .expect("bundle command should parse");

        let Commands::Bundle(args) = cli.command;
        assert_eq!(args.app, BundleApp::HttpClient);
        assert!(!args.install);
    }

    #[test]
    fn parses_bundle_install_flag() {
        let cli = Cli::try_parse_from(["xtask", "bundle", "jaco", "--install"])
            .expect("bundle command should parse");

        let Commands::Bundle(args) = cli.command;
        assert_eq!(args.app, BundleApp::Jaco);
        assert!(args.install);
    }

    #[test]
    fn parses_jaco_bundle_app_argument() {
        let cli =
            Cli::try_parse_from(["xtask", "bundle", "jaco"]).expect("bundle command should parse");

        let Commands::Bundle(args) = cli.command;
        assert_eq!(args.app, BundleApp::Jaco);
        assert_eq!(args.app.package_name(), "jaco");
        assert_eq!(args.app.app_dir_name(), "jaco");
    }
}
