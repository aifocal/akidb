use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use whatlang::{detect, Lang};

/// Language detection and tokenization errors
#[derive(Debug, Error)]
pub enum LanguageError {
    #[error("Language detection failed: {0}")]
    DetectionFailed(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Tokenization failed: {0}")]
    TokenizationFailed(String),
}

/// Supported languages for AkiDB offline RAG
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportedLanguage {
    /// English
    EN,
    /// French
    FR,
    /// Chinese (Simplified & Traditional)
    ZH,
    /// Spanish
    ES,
    /// Japanese
    JA,
}

impl SupportedLanguage {
    /// Get ISO 639-1 language code
    pub fn code(&self) -> &'static str {
        match self {
            Self::EN => "en",
            Self::FR => "fr",
            Self::ZH => "zh",
            Self::ES => "es",
            Self::JA => "ja",
        }
    }

    /// Get human-readable language name
    pub fn name(&self) -> &'static str {
        match self {
            Self::EN => "English",
            Self::FR => "French",
            Self::ZH => "Chinese",
            Self::ES => "Spanish",
            Self::JA => "Japanese",
        }
    }

    /// Check if this is a CJK (Chinese, Japanese, Korean) language
    pub fn is_cjk(&self) -> bool {
        matches!(self, Self::ZH | Self::JA)
    }

    /// Convert from whatlang Lang to SupportedLanguage
    pub fn from_whatlang(lang: Lang) -> Option<Self> {
        match lang {
            Lang::Eng => Some(Self::EN),
            Lang::Fra => Some(Self::FR),
            Lang::Cmn => Some(Self::ZH), // Mandarin Chinese
            Lang::Spa => Some(Self::ES),
            Lang::Jpn => Some(Self::JA),
            _ => None,
        }
    }
}

/// Language metadata for vector payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageMetadata {
    /// Detected language
    pub language: SupportedLanguage,
    /// Language code (ISO 639-1)
    pub language_code: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Whether text is in CJK script
    pub is_cjk: bool,
    /// Number of tokens after tokenization
    pub token_count: usize,
}

/// Language detector with CJK tokenization support
pub struct LanguageDetector {
    /// Minimum confidence threshold for language detection
    confidence_threshold: f64,
}

impl LanguageDetector {
    /// Create a new language detector
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.7, // 70% confidence threshold
        }
    }

    /// Create a language detector with custom confidence threshold
    pub fn with_confidence(confidence_threshold: f64) -> Self {
        Self {
            confidence_threshold,
        }
    }

    /// Detect language from text
    pub fn detect(&self, text: &str) -> Result<SupportedLanguage, LanguageError> {
        if text.trim().is_empty() {
            return Err(LanguageError::DetectionFailed(
                "Empty text provided".to_string(),
            ));
        }

        // Use whatlang for detection
        let info = detect(text).ok_or_else(|| {
            LanguageError::DetectionFailed("Could not detect language".to_string())
        })?;

        // Check confidence threshold
        if info.confidence() < self.confidence_threshold {
            return Err(LanguageError::DetectionFailed(format!(
                "Low confidence: {:.2} < {:.2}",
                info.confidence(),
                self.confidence_threshold
            )));
        }

        // Convert to supported language
        SupportedLanguage::from_whatlang(info.lang()).ok_or_else(|| {
            LanguageError::UnsupportedLanguage(format!(
                "Detected language {:?} is not supported",
                info.lang()
            ))
        })
    }

    /// Detect language with metadata
    pub fn detect_with_metadata(&self, text: &str) -> Result<LanguageMetadata, LanguageError> {
        let info = detect(text).ok_or_else(|| {
            LanguageError::DetectionFailed("Could not detect language".to_string())
        })?;

        let language = SupportedLanguage::from_whatlang(info.lang()).ok_or_else(|| {
            LanguageError::UnsupportedLanguage(format!(
                "Detected language {:?} is not supported",
                info.lang()
            ))
        })?;

        // Tokenize to get token count
        let tokens = self.tokenize(text, language)?;

        Ok(LanguageMetadata {
            language,
            language_code: language.code().to_string(),
            confidence: info.confidence(),
            is_cjk: language.is_cjk(),
            token_count: tokens.len(),
        })
    }

    /// Tokenize text based on language
    pub fn tokenize(
        &self,
        text: &str,
        language: SupportedLanguage,
    ) -> Result<Vec<String>, LanguageError> {
        if language.is_cjk() {
            self.tokenize_cjk(text)
        } else {
            self.tokenize_western(text)
        }
    }

    /// Tokenize Western languages (EN, FR, ES) using Unicode word boundaries
    fn tokenize_western(&self, text: &str) -> Result<Vec<String>, LanguageError> {
        let tokens: Vec<String> = text
            .unicode_words()
            .map(|word| word.to_lowercase())
            .collect();

        Ok(tokens)
    }

    /// Tokenize CJK languages (ZH, JA) using Unicode grapheme clusters
    ///
    /// Note: This is a simple character-based tokenization. For production use,
    /// consider integrating proper CJK segmentation libraries:
    /// - Chinese: jieba-rs, cedarwood
    /// - Japanese: lindera, vibrato
    fn tokenize_cjk(&self, text: &str) -> Result<Vec<String>, LanguageError> {
        // Simple grapheme-based tokenization
        // In production, use proper segmentation:
        // - Chinese: Use jieba-rs for word segmentation
        // - Japanese: Use lindera for morphological analysis
        let tokens: Vec<String> = text
            .graphemes(true)
            .filter(|g| !g.trim().is_empty())
            .map(|g| g.to_string())
            .collect();

        Ok(tokens)
    }

    /// Extract language metadata and add to payload
    pub fn enrich_payload(
        &self,
        text: &str,
        mut payload: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>, LanguageError> {
        let metadata = self.detect_with_metadata(text)?;

        // Add language metadata to payload
        payload.insert(
            "language".to_string(),
            serde_json::json!(metadata.language_code),
        );
        payload.insert(
            "language_name".to_string(),
            serde_json::json!(metadata.language.name()),
        );
        payload.insert(
            "language_confidence".to_string(),
            serde_json::json!(metadata.confidence),
        );
        payload.insert("is_cjk".to_string(), serde_json::json!(metadata.is_cjk));
        payload.insert(
            "token_count".to_string(),
            serde_json::json!(metadata.token_count),
        );

        Ok(payload)
    }
}

