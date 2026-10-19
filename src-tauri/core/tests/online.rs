use std::time::Instant;
use tolk_core::text::Dir;
use tolk_core::translate::Translator;

#[test]
#[ignore]
fn online_translates_word_and_phrase() {
    let t = Translator::default();
    let start = Instant::now();
    let w = t.translate("замок", Dir::RuEn, false).unwrap();
    println!("слово {:?} за {:?}", w.variants, start.elapsed());
    assert!(w.variants.iter().any(|v| v == "castle"));
    let start = Instant::now();
    let p = t.translate("Good morning, friends", Dir::EnRu, false).unwrap();
    println!("фраза {:?} за {:?}", p.text, start.elapsed());
    assert!(p.text.contains("утро"));
    let start = Instant::now();
    assert_eq!(t.translate("замок", Dir::RuEn, false).unwrap(), w);
    assert!(start.elapsed().as_millis() < 5);
}
