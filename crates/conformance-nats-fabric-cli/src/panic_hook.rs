use crate::output::write_stderr_line;

type Hook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

pub struct PanicHookGuard {
    previous: Option<Hook>,
}

impl PanicHookGuard {
    pub fn install() -> Self {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|info| match info.location() {
            Some(at) => write_stderr_line(format_args!(
                "a check panicked at {}:{}",
                at.file(),
                at.line()
            )),
            None => write_stderr_line(format_args!("a check panicked")),
        }));
        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        if let Some(previous) = self.previous.take() {
            std::panic::set_hook(previous);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::PanicHookGuard;

    static HOOK_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn the_previous_hook_is_replaced_while_the_guard_lives_and_restored_when_it_drops() {
        let _serial = HOOK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let restored = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&restored);
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |_| seen.store(true, Ordering::SeqCst)));

        let guard = PanicHookGuard::install();
        let _ = std::panic::catch_unwind(|| panic!("while the guard lives"));
        let fired_while_installed = restored.load(Ordering::SeqCst);

        drop(guard);
        let _ = std::panic::catch_unwind(|| panic!("after the guard"));
        let fired_after_drop = restored.load(Ordering::SeqCst);

        std::panic::set_hook(original);
        assert!(!fired_while_installed);
        assert!(fired_after_drop);
    }

    #[test]
    fn a_panic_while_the_guard_lives_unwinds_without_a_double_panic() {
        let _serial = HOOK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::panic::take_hook();

        let outcome = std::panic::catch_unwind(|| {
            let _guard = PanicHookGuard::install();
            panic!("escaping while the guard lives");
        });

        std::panic::set_hook(original);
        assert!(outcome.is_err());
    }
}
