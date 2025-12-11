## The Slow Text Analyzer - Find & Fix Performance Bottlenecks

**Goal:** You're given intentionally slow code that analyzes text. Profile it, identify bottlenecks, and optimize it to run 10-100x faster.

### The Challenge:

```rust
use std::collections::HashMap;
use std::time::Instant;

fn analyze_text_slow(text: &str) -> TextStats {
    let start = Instant::now();

    // Count word frequencies (SLOW VERSION)
    let mut word_freq = HashMap::new();
    for line in text.lines() {
        for word in line.split_whitespace() {
            let clean_word = word.to_lowercase()
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>();

            if !clean_word.is_empty() {
                *word_freq.entry(clean_word.clone()).or_insert(0) += 1;
            }
        }
    }

    // Find most common words (SLOW VERSION)
    let mut top_words = Vec::new();
    for _ in 0..10 {
        let mut max_word = String::new();
        let mut max_count = 0;

        for (word, count) in &word_freq {
            let mut found = false;
            for (existing_word, _) in &top_words {
                if word == existing_word {
                    found = true;
                    break;
                }
            }

            if !found && *count > max_count {
                max_word = word.clone();
                max_count = *count;
            }
        }

        if max_count > 0 {
            top_words.push((max_word, max_count));
        }
    }

    // Count characters (SLOW VERSION)
    let mut char_count = 0;
    for line in text.lines() {
        for ch in line.chars() {
            if ch.is_alphabetic() {
                char_count += 1;
            }
        }
    }

    // Find longest words (SLOW VERSION)
    let mut all_words = Vec::new();
    for line in text.lines() {
        for word in line.split_whitespace() {
            let clean = word.to_lowercase()
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>();
            if !clean.is_empty() {
                all_words.push(clean);
            }
        }
    }

    all_words.sort_by(|a, b| b.len().cmp(&a.len()));
    let longest_words: Vec<String> = all_words.iter()
        .take(5)
        .map(|s| s.clone())
        .collect();

    TextStats {
        word_count: word_freq.len(),
        char_count,
        top_words,
        longest_words,
        time_ms: start.elapsed().as_millis(),
    }
}

#[derive(Debug)]
struct TextStats {
    word_count: usize,
    char_count: usize,
    top_words: Vec<(String, usize)>,
    longest_words: Vec<String>,
    time_ms: u128,
}

fn generate_test_text(size: usize) -> String {
    let words = vec!["rust", "performance", "optimization", "memory", "speed",
                     "efficiency", "benchmark", "algorithm", "data", "structure"];

    (0..size)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let text = generate_test_text(50_000);

    println!("Analyzing {} bytes of text...\n", text.len());

    let stats = analyze_text_slow(&text);

    println!("Results:");
    println!("  Unique words: {}", stats.word_count);
    println!("  Total chars: {}", stats.char_count);
    println!("  Top 10 words: {:?}", stats.top_words);
    println!("  Longest words: {:?}", stats.longest_words);
    println!("\n⏱️  Time taken: {} ms", stats.time_ms);
}
```

### Your Tasks:

1. **Profile the code** - Run it and note the baseline time
2. **Identify inefficiencies** - Find the performance killers:
   - Unnecessary allocations
   - Redundant iterations
   - Poor algorithm choices
   - Unnecessary clones
3. **Optimize step by step**:
   - Remove unnecessary `.clone()` calls
   - Use better algorithms (heap for top-K, etc.)
   - Eliminate redundant passes over data
   - Use iterators efficiently
   - Consider using `&str` instead of `String` where possible

### Success Criteria:
- **10x faster** = Good job
- **50x faster** = Excellent
- **100x+ faster** = Rust ninja

### Hints:
- You're iterating over the text multiple times unnecessarily
- There are MANY `.clone()` calls that can be eliminated
- The top words algorithm is O(n²) - can you make it O(n log n)?
- Consider using `BinaryHeap` or sorting more efficiently

This exercise teaches you real-world optimization skills: profiling, identifying bottlenecks, and applying Rust's zero-cost abstractions!
