// Integration tests for search functionality
// These tests verify that the search logic handles edge cases correctly
    // Test helper to create a SearchPanel-like structure for testing
    struct TestSearchPanel {
        search_query: String,
        case_sensitive: bool,
    }
    
    impl TestSearchPanel {
        fn new() -> Self {
            Self {
                search_query: String::new(),
                case_sensitive: false,
            }
        }
        
        fn search_in_text(&self, text: &str) -> Vec<(usize, String)> {
            let mut matches = Vec::new();
            
            if self.search_query.is_empty() {
                return matches;
            }
            
            let query = if self.case_sensitive {
                self.search_query.clone()
            } else {
                self.search_query.to_lowercase()
            };
            
            if query.is_empty() {
                return matches;
            }
            
            let search_text = if self.case_sensitive {
                text.to_string()
            } else {
                text.to_lowercase()
            };
            
            let text_chars: Vec<(usize, char)> = text.char_indices().collect();
            let search_chars: Vec<char> = search_text.chars().collect();
            let query_chars: Vec<char> = query.chars().collect();
            
            if query_chars.is_empty() || search_chars.len() < query_chars.len() {
                return matches;
            }
            
            let mut start_char_idx = 0;
            while start_char_idx <= search_chars.len().saturating_sub(query_chars.len()) {
                let mut matched = true;
                for (i, &qc) in query_chars.iter().enumerate() {
                    if start_char_idx + i >= search_chars.len() || search_chars[start_char_idx + i] != qc {
                        matched = false;
                        break;
                    }
                }
                
                if matched {
                    let byte_pos = if start_char_idx < text_chars.len() {
                        text_chars[start_char_idx].0
                    } else {
                        text.len()
                    };
                    
                    let context_before = 40;
                    let context_after = 40;
                    let context_start_char = start_char_idx.saturating_sub(context_before);
                    let context_end_char = (start_char_idx + query_chars.len() + context_after)
                        .min(text_chars.len());
                    
                    let context_start_byte = if context_start_char < text_chars.len() {
                        text_chars[context_start_char].0
                    } else {
                        0
                    };
                    
                    let context_end_byte = if context_end_char < text_chars.len() {
                        text_chars[context_end_char].0
                    } else {
                        text.len()
                    };
                    
                    let context = if context_start_byte < context_end_byte && context_end_byte <= text.len() {
                        text.get(context_start_byte..context_end_byte)
                            .unwrap_or("")
                            .to_string()
                    } else {
                        String::new()
                    };
                    
                    matches.push((byte_pos, context));
                    start_char_idx += query_chars.len();
                } else {
                    start_char_idx += 1;
                }
            }
            
            matches
        }
    }
    
    #[test]
    fn test_search_empty_query() {
        let panel = TestSearchPanel::new();
        let text = "This is a test document with some content.";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 0);
    }
    
    #[test]
    fn test_search_simple_match() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "test".to_string();
        let text = "This is a test document with some content.";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.contains("test"));
    }
    
    #[test]
    fn test_search_utf8_text() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "世界".to_string();
        let text = "Hello 世界 🌍 test";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.contains("世界"));
    }
    
    #[test]
    fn test_search_multibyte_chars() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "café".to_string();
        let text = "Test 🚀 rocket emoji and café and naïve";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.contains("café"));
    }
    
    #[test]
    fn test_search_emoji() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "🚀".to_string();
        let text = "Test 🚀 rocket emoji";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_search_case_insensitive() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "TEST".to_string();
        panel.case_sensitive = false;
        let text = "This is a test document";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_search_case_sensitive() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "TEST".to_string();
        panel.case_sensitive = true;
        let text = "This is a test document";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 0); // Should not match lowercase "test"
    }
    
    #[test]
    fn test_search_multiple_matches() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "test".to_string();
        let text = "This is a test. Another test. And yet another test.";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 3);
    }
    
    #[test]
    fn test_search_empty_text() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "test".to_string();
        let text = "";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 0);
    }
    
    #[test]
    fn test_search_special_characters() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "naïve".to_string();
        let text = "The word naïve contains special characters";
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_search_very_long_text() {
        let mut panel = TestSearchPanel::new();
        panel.search_query = "needle".to_string();
        let text = &("haystack ".repeat(1000) + "needle" + &" haystack ".repeat(1000));
        let results = panel.search_in_text(text);
        assert_eq!(results.len(), 1);
    }
