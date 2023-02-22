use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn valid_arguments() -> Result<(), Box<dyn std::error::Error>> {
    let prefix_args = ["--test-argument-parser", "enroll"];

    // auth0
    let mut cmd = Command::cargo_bin("ockam")?;
    cmd.args(prefix_args);
    cmd.assert().success();

    Ok(())
}
