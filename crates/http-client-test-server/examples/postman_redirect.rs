use std::{error::Error, io::Write as _};

use http_client_test_server::{HeaderSpec, RespondSpec, TestServer};

fn redirect_spec(status: u16, target: &str) -> RespondSpec {
    RespondSpec {
        status,
        headers: vec![HeaderSpec {
            name: "location".to_owned(),
            value: target.to_owned(),
        }],
        ..RespondSpec::default()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let source = TestServer::spawn().await?;
    let target = TestServer::spawn().await?;
    let target_url = format!("{}/v1/echo", target.base_url());
    let found_url = source.respond_url(&redirect_spec(302, &target_url))?;
    let temporary_redirect_url = source.respond_url(&redirect_spec(307, &target_url))?;

    println!("POSTMAN_REDIRECT_SOURCE_ORIGIN={}", source.base_url());
    println!("POSTMAN_REDIRECT_TARGET_ORIGIN={}", target.base_url());
    println!("POSTMAN_REDIRECT_302={found_url}");
    println!("POSTMAN_REDIRECT_307={temporary_redirect_url}");
    println!("Press Ctrl-C to stop both loopback servers.");
    std::io::stdout().flush()?;

    tokio::signal::ctrl_c().await?;
    source.shutdown().await?;
    target.shutdown().await?;
    Ok(())
}
