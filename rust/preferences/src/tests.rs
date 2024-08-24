use super::*;
const KEY: &str = "test";

// Don't run these tests under miri
#[cfg(not(miri))]
mod os_store {
    use super::*;
    #[test]
    fn starts_empty() {
        let mut store = create_os_store("com.p61.test.starts_empty");
        store.reset();
        assert_eq!(store.get(KEY), None);
    }

    #[test]
    fn can_set() {
        let mut store = create_os_store("com.p61.test.can_set");
        store.reset();
        assert_eq!(store.get(KEY), None);
        store.set(KEY, Value::Switch(true));
        assert_eq!(store.get(KEY), Some(Value::Switch(true)));
        store.set(KEY, Value::Switch(false));
        assert_eq!(store.get(KEY), Some(Value::Switch(false)));
    }

    #[test]
    fn domains_dont_conflict() {
        let mut store1 = create_os_store("com.p61.test.domains_dont_conflict1");
        let mut store2 = create_os_store("com.p61.test.domains_dont_conflict2");
        store1.reset();
        store2.reset();
        store1.set(KEY, Value::Switch(true));
        assert_eq!(store1.get(KEY), Some(Value::Switch(true)));
        assert_eq!(store2.get(KEY), None);
    }

    #[test]
    fn can_reset() {
        let mut store = create_os_store("com.p61.test.can_reset");
        store.reset();
        store.set(KEY, Value::Switch(true));
        assert_eq!(store.get(KEY), Some(Value::Switch(true)));
        store.reset();
        assert_eq!(store.get(KEY), None);
    }

    #[test]
    fn persists() {
        {
            let mut store = create_os_store("com.p61.test.persists");
            store.reset();
            store.set(KEY, Value::Switch(true));
        }

        let store = create_os_store("com.p61.test.persists");
        assert_eq!(store.get(KEY), Some(Value::Switch(true)));
    }
}

#[test]
fn cannot_set_unknown_key() {
    let mut store = create_with_fake_os_store(HashMap::new());
    assert_eq!(
        store.set(KEY, Value::Switch(true)),
        Err(StoreError::UnknownKey)
    );
}

#[test]
fn cannot_get_unknown_key() {
    let store = create_with_fake_os_store(HashMap::new());
    assert_eq!(store.get(KEY), Err(StoreError::UnknownKey));
}

#[test]
fn get_key_returns_default() {
    let store =
        create_with_fake_os_store(HashMap::from_iter([(KEY.to_string(), Value::Switch(true))]));
    assert_eq!(store.get(KEY), Ok(Value::Switch(true)));
}

#[test]
fn can_set_key() {
    let mut store = create_with_fake_os_store(HashMap::from_iter([(
        KEY.to_string(),
        Value::Switch(false),
    )]));
    assert_eq!(store.set(KEY, Value::Switch(true)), Ok(()));
    assert_eq!(store.get(KEY), Ok(Value::Switch(true)));
}
