use assert_cmd::Command;

pub fn cli() -> Command {
    Command::cargo_bin("conformance-scope").expect("cargo bin")
}
