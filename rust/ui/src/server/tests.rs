//! Note - this module assumes a _synchronous_ server. This is not mandated by
//! the client, it's just how the current server is implemented. In the future,
//! we may want to change the server to be asynchronous, in which case we should
//! re-write or delete the tests.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::protocol::{self, Request, Response};
use component::parameters::{
    store::{SetError, SetGrabbedError},
    Value,
};

use super::{ResponseSender, Server};
struct ResponseSenderSpy<'a> {
    sent: &'a RefCell<Vec<Response>>,
    pref_updates: &'a RefCell<Vec<(String, preferences::Value)>>,
}

impl<'a> ResponseSender for ResponseSenderSpy<'a> {
    fn send(&mut self, response: Response) {
        self.sent.borrow_mut().push(response);
    }
    fn on_pref_update(&mut self, unique_id: &str, value: &preferences::Value) {
        self.pref_updates
            .borrow_mut()
            .push((unique_id.to_string(), value.clone()));
    }
}

#[derive(Clone, Default)]
struct StubStoreData {
    values: HashMap<String, component::parameters::Value>,
    grabbed: HashSet<String>,
}

impl<I: IntoIterator<Item = (String, component::parameters::Value)>> From<I> for StubStoreData {
    fn from(values: I) -> Self {
        StubStoreData {
            values: values.into_iter().collect(),
            grabbed: Default::default(),
        }
    }
}

#[derive(Clone)]
struct StubStore {
    values: Rc<RefCell<StubStoreData>>,
}

impl crate::ParameterStore for StubStore {
    fn get(&self, unique_id: &str) -> Option<Value> {
        self.values.borrow().values.get(unique_id).cloned()
    }

    fn set(&mut self, unique_id: &str, value: Value) -> Result<(), SetError> {
        self.values
            .borrow_mut()
            .values
            .insert(unique_id.to_string(), value);
        Ok(())
    }

    fn set_grabbed(&mut self, unique_id: &str, grabbed: bool) -> Result<(), SetGrabbedError> {
        if grabbed {
            self.values
                .borrow_mut()
                .grabbed
                .insert(unique_id.to_string());
        } else {
            self.values.borrow_mut().grabbed.remove(unique_id);
        }
        Ok(())
    }

    fn get_info(&self, unique_id: &str) -> Option<component::parameters::Info> {
        if unique_id == "a" {
            Some(component::parameters::Info {
                title: "Test Title".to_string(),
                short_title: "Test Short Title".to_string(),
                unique_id: "a".to_string(),
                flags: component::parameters::Flags { automatable: true },
                type_specific: component::parameters::TypeSpecificInfo::Numeric {
                    default: 1.0,
                    valid_range: 0.0..=10.0,
                    units: "Hz".to_string(),
                },
            })
        } else {
            None
        }
    }
}

#[test]
fn subscribing_to_parameter() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    server.handle_request(&Request::Subscribe {
        path: "params/a".to_string(),
    });
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::Values { values } => values
                .iter()
                .any(|(p, v)| p == "params/a" && v == &protocol::Value::Numeric(1.0)),
            _ => false,
        }
    }));
    sent.borrow_mut().clear();
    server.update_parameter("a", &2.0.into());
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::Values { values } => values
                .iter()
                .any(|(p, v)| p == "params/a" && v == &protocol::Value::Numeric(2.0)),
            _ => false,
        }
    }));

    server.handle_request(&Request::Unsubscribe {
        path: "params/a".to_string(),
    });
    sent.borrow_mut().clear();

    // Note that this behavior cannot depended on by the client - but for now
    // we try not to send updates to unsubscribed parameters.
    server.update_parameter("a", &3.0.into());
    assert!(sent.borrow().iter().all(|m| {
        !match m {
            Response::Values { values } => values
                .iter()
                .any(|(p, v)| p == "params/a" && v == &protocol::Value::Numeric(3.0)),
            _ => false,
        }
    }));
}

#[test]
fn defends_against_subscriptions_to_non_existing_paths() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    let nonsense_path = "nonsense path that does not exist".to_string();
    server.handle_request(&Request::Subscribe {
        path: nonsense_path.clone(),
    });
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::SubscribeValueError { path } => path == &nonsense_path,
            _ => false,
        }
    }));
}

