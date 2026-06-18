use sts_verify::{corpus_path, load_corpus_file, ManualFixture};

#[test]
fn manual_milestone1_corpus_loads_if_present() {
    let path = corpus_path("manual/milestone1.jsonl");
    if !path.exists() {
        return;
    }

    let content = load_corpus_file("manual/milestone1.jsonl").expect("corpus file readable");
    let fixture: ManualFixture =
        serde_json::from_str(content.trim()).expect("manual fixture parses");

    assert_eq!(fixture.name, "milestone1_manual_win");
    assert_eq!(fixture.rng_draws, 0);
    assert_eq!(fixture.actions.len(), 2);
}
