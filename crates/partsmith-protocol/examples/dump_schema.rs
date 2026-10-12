//! Temporary probe.
use partsmith_protocol::*;
fn main() {
    for text in [
        r#"{"type":"engine_info","extra":1}"#,
        r#"{"type":"engine_info"}"#,
        r#"{"type":"undo","doc":1,"extra":1}"#,
        r#"{"type":"new_document","printer":null}"#,
    ] {
        println!("{text} -> {:?}", serde_json::from_str::<Command>(text));
    }
    for text in [
        r#"{"kind":"request","id":1,"command":{"type":"engine_info"},"extra":2}"#,
        r#"{"kind":"request","id":1,"command":{"type":"engine_info"}}"#,
    ] {
        println!("{text} -> {:?}", serde_json::from_str::<Message>(text));
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        println!("  from value -> {:?}", <Message as serde::Deserialize>::deserialize(&value));
    }
    let r = Message::Response(Response { id: 7, result: Outcome::Ok(CommandResult::CloseDocument) });
    println!("{}", serde_json::to_string(&r).unwrap());
    let m: Result<CommandResult, _> = serde_json::from_str(r#"{"type": "measure", "volume": 24000.0, "area": 5200.0}"#);
    println!("{m:?}");
}
