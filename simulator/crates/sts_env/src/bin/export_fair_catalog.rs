fn main() {
    let catalog = sts_env::fair_content_catalog();
    println!(
        "{}",
        serde_json::to_string_pretty(&catalog).expect("fair content catalog is serializable")
    );
}
