use windows_registry::CURRENT_USER;

const BASE_PATH: &str = "Software";

struct Store {
    domain: String,
}

impl Store {
    fn registry_path(&self) -> String {
        format!("{BASE_PATH}\\{}", self.domain)
    }
}

impl super::OSStore for Store {
    fn get(&self, unique_id: &str) -> Option<super::Value> {
        let key = CURRENT_USER.open(self.registry_path()).ok()?;
        let s = key.get_string(unique_id).ok()?;
        serde_json::from_str(&s).ok()
    }

    fn set(&mut self, unique_id: &str, value: super::Value) {
        if let Ok(s) = serde_json::to_string(&value)
            && let Ok(key) = CURRENT_USER.create(self.registry_path())
        {
            let _ = key.set_string(unique_id, &s);
        }
    }

    #[cfg(all(test, not(miri)))]
    fn reset(&mut self) {
        let _ = CURRENT_USER.remove_tree(self.registry_path());
    }
}

pub fn create_os_store(domain: &str) -> impl super::OSStore + use<> {
    Store {
        domain: domain.to_string(),
    }
}
