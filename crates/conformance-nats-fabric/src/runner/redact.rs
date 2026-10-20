const REDACTED: &str = "[redacted]";

pub fn redact_nats_url(urls: &str) -> String {
    chunks(urls)
        .into_iter()
        .map(|chunk| {
            let (scheme, body) = split_scheme(chunk);
            let host = body.rsplit_once('@').map_or(body, |(_, host)| host);
            format!("{scheme}{host}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn redact_in(text: &str, urls: &str) -> String {
    let mut redacted = text.replace(urls, &redact_nats_url(urls));
    for chunk in chunks(urls) {
        redacted = redacted.replace(chunk, &redact_nats_url(chunk));
    }
    let mut secrets = secrets_of(urls);
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    for secret in secrets {
        redacted = redacted.replace(&secret, REDACTED);
    }
    redacted
}

fn secrets_of(urls: &str) -> Vec<String> {
    let mut secrets = Vec::new();
    for chunk in chunks(urls) {
        let (_, body) = split_scheme(chunk);
        let Some((userinfo, _)) = body.rsplit_once('@') else {
            continue;
        };
        let password = userinfo.split_once(':').map(|(_, password)| password);
        for literal in std::iter::once(userinfo).chain(password) {
            for form in [literal.to_string(), percent_decoded(literal)] {
                if !form.is_empty() && !secrets.contains(&form) {
                    secrets.push(form);
                }
            }
        }
    }
    secrets
}

fn chunks(urls: &str) -> Vec<&str> {
    let mut scheme_chunks = Vec::new();
    let mut start = 0;
    for (index, _) in urls.match_indices(',') {
        if starts_with_scheme(&urls[index + 1..]) {
            scheme_chunks.push(&urls[start..index]);
            start = index + 1;
        }
    }
    scheme_chunks.push(&urls[start..]);
    scheme_chunks
        .into_iter()
        .flat_map(|chunk| {
            if starts_with_scheme(chunk) || chunk.contains('@') {
                vec![chunk]
            } else {
                chunk.split(',').collect::<Vec<_>>()
            }
        })
        .collect()
}

fn starts_with_scheme(text: &str) -> bool {
    scheme_len(text).is_some()
}

fn scheme_len(text: &str) -> Option<usize> {
    let end = text.find("://")?;
    let scheme = &text[..end];
    let mut chars = scheme.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some(end + "://".len())
}

fn split_scheme(chunk: &str) -> (&str, &str) {
    chunk.split_at(scheme_len(chunk).unwrap_or(0))
}

fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let pair = bytes
            .get(index + 1..index + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[index], pair) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn userinfo_is_removed_from_every_url_of_the_list() {
        assert_eq!(
            redact_nats_url("nats://user:pa@ss@host:4222,tls://token@other:4223/x"),
            "nats://host:4222,tls://other:4223/x"
        );
    }

    #[test]
    fn a_url_without_userinfo_is_untouched() {
        assert_eq!(
            redact_nats_url("nats://127.0.0.1:4222"),
            "nats://127.0.0.1:4222"
        );
        assert_eq!(redact_nats_url("127.0.0.1:4222"), "127.0.0.1:4222");
        assert_eq!(
            redact_nats_url("nats://h1:4222,tls://h2:4223"),
            "nats://h1:4222,tls://h2:4223"
        );
    }

    #[test]
    fn a_password_holding_a_delimiter_cannot_end_the_userinfo_early() {
        for password in ["Zk9/tail", "pa?ss", "pa#ss", "pa,ss", "a/b?c#d,e@f"] {
            let url = format!("nats://operator:{password}@127.0.0.1:4222");
            let shown = redact_nats_url(&url);
            assert_eq!(shown, "nats://127.0.0.1:4222", "{password}");
        }
    }

    #[test]
    fn a_delimiter_in_the_password_of_one_url_leaves_the_other_urls_readable() {
        assert_eq!(
            redact_nats_url("nats://u:a/b,c@h1:4222,tls://t@h2:4223"),
            "nats://h1:4222,tls://h2:4223"
        );
    }

    #[test]
    fn a_schemeless_entry_with_a_delimiter_in_the_password_leaks_nothing() {
        let shown = redact_nats_url("u:pa,ss/tail@h:4222");
        assert_eq!(shown, "h:4222");
    }

    #[test]
    fn a_message_quoting_the_url_no_longer_carries_the_secret() {
        let message = "failed to connect to NATS at nats://u:secret@h:4222: refused";
        let shown = redact_in(message, "nats://u:secret@h:4222");
        assert!(!shown.contains("secret"), "{shown}");
        assert!(shown.contains("nats://h:4222"), "{shown}");
    }

    #[test]
    fn a_message_quoting_only_the_userinfo_or_the_password_is_scrubbed() {
        let urls = "nats://operator:Zk9/hunter2tail@127.0.0.1:1";
        for message in [
            "bad credentials operator:Zk9/hunter2tail".to_string(),
            "bad password Zk9/hunter2tail".to_string(),
        ] {
            let shown = redact_in(&message, urls);
            assert!(!shown.contains("hunter2tail"), "{shown}");
            assert!(!shown.contains("Zk9"), "{shown}");
        }
    }

    #[test]
    fn a_message_quoting_the_decoded_password_of_an_encoded_url_is_scrubbed() {
        let shown = redact_in(
            "rejected password Zk9/hunter2tail",
            "nats://operator:Zk9%2Fhunter2tail@127.0.0.1:1",
        );
        assert!(!shown.contains("hunter2tail"), "{shown}");
    }

    #[test]
    fn a_message_quoting_a_mangled_url_is_scrubbed_through_its_userinfo() {
        let urls = "nats://operator:hunter2@127.0.0.1:notaport";
        let message = format!("failed to connect to NATS at {urls} for the fabric harness: bad");
        let shown = redact_in(&message, urls);
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(!shown.contains("operator"), "{shown}");
    }
}
