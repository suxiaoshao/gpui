use std::path::PathBuf;

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
    VerifyGstreamer(VerifyGstreamerArgs),
    VerifyGstreamerSdk(VerifyGstreamerSdkArgs),
}

#[derive(Args)]
pub struct BundleArgs {
    pub app: BundleApp,
    #[arg(short = 'i', long)]
    pub install: bool,
}

#[derive(Args)]
pub struct VerifyGstreamerArgs {
    /// The app-local, allow-listed native runtime manifest to validate.
    #[arg(long)]
    pub manifest: PathBuf,
    /// Also execute `gst-inspect-1.0` for every declared required element.
    #[arg(long)]
    pub inspect: bool,
}

#[derive(Args)]
pub struct VerifyGstreamerSdkArgs {
    /// Minimum GStreamer SDK version required by the Rust bindings.
    #[arg(long)]
    pub minimum_version: String,
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

        let Commands::Bundle(args) = cli.command else {
            panic!("expected bundle command");
        };
        assert_eq!(args.app, BundleApp::HttpClient);
        assert!(!args.install);
    }

    #[test]
    fn parses_bundle_install_flag() {
        let cli = Cli::try_parse_from(["xtask", "bundle", "jaco", "--install"])
            .expect("bundle command should parse");

        let Commands::Bundle(args) = cli.command else {
            panic!("expected bundle command");
        };
        assert_eq!(args.app, BundleApp::Jaco);
        assert!(args.install);
    }

    #[test]
    fn parses_jaco_bundle_app_argument() {
        let cli =
            Cli::try_parse_from(["xtask", "bundle", "jaco"]).expect("bundle command should parse");

        let Commands::Bundle(args) = cli.command else {
            panic!("expected bundle command");
        };
        assert_eq!(args.app, BundleApp::Jaco);
        assert_eq!(args.app.package_name(), "jaco");
        assert_eq!(args.app.app_dir_name(), "jaco");
    }

    #[test]
    fn parses_gstreamer_verification_arguments() {
        let cli = Cli::try_parse_from([
            "xtask",
            "verify-gstreamer",
            "--manifest",
            "app/http-client/build-assets/gstreamer/runtime-manifest.toml",
            "--inspect",
        ])
        .expect("GStreamer verification command should parse");

        let Commands::VerifyGstreamer(args) = cli.command else {
            panic!("expected GStreamer verification command");
        };
        assert_eq!(
            args.manifest,
            std::path::PathBuf::from(
                "app/http-client/build-assets/gstreamer/runtime-manifest.toml"
            )
        );
        assert!(args.inspect);
    }

    #[test]
    fn parses_gstreamer_sdk_verification_arguments() {
        let cli =
            Cli::try_parse_from(["xtask", "verify-gstreamer-sdk", "--minimum-version", "1.20"])
                .expect("GStreamer SDK verification command should parse");

        let Commands::VerifyGstreamerSdk(args) = cli.command else {
            panic!("expected GStreamer SDK verification command");
        };
        assert_eq!(args.minimum_version, "1.20");
    }
}
