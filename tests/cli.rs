use assert_cmd::Command;
use predicates::prelude::*;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

const PRG: &str = "openapi_rbs_genarator";

#[test]
fn usage() -> TestResult {
    for flag in &["-h", "--help"] {
        Command::cargo_bin(PRG)?
            .arg(flag)
            .assert()
            .stdout(predicate::str::contains("Usage"));
    }
    Ok(())
}

#[test]
fn gen_petstore_rbs() -> TestResult {
    let generated_path = "tests/outputs/petstore.rbs";
    let expected_path = "tests/expected/petstore.rbs";
    Command::cargo_bin(PRG)?
        .arg("-i")
        .arg("tests/inputs/petstore.json")
        .arg("-o")
        .arg(generated_path)
        .assert()
        .success();
    // check if the generated file is equal to the expected file
    assert_cmd::Command::new("diff")
        .arg(generated_path)
        .arg(expected_path)
        .assert()
        .success();
    Ok(())
}
