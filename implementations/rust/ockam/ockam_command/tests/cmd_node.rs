use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn valid_arguments() -> Result<(), Box<dyn std::error::Error>> {
    // show node success
    let mut cmd = Command::cargo_bin("ockam")?;
    cmd.arg("--test-argument-parser")
        .arg("node")
        .arg("show")
        .arg("node-name");
    cmd.assert().success();

    // delete node success
    let mut cmd = Command::cargo_bin("ockam")?;
    cmd.arg("--test-argument-parser")
        .arg("node")
        .arg("delete")
        .arg("node-name");
    cmd.assert().success();

    Ok(())
}
