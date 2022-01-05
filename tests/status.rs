use windowPost_snark_server::status::{ServerStatus,TaskStatus};

#[test]
fn test_enum() {
    assert_eq!(
        String::from("Unknown"),
        (ServerStatus::Unknown).to_string().as_ref()
    );
    assert_eq!(
        String::from("Free"),
        (ServerStatus::Free.clone()).to_string().as_ref()
    );
    println!("{:?}", ServerStatus::Working);
    println!("{}", ServerStatus::default().to_string());
    println!("{}", TaskStatus::default().to_string())
}