#[test]
fn defends_against_subscription_to_parameter_with_invalid_path() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    server.handle_request(&Request::Subscribe {
        path: "params/b".to_string(),
    });
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::SubscribeValueError { path } => path == "params/b",
            _ => false,
        }
    }));
}

#[test]
fn set_does_not_echo_value() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    server.handle_request(&Request::Subscribe {
        path: "params/a".to_string(),
    });
    sent.borrow_mut().clear();
    server.handle_request(&Request::Set {
        path: "params/a".to_string(),
        value: protocol::Value::Numeric(2.0),
    });
    assert!(sent.borrow().is_empty());
}

#[test]
fn set_changes_store() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    server.handle_request(&Request::Subscribe {
        path: "params/a".to_string(),
    });
    sent.borrow_mut().clear();
    server.handle_request(&Request::Set {
        path: "params/a".to_string(),
        value: protocol::Value::Numeric(2.0),
    });
    assert_eq!(
        store.values.borrow().values.get("a"),
        Some(&component::parameters::Value::Numeric(2.0))
    );
}

#[test]
fn grab_basics() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    server.handle_request(&Request::Set {
        path: "params-grabbed/a".to_string(),
        value: protocol::Value::Bool(true),
    });
    assert!(store.values.borrow().grabbed.contains("a"));
    server.handle_request(&Request::Set {
        path: "params-grabbed/a".to_string(),
        value: protocol::Value::Bool(false),
    });
    assert!(!store.values.borrow().grabbed.contains("a"));
}

#[test]
fn get_info() {
    let sent = RefCell::new(Vec::new());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &RefCell::new(Default::default()),
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            Default::default(),
        ))),
        sender,
    );
    server.handle_request(&Request::Subscribe {
        path: "params-info/a".to_string(),
    });
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::Values { values } => values.iter().any(|(p, v)| {
                if p != "params-info/a" {
                    return false;
                }
                if let protocol::Value::Bytes(b) = v {
                    if let Ok(i) =
                        protocol::deserialize_from_bytes::<protocol::parameter_info::Info>(b)
                    {
                        assert_eq!(i.title, "Test Title");
                        if let protocol::parameter_info::TypeSpecific::Numeric {
                            default,
                            valid_range,
                            units,
                        } = i.type_specific
                        {
                            assert_eq!(default, 1.0);
                            assert_eq!(valid_range, (0.0, 10.0));
                            assert_eq!(units, "Hz");
                            true
                        } else {
                            panic!();
                        }
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            }),
            _ => false,
        }
    }));
}

#[test]
fn get_set_preferences() {
    let sent = RefCell::new(Vec::new());
    let pref_updates = RefCell::new(Default::default());
    let sender = ResponseSenderSpy {
        sent: &sent,
        pref_updates: &pref_updates,
    };
    let store = StubStore {
        values: Rc::new(RefCell::new([("a".to_string(), 1.0.into())].into())),
    };
    let mut server = Server::new(
        store.clone(),
        Box::new(RefCell::new(preferences::create_with_fake_os_store(
            HashMap::from_iter([("a".to_string(), preferences::Value::Switch(false))]),
        ))),
        sender,
    );
    server.handle_request(&Request::Subscribe {
        path: "prefs/a".to_string(),
    });
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::Values { values } => values.iter().any(|(p, v)| {
                if p != "prefs/a" {
                    return false;
                }
                if let protocol::Value::Bool(b) = v {
                    *b == false
                } else {
                    false
                }
            }),
            _ => false,
        }
    }));
    sent.borrow_mut().clear();
    server.handle_request(&Request::Set {
        path: "prefs/a".to_string(),
        value: protocol::Value::Bool(true),
    });
    assert_eq!(
        pref_updates.borrow().as_slice(),
        &[("a".to_string(), preferences::Value::Switch(true))]
    );
    assert!(sent.borrow().iter().any(|m| {
        match m {
            Response::Values { values } => values.iter().any(|(p, v)| {
                if p != "prefs/a" {
                    return false;
                }
                if let protocol::Value::Bool(b) = v {
                    *b == true
                } else {
                    false
                }
            }),
            _ => false,
        }
    }));
}
