use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn valid_arguments() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ockam")?;
    cmd.arg("--test-argument-parser")
        .arg("forwarder")
        .arg("create")
        .arg("n1")
        .arg("--at")
        .arg("/ip4/127.0.0.1/tcp/8080")
        .arg("--to")
        .arg("node_blue");
    cmd.assert().success();

    Ok(())
}
