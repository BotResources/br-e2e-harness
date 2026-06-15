pub(super) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(super) fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::{quote_ident, quote_literal};

    #[test]
    fn plain_identifier_is_double_quoted() {
        assert_eq!(quote_ident("identity_app"), "\"identity_app\"");
    }

    #[test]
    fn embedded_double_quote_is_doubled() {
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn injection_attempt_in_identifier_stays_inside_the_quotes() {
        assert_eq!(
            quote_ident("x\" WITH SUPERUSER --"),
            "\"x\"\" WITH SUPERUSER --\""
        );
    }

    #[test]
    fn plain_literal_is_single_quoted() {
        assert_eq!(quote_literal("app_pw"), "'app_pw'");
    }

    #[test]
    fn embedded_single_quote_is_doubled() {
        assert_eq!(quote_literal("a'b"), "'a''b'");
    }
}
