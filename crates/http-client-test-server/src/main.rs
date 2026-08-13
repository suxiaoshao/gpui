use std::{env, process::ExitCode};

const USAGE: &str = "Usage: http-client-test-server [--port <u16>]";

fn parse_port<I>(mut arguments: I) -> Result<u16, ()>
where
    I: Iterator<Item = String>,
{
    match (arguments.next(), arguments.next(), arguments.next()) {
        (None, None, None) => Ok(0),
        (Some(flag), Some(value), None) if flag == "--port" => value.parse().map_err(|_| ()),
        _ => Err(()),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let port = match parse_port(env::args().skip(1)) {
        Ok(port) => port,
        Err(()) => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match http_client_test_server::run_cli(port).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_port;

    #[test]
    fn parses_only_the_frozen_cli_shape() {
        assert_eq!(parse_port([].into_iter()), Ok(0));
        assert_eq!(
            parse_port(["--port".to_owned(), "8080".to_owned()].into_iter()),
            Ok(8080)
        );
        assert_eq!(parse_port(["--port".to_owned()].into_iter()), Err(()));
        assert_eq!(
            parse_port(["--host".to_owned(), "127.0.0.1".to_owned()].into_iter()),
            Err(())
        );
    }
}
