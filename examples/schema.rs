fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&herdr_idle_inhibitor::status::public_schema()).unwrap()
    );
}
