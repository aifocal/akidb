# Multi-Language Support in AkiDB

AkiDB supports automatic language detection and CJK (Chinese, Japanese, Korean) tokenization for offline RAG systems.

## Supported Languages

| Language | Code | Script | Tokenization |
|----------|------|--------|--------------|
| English  | `en` | Latin  | Word-based   |
| French   | `fr` | Latin  | Word-based   |
| Spanish  | `es` | Latin  | Word-based   |
| Chinese  | `zh` | CJK    | Character-based |
| Japanese | `ja` | CJK    | Character-based |

## Features

### 1. Automatic Language Detection

AkiDB uses [whatlang](https://github.com/greyblake/whatlang-rs) for fast, accurate language detection:

```rust
use akidb_ingest::language::LanguageDetector;

let detector = LanguageDetector::new();
let lang = detector.detect("This is English text")?;
assert_eq!(lang, SupportedLanguage::EN);
```

### 2. Language Metadata Enrichment

Automatically add language metadata to vector payloads:

```rust
let mut payload = HashMap::new();
payload.insert("content".to_string(), serde_json::json!(text));

let enriched = detector.enrich_payload(text, payload)?;
// Adds: language, language_name, language_confidence, is_cjk, token_count
```

### 3. CJK Tokenization

AkiDB provides basic character-based tokenization for CJK languages:

```rust
let tokens = detector.tokenize("你好世界", SupportedLanguage::ZH)?;
// Returns: ["你", "好", "世", "界"]
```

**Note**: For production CJK deployments, we recommend integrating:
- **Chinese**: [jieba-rs](https://github.com/messense/jieba-rs) for word segmentation
- **Japanese**: [lindera](https://github.com/lindera-morphology/lindera) for morphological analysis

## Usage in Ingestion Pipeline

### Basic Usage

```bash
# Ingest multilingual documents with automatic language detection
akidb-ingest \
  --collection my-docs \
  --file documents.csv \
  --id-column doc_id \
  --vector-column embedding \
  --payload-columns text,title,author
```

Language metadata will be automatically added to each document:
- `language`: ISO 639-1 code (e.g., "en", "zh")
- `language_name`: Human-readable name (e.g., "English", "Chinese")
- `language_confidence`: Detection confidence (0.0 - 1.0)
- `is_cjk`: Boolean indicating CJK script
- `token_count`: Number of tokens after tokenization

### Filtering by Language

Query vectors by language using the `language` field:

```bash
curl -X POST http://localhost:8080/collections/my-docs/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, ...],
    "k": 10,
    "filter": {
      "language": "zh"
    }
  }'
```

### Language-Specific Search

```bash
# Search only in English documents
curl -X POST http://localhost:8080/collections/my-docs/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, ...],
    "k": 10,
    "filter": {
      "language": "en"
    }
  }'

# Search in CJK documents (Chinese or Japanese)
curl -X POST http://localhost:8080/collections/my-docs/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, ...],
    "k": 10,
    "filter": {
      "is_cjk": true
    }
  }'
```

## Configuration

### Confidence Threshold

Control language detection confidence:

```rust
// Default: 70% confidence threshold
let detector = LanguageDetector::new();

// Custom threshold: 85%
let detector = LanguageDetector::with_confidence(0.85);
```

### Environment Variables

```bash
# Set logging level for language detection debugging
RUST_LOG=akidb_ingest::language=debug akidb-ingest ...
```

## Advanced: Custom CJK Tokenization

For production CJK workloads, integrate advanced tokenizers:

### Chinese (jieba-rs)

```toml
# Add to Cargo.toml
[dependencies]
jieba-rs = "0.6"
```

```rust
use jieba_rs::Jieba;

let jieba = Jieba::new();
let words = jieba.cut("我来到北京清华大学", false);
// Returns: ["我", "来到", "北京", "清华大学"]
```

### Japanese (lindera)

```toml
# Add to Cargo.toml
[dependencies]
lindera = "0.27"
lindera-core = "0.27"
lindera-ipadic = "0.27"
```

```rust
use lindera::tokenizer::Tokenizer;

let tokenizer = Tokenizer::new()?;
let tokens = tokenizer.tokenize("すもももももももものうち")?;
// Returns morphological analysis with part-of-speech tags
```

## Architecture

### Language Detection Flow

```
Input Text
    ↓
┌─────────────────────┐
│ Language Detection  │
│   (whatlang)        │
└──────────┬──────────┘
           ↓
    ┌──────────┐
    │ EN/FR/ES?│
    └─────┬────┘
          │
    ┌─────┴─────┐
    ↓           ↓
┌───────┐   ┌───────┐
│Western│   │  CJK  │
│ Word  │   │ Char  │
│ Token │   │ Token │
└───┬───┘   └───┬───┘
    └──────┬────┘
           ↓
   ┌───────────────┐
   │ Add Metadata  │
   └───────┬───────┘
           ↓
    Enriched Payload
```

### Payload Structure

```json
{
  "id": "doc_001",
  "vector": [0.1, 0.2, ...],
  "payload": {
    "content": "这是一个中文文档",
    "title": "示例文档",
    "language": "zh",
    "language_name": "Chinese",
    "language_confidence": 0.98,
    "is_cjk": true,
    "token_count": 8
  }
}
```

## Performance

### Benchmark Results (Apple M2)

| Operation | Time (avg) | Throughput |
|-----------|-----------|------------|
| Language Detection (EN) | 5.2 μs | ~192K docs/sec |
| Language Detection (ZH) | 6.8 μs | ~147K docs/sec |
| Western Tokenization | 12.3 μs | ~81K docs/sec |
| CJK Tokenization (char) | 18.7 μs | ~53K docs/sec |
| Payload Enrichment | 22.1 μs | ~45K docs/sec |

### Memory Usage

- Language detector: ~1 MB (model data)
- Per-document overhead: ~200 bytes (metadata)

## Limitations

### Current Implementation

1. **CJK Tokenization**: Character-based (not word-based)
   - **Solution**: Integrate jieba-rs (Chinese) or lindera (Japanese) for production

2. **Language Support**: 5 languages (EN, FR, ES, ZH, JA)
   - **Future**: Add more languages via whatlang (supports 87 languages)

3. **Code Mixing**: Limited support for multilingual documents
   - **Example**: "This is English with 中文 mixed in" may have unpredictable detection

4. **Short Texts**: Detection accuracy decreases for <20 characters
   - **Solution**: Use language hints or context from document metadata

## Examples

See `examples/multilang_ingest.rs` for complete examples:

```bash
cargo run --example multilang_ingest
```

## Testing

Run language detection tests:

```bash
# Unit tests
cargo test -p akidb-ingest language

# Integration tests with real data
cargo test -p akidb-ingest --test multilang_integration
```

## Future Enhancements

### Planned Features (Phase 7+)

1. **Word-based CJK tokenization** (jieba-rs, lindera integration)
2. **Language hints** (user-specified language override)
3. **Mixed-language documents** (segment by language)
4. **More languages** (DE, IT, PT, RU, KO, etc.)
5. **Custom tokenization rules** (domain-specific dictionaries)
6. **Language-specific stop words** (filter common words)
7. **Transliteration support** (e.g., Pinyin for Chinese)

### Community Contributions Welcome!

Want to add support for more languages? See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## References

- [whatlang-rs](https://github.com/greyblake/whatlang-rs) - Language detection library
- [jieba-rs](https://github.com/messense/jieba-rs) - Chinese text segmentation
- [lindera](https://github.com/lindera-morphology/lindera) - Japanese morphological analysis
- [Unicode Segmentation](https://docs.rs/unicode-segmentation/) - Unicode text segmentation
- [ISO 639-1](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes) - Language codes

## Support

For questions or issues with multi-language support:

- GitHub Issues: https://github.com/aifocal/akidb/issues
- Discussions: https://github.com/aifocal/akidb/discussions
- Email: support@aifocal.com
