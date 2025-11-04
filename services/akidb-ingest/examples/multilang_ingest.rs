/// Example: Multi-language vector ingestion with automatic language detection
///
/// This example demonstrates how to use AkiDB's language detection and CJK
/// tokenization features for offline RAG systems supporting multiple languages.
///
/// Usage:
///   cargo run --example multilang_ingest
///
/// Supported Languages:
///   - EN (English)
///   - FR (French)
///   - ZH (Chinese - Simplified & Traditional)
///   - ES (Spanish)
///   - JA (Japanese)
use akidb_ingest::language::{LanguageDetector, SupportedLanguage};
use std::collections::HashMap;

fn main() {
    println!("🌍 AkiDB Multi-Language Ingestion Example\n");

    let detector = LanguageDetector::new();

    // Example texts in different languages
    let examples = vec![
        (
            "English",
            "The quick brown fox jumps over the lazy dog. This is a sample English text.",
        ),
        (
            "French",
            "Le renard brun rapide saute par-dessus le chien paresseux. Ceci est un exemple de texte français.",
        ),
        (
            "Spanish",
            "El rápido zorro marrón salta sobre el perro perezoso. Este es un texto de ejemplo en español.",
        ),
        (
            "Chinese",
            "敏捷的棕色狐狸跳过懒狗。这是一个中文示例文本。人工智能和机器学习正在改变世界。",
        ),
        (
            "Japanese",
            "素早い茶色のキツネが怠け者の犬を飛び越えます。これは日本語のサンプルテキストです。",
        ),
    ];

    for (label, text) in examples {
        println!("─────────────────────────────────────────────────────");
        println!("📝 {label}:");
        println!("   Text: {text}");
        println!();

        // Detect language
        match detector.detect(text) {
            Ok(lang) => {
                println!("   ✅ Detected: {} ({})", lang.name(), lang.code());
                println!("   🔤 CJK: {}", if lang.is_cjk() { "Yes" } else { "No" });

                // Get detailed metadata
                if let Ok(metadata) = detector.detect_with_metadata(text) {
                    println!("   📊 Confidence: {:.2}%", metadata.confidence * 100.0);
                    println!("   🔢 Tokens: {}", metadata.token_count);
                }

                // Tokenize
                if let Ok(tokens) = detector.tokenize(text, lang) {
                    println!(
                        "   🔍 First 10 tokens: {:?}",
                        &tokens[..tokens.len().min(10)]
                    );
                }

                // Enrich payload with language metadata
                let mut payload = HashMap::new();
                payload.insert("content".to_string(), serde_json::json!(text));

                if let Ok(enriched) = detector.enrich_payload(text, payload) {
                    println!("   📦 Enriched Payload:");
                    for (key, value) in enriched.iter() {
                        println!("      - {}: {}", key, value);
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Detection failed: {}", e);
            }
        }

        println!();
    }

    // Example: Batch processing with language filtering
    println!("─────────────────────────────────────────────────────");
    println!("📚 Batch Processing Example:");
    println!();

    let documents = vec![
        "This is an English document about AI.",
        "Ceci est un document français sur l'IA.",
        "これはAIに関する日本語の文書です。",
        "Este es un documento español sobre IA.",
        "这是一份关于人工智能的中文文件。",
    ];

    let mut language_counts: HashMap<String, usize> = HashMap::new();

    for doc in &documents {
        if let Ok(lang) = detector.detect(doc) {
            *language_counts.entry(lang.code().to_string()).or_insert(0) += 1;
        }
    }

    println!("   📊 Language Distribution:");
    for (lang, count) in language_counts {
        println!("      {}: {} documents", lang, count);
    }

    println!();
    println!("✅ Multi-language ingestion example complete!");
    println!();
    println!("💡 Integration Tips:");
    println!("   1. Use LanguageDetector::detect() for basic language detection");
    println!("   2. Use detect_with_metadata() for detailed analysis");
    println!("   3. Use enrich_payload() to add language metadata to vectors");
    println!("   4. Filter by language in queries using 'language' field");
    println!("   5. For CJK languages, consider integrating:");
    println!("      - Chinese: jieba-rs for better word segmentation");
    println!("      - Japanese: lindera for morphological analysis");
}
