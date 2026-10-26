use std::error::Error;

pub(crate) fn describe(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct Layer {
        text: &'static str,
        source: Option<Box<Layer>>,
    }

    impl fmt::Display for Layer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.text)
        }
    }

    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.source.as_deref().map(|s| s as &(dyn Error + 'static))
        }
    }

    #[test]
    fn a_two_level_chain_is_joined_outermost_first() {
        let error = Layer {
            text: "calling GET /internal/passport failed",
            source: Some(Box::new(Layer {
                text: "Connection refused",
                source: None,
            })),
        };
        assert_eq!(
            describe(&error),
            "calling GET /internal/passport failed: Connection refused"
        );
    }

    #[test]
    fn an_error_without_source_is_its_own_text() {
        let error = Layer {
            text: "alone",
            source: None,
        };
        assert_eq!(describe(&error), "alone");
    }
}
