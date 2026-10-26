use async_nats::jetstream::ErrorCode;
use async_nats::jetstream::context::{
    DeleteStreamError, DeleteStreamErrorKind, GetStreamError, GetStreamErrorKind, KeyValueError,
};

pub(crate) fn stream_not_found(err: &GetStreamError) -> bool {
    matches!(err.kind(), GetStreamErrorKind::JetStream(inner) if inner.error_code() == ErrorCode::STREAM_NOT_FOUND)
}

pub(crate) fn delete_stream_not_found(err: &DeleteStreamError) -> bool {
    matches!(err.kind(), DeleteStreamErrorKind::JetStream(inner) if inner.error_code() == ErrorCode::STREAM_NOT_FOUND)
}

pub(crate) fn bucket_not_found(err: &KeyValueError) -> bool {
    backing_stream_answer(err, |cause| {
        cause
            .downcast_ref::<GetStreamError>()
            .map(stream_not_found)
            .or_else(|| {
                cause
                    .downcast_ref::<DeleteStreamError>()
                    .map(delete_stream_not_found)
            })
    })
}

fn backing_stream_answer(
    err: &KeyValueError,
    answer: impl Fn(&(dyn std::error::Error + 'static)) -> Option<bool>,
) -> bool {
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        if let Some(verdict) = answer(cause) {
            return verdict;
        }
        source = cause.source();
    }
    false
}
