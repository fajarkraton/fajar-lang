//! Playground UI — Monaco editor integration, output panel, keyboard shortcuts.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Monaco Editor Configuration (S30.1, S30.2)
// ═══════════════════════════════════════════════════════════════════════

/// Monaco Editor configuration for the Fajar Lang playground.
#[derive(Debug, Clone)]
pub struct MonacoConfig {
    /// Language ID registered with Monaco.
    pub language_id: String,
    /// Theme name.
    pub theme_name: String,
    /// Font family.
    pub font_family: String,
    /// Font size in pixels.
    pub font_size: u32,
    /// Enable line numbers.
    pub line_numbers: bool,
    /// Enable minimap.
    pub minimap: bool,
    /// Tab size.
    pub tab_size: u32,
    /// Word wrap mode.
    pub word_wrap: WordWrap,
}

/// Word wrap mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordWrap {
    /// No wrapping.
    Off,
    /// Wrap at viewport width.
    On,
    /// Wrap at specific column.
    WordWrapColumn(u32),
}

impl Default for MonacoConfig {
    fn default() -> Self {
        Self {
            language_id: "fajar".to_string(),
            theme_name: "fajar-dark".to_string(),
            font_family: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace".to_string(),
            font_size: 14,
            line_numbers: true,
            minimap: false,
            tab_size: 4,
            word_wrap: WordWrap::Off,
        }
    }
}

/// Fajar Lang token types for Monaco syntax highlighting.
pub fn monarch_token_rules() -> Vec<TokenRule> {
    vec![
        TokenRule {
            regex: r"\b(fn|let|mut|const|struct|enum|impl|trait|type|if|else|match|while|for|in|return|break|continue|loop|use|mod|pub|extern|as)\b".to_string(),
            token_type: "keyword".to_string(),
        },
        TokenRule {
            regex: r"\b(bool|i8|i16|i32|i64|i128|u8|u16|u32|u64|u128|isize|usize|f32|f64|str|char|void|never)\b".to_string(),
            token_type: "type".to_string(),
        },
        TokenRule {
            regex: r"\b(tensor|grad|loss|layer|model)\b".to_string(),
            token_type: "type.ml".to_string(),
        },
        TokenRule {
            regex: r"\b(ptr|addr|page|region|irq|syscall)\b".to_string(),
            token_type: "type.os".to_string(),
        },
        TokenRule {
            regex: r"@(kernel|device|safe|unsafe|ffi|npu|infer|test|should_panic|ignore)".to_string(),
            token_type: "annotation".to_string(),
        },
        TokenRule {
            regex: r"\b(true|false|null)\b".to_string(),
            token_type: "constant".to_string(),
        },
        TokenRule {
            regex: r"\b(Some|None|Ok|Err)\b".to_string(),
            token_type: "constant.builtin".to_string(),
        },
        TokenRule {
            regex: r#""[^"]*""#.to_string(),
            token_type: "string".to_string(),
        },
        TokenRule {
            regex: r"//.*$".to_string(),
            token_type: "comment".to_string(),
        },
        TokenRule {
            regex: r"\b\d+(\.\d+)?\b".to_string(),
            token_type: "number".to_string(),
        },
        TokenRule {
            regex: r"\|>".to_string(),
            token_type: "operator.pipeline".to_string(),
        },
    ]
}

/// A syntax highlighting token rule.
#[derive(Debug, Clone)]
pub struct TokenRule {
    /// Regular expression pattern.
    pub regex: String,
    /// Token type name (for theming).
    pub token_type: String,
}

/// Fajar Lang theme definition for Monaco.
#[derive(Debug, Clone)]
pub struct MonacoTheme {
    /// Theme name.
    pub name: String,
    /// Base theme to inherit from.
    pub base: String,
    /// Whether this is a dark theme.
    pub is_dark: bool,
    /// Color rules.
    pub rules: Vec<ThemeRule>,
    /// Editor colors.
    pub colors: Vec<(String, String)>,
}

/// A single theme color rule.
#[derive(Debug, Clone)]
pub struct ThemeRule {
    /// Token type to style.
    pub token: String,
    /// Foreground color (hex).
    pub foreground: String,
    /// Whether to bold.
    pub bold: bool,
}

