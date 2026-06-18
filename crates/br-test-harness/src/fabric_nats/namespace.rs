use br_util_nats_fabric::KvPrefix;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RunNamespace {
    run_id: String,
}

impl RunNamespace {
    pub fn mint() -> Self {
        Self {
            run_id: Uuid::now_v7().simple().to_string(),
        }
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn durable(&self, logical: &str) -> String {
        format!("{logical}_{}", self.run_id)
    }

    pub fn key_prefix(&self) -> KvPrefix {
        KvPrefix::new(format!("{}/", self.run_id))
            .expect("a uuid-simple run id is a valid kv prefix segment")
    }

    pub fn correlation(&self) -> Uuid {
        Uuid::now_v7()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_suffixes_the_logical_name_with_the_run_id() {
        let ns = RunNamespace::mint();
        let durable = ns.durable("declare_worker");
        assert!(durable.starts_with("declare_worker_"));
        assert!(durable.ends_with(ns.run_id()));
    }

    #[test]
    fn two_runs_namespace_the_same_logical_name_apart() {
        let a = RunNamespace::mint();
        let b = RunNamespace::mint();
        assert_ne!(a.durable("worker"), b.durable("worker"));
    }

    #[test]
    fn key_prefix_is_the_run_id_with_a_trailing_slash() {
        let ns = RunNamespace::mint();
        let prefix = ns.key_prefix();
        assert_eq!(prefix.as_str(), format!("{}/", ns.run_id()));
        assert!(prefix.matches(&format!("{}/published/user/1", ns.run_id())));
        assert!(!prefix.matches("other/published/user/1"));
    }

    #[test]
    fn each_correlation_is_unique() {
        let ns = RunNamespace::mint();
        assert_ne!(ns.correlation(), ns.correlation());
    }
}