impl Default for LanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_english() {
        let detector = LanguageDetector::new();
        let result = detector.detect("This is an English sentence for testing.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SupportedLanguage::EN);
    }

    #[test]
    fn test_detect_french() {
        let detector = LanguageDetector::new();
        let result = detector.detect("Ceci est une phrase en français pour les tests.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SupportedLanguage::FR);
    }

    #[test]
    fn test_detect_spanish() {
        let detector = LanguageDetector::new();
        let result = detector.detect("Esta es una oración en español para pruebas.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SupportedLanguage::ES);
    }

    #[test]
    fn test_detect_chinese() {
        let detector = LanguageDetector::new();
        let result = detector.detect("这是一个用于测试的中文句子。");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SupportedLanguage::ZH);
    }

    #[test]
    fn test_detect_japanese() {
        let detector = LanguageDetector::new();
        let result = detector.detect("これはテスト用の日本語の文です。");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SupportedLanguage::JA);
    }

    #[test]
    fn test_tokenize_english() {
        let detector = LanguageDetector::new();
        let tokens = detector
            .tokenize("Hello World!", SupportedLanguage::EN)
            .unwrap();
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_cjk() {
        let detector = LanguageDetector::new();
        let tokens = detector
            .tokenize("你好世界", SupportedLanguage::ZH)
            .unwrap();
        assert_eq!(tokens.len(), 4); // 4 characters
    }

    #[test]
    fn test_detect_with_metadata() {
        let detector = LanguageDetector::new();
        let metadata = detector
            .detect_with_metadata("This is an English sentence.")
            .unwrap();

        assert_eq!(metadata.language, SupportedLanguage::EN);
        assert_eq!(metadata.language_code, "en");
        assert!(metadata.confidence > 0.7);
        assert!(!metadata.is_cjk);
        assert!(metadata.token_count > 0);
    }

    #[test]
    fn test_enrich_payload() {
        let detector = LanguageDetector::new();
        let mut payload = HashMap::new();
        payload.insert("content".to_string(), serde_json::json!("Hello World"));

        let enriched = detector
            .enrich_payload("This is an English sentence.", payload)
            .unwrap();

        assert!(enriched.contains_key("language"));
        assert!(enriched.contains_key("language_name"));
        assert!(enriched.contains_key("language_confidence"));
        assert!(enriched.contains_key("is_cjk"));
        assert!(enriched.contains_key("token_count"));
    }

    #[test]
    fn test_empty_text() {
        let detector = LanguageDetector::new();
        let result = detector.detect("");
        assert!(result.is_err());
    }

    #[test]
    fn test_supported_language_code() {
        assert_eq!(SupportedLanguage::EN.code(), "en");
        assert_eq!(SupportedLanguage::FR.code(), "fr");
        assert_eq!(SupportedLanguage::ZH.code(), "zh");
        assert_eq!(SupportedLanguage::ES.code(), "es");
        assert_eq!(SupportedLanguage::JA.code(), "ja");
    }

    #[test]
    fn test_is_cjk() {
        assert!(!SupportedLanguage::EN.is_cjk());
        assert!(!SupportedLanguage::FR.is_cjk());
        assert!(SupportedLanguage::ZH.is_cjk());
        assert!(!SupportedLanguage::ES.is_cjk());
        assert!(SupportedLanguage::JA.is_cjk());
    }
}
