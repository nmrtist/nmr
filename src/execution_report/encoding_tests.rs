use crate::execution_report::{
    ReportError,
    json::{Encode, Json},
};

#[test]
fn serializer_rejects_nonfinite_values_and_preserves_json_string_escaping() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            value.encode(&mut Json {
                writer: &mut std::io::sink(),
                written: 0,
                limit: 100
            }),
            Err(ReportError::NonFinite)
        ));
    }
    let mut bytes = Vec::new();
    "\"\\\u{0}\n雪"
        .encode(&mut Json {
            writer: &mut bytes,
            written: 0,
            limit: 100,
        })
        .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "\"\\\"\\\\\\u0000\\n雪\""
    );
}