/// Returns the default dark theme for Fajar Lang.
pub fn fajar_dark_theme() -> MonacoTheme {
    MonacoTheme {
        name: "fajar-dark".to_string(),
        base: "vs-dark".to_string(),
        is_dark: true,
        rules: vec![
            ThemeRule {
                token: "keyword".to_string(),
                foreground: "f85149".to_string(),
                bold: true,
            },
            ThemeRule {
                token: "type".to_string(),
                foreground: "79c0ff".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "type.ml".to_string(),
                foreground: "d2a8ff".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "type.os".to_string(),
                foreground: "ffa657".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "annotation".to_string(),
                foreground: "d29922".to_string(),
                bold: true,
            },
            ThemeRule {
                token: "string".to_string(),
                foreground: "3fb950".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "number".to_string(),
                foreground: "d2a8ff".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "comment".to_string(),
                foreground: "8b949e".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "constant".to_string(),
                foreground: "79c0ff".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "operator.pipeline".to_string(),
                foreground: "ff7b72".to_string(),
                bold: true,
            },
        ],
        colors: vec![
            ("editor.background".to_string(), "#0d1117".to_string()),
            ("editor.foreground".to_string(), "#e6edf3".to_string()),
            (
                "editorLineNumber.foreground".to_string(),
                "#484f58".to_string(),
            ),
            (
                "editor.selectionBackground".to_string(),
                "#264f78".to_string(),
            ),
        ],
    }
}

/// Returns the light theme for Fajar Lang.
pub fn fajar_light_theme() -> MonacoTheme {
    MonacoTheme {
        name: "fajar-light".to_string(),
        base: "vs".to_string(),
        is_dark: false,
        rules: vec![
            ThemeRule {
                token: "keyword".to_string(),
                foreground: "cf222e".to_string(),
                bold: true,
            },
            ThemeRule {
                token: "type".to_string(),
                foreground: "0550ae".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "string".to_string(),
                foreground: "116329".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "number".to_string(),
                foreground: "8250df".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "comment".to_string(),
                foreground: "6e7781".to_string(),
                bold: false,
            },
            ThemeRule {
                token: "annotation".to_string(),
                foreground: "953800".to_string(),
                bold: true,
            },
        ],
        colors: vec![
            ("editor.background".to_string(), "#ffffff".to_string()),
            ("editor.foreground".to_string(), "#1f2328".to_string()),
        ],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Output Panel (S30.3, S30.4)
// ═══════════════════════════════════════════════════════════════════════

/// Output panel tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputTab {
    /// Standard output.
    Stdout,
    /// Return value.
    Result,
    /// Error messages.
    Errors,
    /// AST tree view.
    Ast,
    /// Token list.
    Tokens,
}

impl fmt::Display for OutputTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout => write!(f, "Output"),
            Self::Result => write!(f, "Result"),
            Self::Errors => write!(f, "Errors"),
            Self::Ast => write!(f, "AST"),
            Self::Tokens => write!(f, "Tokens"),
        }
    }
}

