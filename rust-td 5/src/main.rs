use std::time::Instant;
use rustc_hash::FxHashMap;

#[derive(Debug)]
struct TextStats {
    word_count: usize,
    char_count: usize,
    top_words: Vec<(String, usize)>,
    longest_words: Vec<String>,
    time_ms: u128,
}

fn analyze_text_fast(text: &str) -> TextStats {
    let start = Instant::now();

    let mut word_freq: FxHashMap<String, usize> =
        FxHashMap::with_capacity_and_hasher(1024, Default::default());
    let mut char_count = 0usize;
    let mut buf = String::with_capacity(32);
    for &b in text.as_bytes() {
        match b {
            b'a'..=b'z' => {
                buf.push(b as char);
                char_count += 1;
            }
            b'A'..=b'Z' => {
                buf.push((b + 32) as char); // to lowercase
                char_count += 1;
            }
            _ => {
                if !buf.is_empty() {
                    process_word(&mut buf, &mut word_freq);
                }
            }
        }
    }
    if !buf.is_empty() {
        process_word(&mut buf, &mut word_freq);
    }

    let unique_words = word_freq.len();

    // Top 10 via sort (fast for map sizes).
    let mut top_words: Vec<(String, usize)> = word_freq
        .iter()
        .map(|(w, c)| (w.clone(), *c))
        .collect();
    top_words.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if top_words.len() > 10 {
        top_words.truncate(10);
    }

    // Longest 5 words.
    let mut longest_words: Vec<(usize, String)> = word_freq
        .keys()
        .map(|w| (w.len(), w.clone()))
        .collect();
    longest_words.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if longest_words.len() > 5 {
        longest_words.truncate(5);
    }
    let longest_words: Vec<String> = longest_words.into_iter().map(|(_, w)| w).collect();

    TextStats {
        word_count: unique_words,
        char_count,
        top_words,
        longest_words,
        time_ms: start.elapsed().as_millis(),
    }
}

fn generate_test_text(size: usize) -> String {
    const WORDS: [&str; 10] = [
        "rust",
        "performance",
        "optimization",
        "memory",
        "speed",
        "efficiency",
        "benchmark",
        "algorithm",
        "data",
        "structure",
    ];

    let mut output = String::with_capacity(size * 9);
    for i in 0..size {
        if i > 0 {
            output.push(' ');
        }
        output.push_str(WORDS[i % WORDS.len()]);
    }
    output
}

fn main() {
    let text = generate_test_text(50_000);

    println!("Analyzing {} bytes of text...", text.len());

    let stats = analyze_text_fast(&text);

    println!("Results:");
    println!("  Unique words: {}", stats.word_count);
    println!("  Total alphabetic chars: {}", stats.char_count);
    println!("  Top 10 words: {:?}", stats.top_words);
    println!("  Longest words: {:?}", stats.longest_words);
    println!("  Time taken: {} ms", stats.time_ms);
}

#[inline(always)]
fn process_word(
    buf: &mut String,
    word_freq: &mut FxHashMap<String, usize>,
) {
    let word = buf.clone();
    buf.clear(); 
    word_freq
        .entry(word)
        .and_modify(|c| *c += 1)
        .or_insert(1);
}
