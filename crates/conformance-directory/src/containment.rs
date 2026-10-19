use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;

use futures_util::FutureExt;

pub(crate) async fn contained<T>(
    future: impl Future<Output = T>,
) -> std::result::Result<T, String> {
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(panic_message)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "the harness panicked with a non-string payload".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_panic_in_the_harness_is_contained_with_its_message() {
        let formatted = contained(async { panic!("harness exploded: {}", 7) }).await;
        assert_eq!(formatted.unwrap_err(), "harness exploded: 7");
        let literal = contained(async { panic!("plain literal") }).await;
        assert_eq!(literal.unwrap_err(), "plain literal");
    }

    #[tokio::test]
    async fn a_value_passes_through_containment() {
        assert_eq!(contained(async { 41 + 1 }).await.unwrap(), 42);
    }
}
