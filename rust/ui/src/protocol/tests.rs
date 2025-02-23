use super::{decode_message, encode_message, Request, Response, Value};

#[test]
fn decode_defends_against_non_b64_chars() {
    assert!(decode_message::<Request>("***").is_err());
}

#[test]
fn request_round_trip() {
    let request = Request::Subscribe {
        path: "foo".to_string(),
    };
    let decoded = decode_message::<Request>(&encode_message(&request)).unwrap();
    assert_eq!(request, decoded);
}

#[test]
fn request_set_bytes_round_trip() {
    let request = Request::Set {
        path: "foo".to_string(),
        value: Value::Bytes(vec![1, 2, 3]),
    };
    let decoded = decode_message::<Request>(&encode_message(&request)).unwrap();
    assert_eq!(request, decoded);
}

#[test]
fn request_set_string_round_trip() {
    let request = Request::Set {
        path: "foo".to_string(),
        value: Value::String("bar".to_string()),
    };
    let decoded = decode_message::<Request>(&encode_message(&request)).unwrap();
    assert_eq!(request, decoded);
}

#[test]
fn response_round_trip() {
    let response = Response::Values {
        values: [("foo".to_string(), Value::Numeric(1.0))].into(),
    };
    let decoded = decode_message::<super::Response>(&encode_message(&response)).unwrap();
    assert_eq!(response, decoded);
}
