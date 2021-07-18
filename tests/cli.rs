use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::{self, Write};
use std::process::Command;
use tempfile::NamedTempFile;

macro_rules! cli_test_success {
    ($($name:ident: $value:expr,)*) => {
        $(
            #[test]
            fn $name() -> Result<(), Box<dyn std::error::Error>> {
                let (input, pattern, expected) = $value;
                let mut file = NamedTempFile::new()?;
                writeln!(file, "{}", input)?;
                let mut cmd = Command::cargo_bin("grrs")?;
                cmd.arg(pattern).arg(file.path());
                cmd.assert()
                    .success()
                    .stdout(predicate::str::contains(expected));
                Ok(())
            }
        )*
    }
}

cli_test_success! {
    simple: ("A test\nActual content\nMore content\nAnother test", "test", "A test\nAnother test"),
    empty_pattern: ("A test\nActual content\nMore content\nAnother test", "", ""),
    empty_input: ("", "test", ""),
}

#[test]
fn file_doesnt_exist() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("grrs")?;
    cmd.arg("foobar").arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
    Ok(())
}

#[test]
fn cli_args_insufficient() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("grrs")?;
    cmd.arg("foobar");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("The following required arguments were not provided"));
    Ok(())
}
