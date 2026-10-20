use tokio::task::AbortHandle;

pub(crate) struct AbortOnDrop(AbortHandle);

impl AbortOnDrop {
    pub(crate) fn new(handle: AbortHandle) -> Self {
        Self(handle)
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