/// Returns the default output panel tabs.
pub fn output_tabs() -> Vec<OutputTab> {
    vec![
        OutputTab::Stdout,
        OutputTab::Result,
        OutputTab::Errors,
        OutputTab::Ast,
        OutputTab::Tokens,
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Keyboard Shortcuts (S30.7)
// ═══════════════════════════════════════════════════════════════════════

/// A keyboard shortcut binding.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Key combination (e.g., "Ctrl+Enter").
    pub key: String,
    /// Action name.
    pub action: String,
    /// Description.
    pub description: String,
}

/// Returns playground keyboard shortcuts.
pub fn keyboard_shortcuts() -> Vec<KeyBinding> {
    vec![
        KeyBinding {
            key: "Ctrl+Enter".to_string(),
            action: "run".to_string(),
            description: "Run the program".to_string(),
        },
        KeyBinding {
            key: "Ctrl+S".to_string(),
            action: "save".to_string(),
            description: "Save to localStorage".to_string(),
        },
        KeyBinding {
            key: "Ctrl+L".to_string(),
            action: "clear".to_string(),
            description: "Clear output".to_string(),
        },
        KeyBinding {
            key: "Ctrl+Shift+F".to_string(),
            action: "format".to_string(),
            description: "Format code".to_string(),
        },
        KeyBinding {
            key: "Ctrl+/".to_string(),
            action: "comment".to_string(),
            description: "Toggle line comment".to_string(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Layout (S30.8)
// ═══════════════════════════════════════════════════════════════════════

/// Playground layout mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutMode {
    /// Side-by-side (desktop).
    Horizontal,
    /// Stacked (mobile).
    Vertical,
}

/// Returns the layout mode based on viewport width.
pub fn layout_for_width(width: u32) -> LayoutMode {
    if width < 768 {
        LayoutMode::Vertical
    } else {
        LayoutMode::Horizontal
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Local Storage (S30.9)
// ═══════════════════════════════════════════════════════════════════════

/// Local storage keys used by the playground.
pub fn storage_keys() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fj-playground-code", "Editor content"),
        ("fj-playground-theme", "Theme preference"),
        ("fj-playground-layout", "Layout preference"),
        ("fj-playground-font-size", "Font size"),
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S30.1: Monaco editor
    #[test]
    fn s30_1_monaco_config() {
        let cfg = MonacoConfig::default();
        assert_eq!(cfg.language_id, "fajar");
        assert_eq!(cfg.font_size, 14);
        assert!(!cfg.minimap);
    }

    #[test]
    fn s30_1_monarch_token_rules() {
        let rules = monarch_token_rules();
        assert!(rules.len() >= 10);
        assert!(rules.iter().any(|r| r.token_type == "keyword"));
        assert!(rules.iter().any(|r| r.token_type == "annotation"));
        assert!(rules.iter().any(|r| r.token_type == "string"));
        assert!(rules.iter().any(|r| r.token_type == "operator.pipeline"));
    }

    // S30.2: Syntax theme
    #[test]
    fn s30_2_fajar_dark_theme() {
        let theme = fajar_dark_theme();
        assert_eq!(theme.name, "fajar-dark");
        assert!(theme.is_dark);
        assert!(!theme.rules.is_empty());
        assert!(theme.rules.iter().any(|r| r.token == "keyword"));
    }

    #[test]
    fn s30_2_fajar_light_theme() {
        let theme = fajar_light_theme();
        assert_eq!(theme.name, "fajar-light");
        assert!(!theme.is_dark);
        assert!(!theme.rules.is_empty());
    }

    // S30.4: Output panel
    #[test]
    fn s30_4_output_tabs() {
        let tabs = output_tabs();
        assert_eq!(tabs.len(), 5);
        assert_eq!(tabs[0], OutputTab::Stdout);
        assert_eq!(format!("{}", OutputTab::Result), "Result");
    }

    // S30.7: Keyboard shortcuts
    #[test]
    fn s30_7_keyboard_shortcuts() {
        let shortcuts = keyboard_shortcuts();
        assert!(shortcuts.len() >= 4);
        assert!(
            shortcuts
                .iter()
                .any(|s| s.key == "Ctrl+Enter" && s.action == "run")
        );
        assert!(
            shortcuts
                .iter()
                .any(|s| s.key == "Ctrl+S" && s.action == "save")
        );
        assert!(
            shortcuts
                .iter()
                .any(|s| s.key == "Ctrl+L" && s.action == "clear")
        );
    }

    // S30.8: Responsive layout
    #[test]
    fn s30_8_layout_responsive() {
        assert_eq!(layout_for_width(375), LayoutMode::Vertical);
        assert_eq!(layout_for_width(768), LayoutMode::Horizontal);
        assert_eq!(layout_for_width(1920), LayoutMode::Horizontal);
    }

    // S30.9: Local storage keys
    #[test]
    fn s30_9_storage_keys() {
        let keys = storage_keys();
        assert!(keys.len() >= 4);
        assert!(keys.iter().any(|(k, _)| *k == "fj-playground-code"));
        assert!(keys.iter().any(|(k, _)| *k == "fj-playground-theme"));
    }

    // S30.10: Theme rule structure
    #[test]
    fn s30_10_theme_rules_have_colors() {
        let theme = fajar_dark_theme();
        for rule in &theme.rules {
            assert!(!rule.foreground.is_empty());
            assert_eq!(rule.foreground.len(), 6); // hex without #
        }
    }

    #[test]
    fn s30_10_word_wrap_variants() {
        let cfg = MonacoConfig::default();
        assert_eq!(cfg.word_wrap, WordWrap::Off);
    }
}
