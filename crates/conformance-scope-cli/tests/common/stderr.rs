pub fn stderr_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr")
}
