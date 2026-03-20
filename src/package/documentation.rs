//! Documentation generation and learning infrastructure for Fajar Lang.
//!
//! Provides reference manual generation, progressive tutorials, enhanced
//! doc output with cross-references, browser playground support,
//! documentation quality validation, and static site generation.

use std::collections::HashMap;
use std::fmt;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during documentation generation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DocumentationError {
    /// A cross-reference target could not be resolved.
    #[error("unresolved reference: [{target}]")]
    UnresolvedReference {
        /// The reference target name.
        target: String,
    },

    /// A code example failed validation.
    #[error("example '{title}' failed: {reason}")]
    ExampleFailed {
        /// Title of the failing example.
        title: String,
        /// Reason for the failure.
        reason: String,
    },

    /// Tutorial numbering is invalid or out of order.
    #[error("invalid tutorial number {number}: {reason}")]
    InvalidTutorialNumber {
        /// The problematic tutorial number.
        number: u32,
        /// Description of the issue.
        reason: String,
    },

    /// Playground execution timed out.
    #[error("playground execution timed out after {timeout_ms}ms")]
    PlaygroundTimeout {
        /// Configured timeout in milliseconds.
        timeout_ms: u64,
    },

    /// Playground execution exceeded memory limit.
    #[error("playground exceeded memory limit of {max_mb}MB")]
    PlaygroundMemoryExceeded {
        /// Maximum allowed memory in megabytes.
        max_mb: u64,
    },

    /// Site generation failed.
    #[error("site generation error: {reason}")]
    SiteGenerationError {
        /// Reason for the failure.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// 1. ReferenceGenerator — Language reference manual
// ═══════════════════════════════════════════════════════════════════════

/// Section categories in the language reference manual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceSection {
    /// Primitive and composite types.
    Types,
    /// Unary, binary, and special operators.
    Operators,
    /// Expression forms (literals, calls, closures).
    Expressions,
    /// Statement forms (let, if, while, for).
    Statements,
    /// Ownership and borrowing rules.
    Ownership,
    /// Context annotations (@kernel, @device, @safe, @unsafe).
    Contexts,
    /// Effect system and error handling.
    Effects,
    /// Macro system.
    Macros,
    /// Threads, channels, async/await.
    Concurrency,
    /// Generic types, traits, monomorphization.
    Generics,
}

impl fmt::Display for ReferenceSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Types => write!(f, "Types"),
            Self::Operators => write!(f, "Operators"),
            Self::Expressions => write!(f, "Expressions"),
            Self::Statements => write!(f, "Statements"),
            Self::Ownership => write!(f, "Ownership"),
            Self::Contexts => write!(f, "Contexts"),
            Self::Effects => write!(f, "Effects"),
            Self::Macros => write!(f, "Macros"),
            Self::Concurrency => write!(f, "Concurrency"),
            Self::Generics => write!(f, "Generics"),
        }
    }
}

impl ReferenceSection {
    /// Returns a URL-safe anchor slug for this section.
    pub fn anchor(&self) -> &'static str {
        match self {
            Self::Types => "types",
            Self::Operators => "operators",
            Self::Expressions => "expressions",
            Self::Statements => "statements",
            Self::Ownership => "ownership",
            Self::Contexts => "contexts",
            Self::Effects => "effects",
            Self::Macros => "macros",
            Self::Concurrency => "concurrency",
            Self::Generics => "generics",
        }
    }

    /// Returns all reference sections in canonical order.
    pub fn all() -> &'static [ReferenceSection] {
        &[
            Self::Types,
            Self::Operators,
            Self::Expressions,
            Self::Statements,
            Self::Ownership,
            Self::Contexts,
            Self::Effects,
            Self::Macros,
            Self::Concurrency,
            Self::Generics,
        ]
    }
}

/// A runnable code example in the reference manual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeExample {
    /// Short title for the example.
    pub title: String,
    /// Fajar Lang source code.
    pub code: String,
    /// Expected output when run.
    pub expected_output: String,
    /// Prose description of what the example demonstrates.
    pub description: String,
}

impl CodeExample {
    /// Creates a new code example.
    pub fn new(
        title: impl Into<String>,
        code: impl Into<String>,
        expected_output: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            code: code.into(),
            expected_output: expected_output.into(),
            description: description.into(),
        }
    }
}

/// A single entry in the language reference manual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceEntry {
    /// Entry title (e.g., "Integer Types").
    pub title: String,
    /// Which section this entry belongs to.
    pub section: ReferenceSection,
    /// The main content in Markdown format.
    pub content_markdown: String,
    /// Code examples illustrating the concept.
    pub examples: Vec<CodeExample>,
    /// Names of related entries for cross-linking.
    pub see_also: Vec<String>,
}

impl ReferenceEntry {
    /// Creates a new reference entry.
    pub fn new(
        title: impl Into<String>,
        section: ReferenceSection,
        content_markdown: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            section,
            content_markdown: content_markdown.into(),
            examples: Vec::new(),
            see_also: Vec::new(),
        }
    }

    /// Adds a code example to this entry.
    pub fn add_example(&mut self, example: CodeExample) {
        self.examples.push(example);
    }

    /// Adds a see-also reference.
    pub fn add_see_also(&mut self, name: impl Into<String>) {
        self.see_also.push(name.into());
    }

    /// Returns a URL-safe anchor for this entry.
    pub fn anchor(&self) -> String {
        slug(&self.title)
    }
}

/// The complete language reference manual.
#[derive(Debug, Clone)]
pub struct ReferenceManual {
    /// All entries in insertion order.
    entries: Vec<ReferenceEntry>,
    /// Map from entry title to index for cross-ref resolution.
    title_index: HashMap<String, usize>,
}

impl ReferenceManual {
    /// Creates an empty reference manual.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            title_index: HashMap::new(),
        }
    }

    /// Adds an entry to the manual.
    pub fn add_entry(&mut self, entry: ReferenceEntry) {
        let idx = self.entries.len();
        self.title_index.insert(entry.title.clone(), idx);
        self.entries.push(entry);
    }

    /// Returns entries belonging to a given section.
    pub fn entries_for_section(&self, section: ReferenceSection) -> Vec<&ReferenceEntry> {
        self.entries
            .iter()
            .filter(|e| e.section == section)
            .collect()
    }

    /// Returns the total number of entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Generates the full reference manual as Markdown.
    pub fn generate_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Fajar Lang Reference Manual\n\n");
        md.push_str(&self.generate_toc());
        md.push('\n');

        for section in ReferenceSection::all() {
            let entries = self.entries_for_section(*section);
            if entries.is_empty() {
                continue;
            }
            md.push_str(&format!("## {section}\n\n"));
            for entry in entries {
                self.render_entry_markdown(entry, &mut md);
            }
        }

        md.push_str(&self.generate_index());
        md
    }

    /// Generates an HTML version of the reference manual.
    pub fn generate_html(&self) -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("  <meta charset=\"utf-8\">\n");
        html.push_str("  <title>Fajar Lang Reference Manual</title>\n");
        html.push_str("  <style>\n");
        html.push_str("    body { font-family: system-ui, sans-serif; ");
        html.push_str("max-width: 900px; margin: 0 auto; padding: 2em; }\n");
        html.push_str("    pre { background: #f5f5f5; padding: 1em; ");
        html.push_str("overflow-x: auto; border-radius: 4px; }\n");
        html.push_str("    .see-also { color: #0366d6; }\n");
        html.push_str("    .toc a { text-decoration: none; }\n");
        html.push_str("    h2 { border-bottom: 1px solid #eee; ");
        html.push_str("padding-bottom: 0.3em; }\n");
        html.push_str("  </style>\n");
        html.push_str("</head>\n<body>\n");
        html.push_str("<h1>Fajar Lang Reference Manual</h1>\n");

        html.push_str(&self.generate_toc_html());

        for section in ReferenceSection::all() {
            let entries = self.entries_for_section(*section);
            if entries.is_empty() {
                continue;
            }
            let anchor = section.anchor();
            html.push_str(&format!("<h2 id=\"{anchor}\">{section}</h2>\n"));
            for entry in entries {
                self.render_entry_html(entry, &mut html);
            }
        }

        html.push_str(&self.generate_index_html());
        html.push_str("</body>\n</html>\n");
        html
    }

    /// Generates a Markdown table of contents.
    fn generate_toc(&self) -> String {
        let mut toc = String::from("## Table of Contents\n\n");
        for section in ReferenceSection::all() {
            let entries = self.entries_for_section(*section);
            if entries.is_empty() {
                continue;
            }
            toc.push_str(&format!("- [{}](#{})\n", section, section.anchor()));
            for entry in &entries {
                toc.push_str(&format!("  - [{}](#{})\n", entry.title, entry.anchor()));
            }
        }
        toc
    }

    /// Generates an HTML table of contents.
    fn generate_toc_html(&self) -> String {
        let mut html = String::from("<nav class=\"toc\">\n<h2>Table of Contents</h2>\n<ul>\n");
        for section in ReferenceSection::all() {
            let entries = self.entries_for_section(*section);
            if entries.is_empty() {
                continue;
            }
            let anchor = section.anchor();
            html.push_str(&format!("<li><a href=\"#{anchor}\">{section}</a>\n<ul>\n"));
            for entry in &entries {
                let ea = entry.anchor();
                html.push_str(&format!(
                    "<li><a href=\"#{ea}\">{}</a></li>\n",
                    html_escape(&entry.title)
                ));
            }
            html.push_str("</ul></li>\n");
        }
        html.push_str("</ul>\n</nav>\n");
        html
    }

    /// Renders a single entry as Markdown.
    fn render_entry_markdown(&self, entry: &ReferenceEntry, md: &mut String) {
        md.push_str(&format!("### {}\n\n", entry.title));
        let resolved = self.resolve_cross_refs(&entry.content_markdown);
        md.push_str(&resolved);
        md.push_str("\n\n");

        for ex in &entry.examples {
            md.push_str(&format!("**Example: {}**\n\n", ex.title));
            md.push_str(&format!("```fj\n{}\n```\n\n", ex.code));
            if !ex.expected_output.is_empty() {
                md.push_str(&format!("Output: `{}`\n\n", ex.expected_output));
            }
            if !ex.description.is_empty() {
                md.push_str(&format!("{}\n\n", ex.description));
            }
        }

        if !entry.see_also.is_empty() {
            md.push_str("**See also:** ");
            let links: Vec<String> = entry
                .see_also
                .iter()
                .map(|s| format!("[{s}](#{})", slug(s)))
                .collect();
            md.push_str(&links.join(", "));
            md.push_str("\n\n");
        }
    }

    /// Renders a single entry as HTML.
    fn render_entry_html(&self, entry: &ReferenceEntry, html: &mut String) {
        let anchor = entry.anchor();
        html.push_str(&format!(
            "<h3 id=\"{anchor}\">{}</h3>\n",
            html_escape(&entry.title)
        ));
        let resolved = self.resolve_cross_refs(&entry.content_markdown);
        html.push_str(&format!("<p>{}</p>\n", html_escape(&resolved)));

        for ex in &entry.examples {
            html.push_str(&format!("<h4>{}</h4>\n", html_escape(&ex.title)));
            html.push_str(&format!(
                "<pre><code>{}</code></pre>\n",
                html_escape(&ex.code)
            ));
            if !ex.expected_output.is_empty() {
                html.push_str(&format!(
                    "<p>Output: <code>{}</code></p>\n",
                    html_escape(&ex.expected_output)
                ));
            }
            if !ex.description.is_empty() {
                html.push_str(&format!("<p>{}</p>\n", html_escape(&ex.description)));
            }
        }

        if !entry.see_also.is_empty() {
            html.push_str("<p class=\"see-also\">See also: ");
            let links: Vec<String> = entry
                .see_also
                .iter()
                .map(|s| format!("<a href=\"#{}\">{}</a>", slug(s), html_escape(s)))
                .collect();
            html.push_str(&links.join(", "));
            html.push_str("</p>\n");
        }
    }

    /// Resolves `[TypeName]` cross-references in text.
    fn resolve_cross_refs(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut remaining = text;

        while let Some(start) = remaining.find('[') {
            result.push_str(&remaining[..start]);
            let after = &remaining[start + 1..];

            if let Some(end) = after.find(']') {
                let name = &after[..end];
                if self.title_index.contains_key(name) {
                    result.push_str(&format!("[{name}](#{})", slug(name)));
                } else {
                    result.push('[');
                    result.push_str(name);
                    result.push(']');
                }
                remaining = &after[end + 1..];
            } else {
                result.push('[');
                remaining = after;
            }
        }
        result.push_str(remaining);
        result
    }

    /// Generates an alphabetical index of all entry titles.
    fn generate_index(&self) -> String {
        let mut titles: Vec<&str> = self.entries.iter().map(|e| e.title.as_str()).collect();
        titles.sort_unstable();
        let mut idx = String::from("## Index\n\n");
        for t in titles {
            idx.push_str(&format!("- [{t}](#{})\n", slug(t)));
        }
        idx
    }

    /// Generates an HTML alphabetical index.
    fn generate_index_html(&self) -> String {
        let mut titles: Vec<&str> = self.entries.iter().map(|e| e.title.as_str()).collect();
        titles.sort_unstable();
        let mut html = String::from("<h2 id=\"index\">Index</h2>\n<ul>\n");
        for t in titles {
            html.push_str(&format!(
                "<li><a href=\"#{}\">{}</a></li>\n",
                slug(t),
                html_escape(t)
            ));
        }
        html.push_str("</ul>\n");
        html
    }
}

impl Default for ReferenceManual {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. TutorialBuilder — Progressive tutorials
// ═══════════════════════════════════════════════════════════════════════

/// Difficulty level for tutorials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Difficulty {
    /// For newcomers to the language.
    Beginner,
    /// Requires basic familiarity with Fajar Lang.
    Intermediate,
    /// Covers advanced language features.
    Advanced,
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Beginner => write!(f, "Beginner"),
            Self::Intermediate => write!(f, "Intermediate"),
            Self::Advanced => write!(f, "Advanced"),
        }
    }
}

/// A code block within a tutorial section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// Source code content.
    pub code: String,
    /// Language identifier for syntax highlighting.
    pub language: String,
    /// Optional caption describing the code block.
    pub caption: String,
    /// Whether this code block is runnable in the playground.
    pub runnable: bool,
}

impl CodeBlock {
    /// Creates a new code block.
    pub fn new(
        code: impl Into<String>,
        language: impl Into<String>,
        caption: impl Into<String>,
        runnable: bool,
    ) -> Self {
        Self {
            code: code.into(),
            language: language.into(),
            caption: caption.into(),
            runnable,
        }
    }

    /// Creates a runnable Fajar Lang code block.
    pub fn fj(code: impl Into<String>, caption: impl Into<String>) -> Self {
        Self::new(code, "fj", caption, true)
    }
}

/// An exercise for the reader in a tutorial section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exercise {
    /// The exercise prompt.
    pub prompt: String,
    /// An optional hint.
    pub hint: String,
    /// The reference solution.
    pub solution: String,
}

impl Exercise {
    /// Creates a new exercise.
    pub fn new(
        prompt: impl Into<String>,
        hint: impl Into<String>,
        solution: impl Into<String>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            hint: hint.into(),
            solution: solution.into(),
        }
    }
}

/// A section within a tutorial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TutorialSection {
    /// Section heading.
    pub heading: String,
    /// Prose explanation of the concept.
    pub explanation: String,
    /// Code blocks illustrating the concept.
    pub code_blocks: Vec<CodeBlock>,
    /// Exercises for hands-on practice.
    pub exercises: Vec<Exercise>,
}

impl TutorialSection {
    /// Creates a new tutorial section.
    pub fn new(heading: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
            explanation: explanation.into(),
            code_blocks: Vec::new(),
            exercises: Vec::new(),
        }
    }

    /// Adds a code block to this section.
    pub fn add_code_block(&mut self, block: CodeBlock) {
        self.code_blocks.push(block);
    }

    /// Adds an exercise to this section.
    pub fn add_exercise(&mut self, exercise: Exercise) {
        self.exercises.push(exercise);
    }
}

/// A single tutorial in the series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tutorial {
    /// Tutorial number (1-based).
    pub number: u32,
    /// Tutorial title.
    pub title: String,
    /// Difficulty level.
    pub difficulty: Difficulty,
    /// Ordered sections of the tutorial.
    pub sections: Vec<TutorialSection>,
}

impl Tutorial {
    /// Creates a new tutorial.
    pub fn new(number: u32, title: impl Into<String>, difficulty: Difficulty) -> Self {
        Self {
            number,
            title: title.into(),
            difficulty,
            sections: Vec::new(),
        }
    }

    /// Adds a section to this tutorial.
    pub fn add_section(&mut self, section: TutorialSection) {
        self.sections.push(section);
    }

    /// Returns the total number of exercises in the tutorial.
    pub fn exercise_count(&self) -> usize {
        self.sections.iter().map(|s| s.exercises.len()).sum()
    }
}

/// An ordered collection of tutorials forming a learning path.
#[derive(Debug, Clone)]
pub struct TutorialSeries {
    /// Tutorials in order.
    tutorials: Vec<Tutorial>,
}

impl TutorialSeries {
    /// Creates an empty tutorial series.
    pub fn new() -> Self {
        Self {
            tutorials: Vec::new(),
        }
    }

    /// Adds a tutorial to the series.
    ///
    /// Returns an error if the tutorial number is zero or duplicates
    /// an existing tutorial number.
    pub fn add_tutorial(&mut self, tutorial: Tutorial) -> Result<(), DocumentationError> {
        if tutorial.number == 0 {
            return Err(DocumentationError::InvalidTutorialNumber {
                number: 0,
                reason: "tutorial numbers must be >= 1".to_string(),
            });
        }
        if self.tutorials.iter().any(|t| t.number == tutorial.number) {
            return Err(DocumentationError::InvalidTutorialNumber {
                number: tutorial.number,
                reason: "duplicate tutorial number".to_string(),
            });
        }
        self.tutorials.push(tutorial);
        self.tutorials.sort_by_key(|t| t.number);
        Ok(())
    }

    /// Returns the number of tutorials in the series.
    pub fn len(&self) -> usize {
        self.tutorials.len()
    }

    /// Returns `true` if no tutorials have been added.
    pub fn is_empty(&self) -> bool {
        self.tutorials.is_empty()
    }

    /// Returns the tutorials as a slice.
    pub fn tutorials(&self) -> &[Tutorial] {
        &self.tutorials
    }

    /// Generates Markdown for all tutorials with navigation links.
    pub fn generate_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Fajar Lang Tutorial Series\n\n");

        for (i, tutorial) in self.tutorials.iter().enumerate() {
            self.render_tutorial_markdown(tutorial, i, &mut md);
        }
        md
    }

    /// Renders a single tutorial to Markdown with prev/next links.
    fn render_tutorial_markdown(&self, tutorial: &Tutorial, index: usize, md: &mut String) {
        md.push_str(&format!(
            "## Tutorial {}: {}\n\n",
            tutorial.number, tutorial.title
        ));
        md.push_str(&format!("*Difficulty: {}*\n\n", tutorial.difficulty));

        for section in &tutorial.sections {
            md.push_str(&format!("### {}\n\n", section.heading));
            md.push_str(&section.explanation);
            md.push_str("\n\n");

            for block in &section.code_blocks {
                if !block.caption.is_empty() {
                    md.push_str(&format!("*{}*\n\n", block.caption));
                }
                md.push_str(&format!("```{}\n{}\n```\n\n", block.language, block.code));
            }

            for (j, ex) in section.exercises.iter().enumerate() {
                md.push_str(&format!("**Exercise {}:** {}\n\n", j + 1, ex.prompt));
                if !ex.hint.is_empty() {
                    md.push_str(&format!(
                        "<details><summary>Hint</summary>{}</details>\n\n",
                        ex.hint
                    ));
                }
                md.push_str(&format!(
                    "<details><summary>Solution</summary>\n\n```fj\n{}\n```\n\n</details>\n\n",
                    ex.solution
                ));
            }
        }

        self.render_navigation(index, md);
        md.push_str("---\n\n");
    }

    /// Renders prev/next navigation links.
    fn render_navigation(&self, index: usize, md: &mut String) {
        let prev = if index > 0 {
            let p = &self.tutorials[index - 1];
            format!(
                "[< Previous: Tutorial {} - {}](#tutorial-{}-{})",
                p.number,
                p.title,
                p.number,
                slug(&p.title)
            )
        } else {
            String::new()
        };

        let next = if index + 1 < self.tutorials.len() {
            let n = &self.tutorials[index + 1];
            format!(
                "[Next: Tutorial {} - {} >](#tutorial-{}-{})",
                n.number,
                n.title,
                n.number,
                slug(&n.title)
            )
        } else {
            String::new()
        };

        if !prev.is_empty() || !next.is_empty() {
            md.push_str(&format!("{prev}  |  {next}\n\n"));
        }
    }
}

impl Default for TutorialSeries {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. DocEnhancer — Enhanced documentation generation
// ═══════════════════════════════════════════════════════════════════════

/// The kind of a documented item (extended from docgen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocItemKind {
    /// A standalone function.
    Function,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// A constant value.
    Constant,
    /// A type alias.
    TypeAlias,
    /// A module.
    Module,
    /// A method on a struct or trait.
    Method,
    /// A field in a struct.
    Field,
}

impl fmt::Display for DocItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Constant => write!(f, "constant"),
            Self::TypeAlias => write!(f, "type alias"),
            Self::Module => write!(f, "module"),
            Self::Method => write!(f, "method"),
            Self::Field => write!(f, "field"),
        }
    }
}

/// A documented item with full metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocItem {
    /// Fully qualified name (e.g., `std::math::abs`).
    pub name: String,
    /// What kind of item this is.
    pub kind: DocItemKind,
    /// The type/function signature.
    pub signature: String,
    /// Doc comment text (Markdown).
    pub doc_comment: String,
    /// Source file path.
    pub source_file: String,
    /// Line number in the source file.
    pub source_line: u32,
}

impl DocItem {
    /// Creates a new doc item.
    pub fn new(
        name: impl Into<String>,
        kind: DocItemKind,
        signature: impl Into<String>,
        doc_comment: impl Into<String>,
        source_file: impl Into<String>,
        source_line: u32,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            signature: signature.into(),
            doc_comment: doc_comment.into(),
            source_file: source_file.into(),
            source_line,
        }
    }

    /// Returns a URL-safe anchor for this item.
    pub fn anchor(&self) -> String {
        slug(&self.name)
    }
}

/// The type of cross-reference between two doc items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrossRefType {
    /// One item uses another (calls, references).
    Uses,
    /// A type implements a trait.
    ImplementedBy,
    /// A trait extends another trait.
    Extends,
    /// A module contains an item.
    Contains,
    /// A manually added "see also" link.
    SeeAlso,
}

impl fmt::Display for CrossRefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uses => write!(f, "uses"),
            Self::ImplementedBy => write!(f, "implemented by"),
            Self::Extends => write!(f, "extends"),
            Self::Contains => write!(f, "contains"),
            Self::SeeAlso => write!(f, "see also"),
        }
    }
}

/// A cross-reference link between two documented items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRef {
    /// Source item name.
    pub from_item: String,
    /// Target item name.
    pub to_item: String,
    /// Kind of reference.
    pub ref_type: CrossRefType,
}

impl CrossRef {
    /// Creates a new cross-reference.
    pub fn new(
        from_item: impl Into<String>,
        to_item: impl Into<String>,
        ref_type: CrossRefType,
    ) -> Self {
        Self {
            from_item: from_item.into(),
            to_item: to_item.into(),
            ref_type,
        }
    }
}

/// A JSON-serializable search index entry for client-side search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexEntry {
    /// Item name.
    pub name: String,
    /// Item kind as a string.
    pub kind: String,
    /// Module path.
    pub module_path: String,
    /// Short description (first sentence of doc comment).
    pub summary: String,
    /// Anchor link.
    pub anchor: String,
}

/// A search index for documentation (JSON-serializable).
#[derive(Debug, Clone)]
pub struct SearchIndex {
    /// All indexed entries.
    entries: Vec<SearchIndexEntry>,
}

impl SearchIndex {
    /// Creates an empty search index.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Builds a search index from a list of doc items.
    pub fn from_items(items: &[DocItem]) -> Self {
        let entries = items
            .iter()
            .map(|item| {
                let summary = first_sentence(&item.doc_comment);
                let module_path = extract_module_path(&item.name);
                SearchIndexEntry {
                    name: item.name.clone(),
                    kind: item.kind.to_string(),
                    module_path,
                    summary,
                    anchor: item.anchor(),
                }
            })
            .collect();
        Self { entries }
    }

    /// Searches the index for items matching the query (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&SearchIndexEntry> {
        let q = query.to_lowercase();
        let mut results: Vec<(&SearchIndexEntry, usize)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let name_lower = e.name.to_lowercase();
                if name_lower == q {
                    Some((e, 0))
                } else if name_lower.starts_with(&q) {
                    Some((e, 1))
                } else if name_lower.contains(&q) {
                    Some((e, 2))
                } else if e.summary.to_lowercase().contains(&q) {
                    Some((e, 3))
                } else {
                    None
                }
            })
            .collect();
        results.sort_by_key(|(_, score)| *score);
        results.into_iter().map(|(e, _)| e).collect()
    }

    /// Serializes the search index as a JSON string.
    pub fn to_json(&self) -> String {
        let mut json = String::from("[\n");
        for (i, entry) in self.entries.iter().enumerate() {
            json.push_str("  {\n");
            json.push_str(&format!(
                "    \"name\": \"{}\",\n",
                json_escape(&entry.name)
            ));
            json.push_str(&format!(
                "    \"kind\": \"{}\",\n",
                json_escape(&entry.kind)
            ));
            json.push_str(&format!(
                "    \"module\": \"{}\",\n",
                json_escape(&entry.module_path)
            ));
            json.push_str(&format!(
                "    \"summary\": \"{}\",\n",
                json_escape(&entry.summary)
            ));
            json.push_str(&format!(
                "    \"anchor\": \"{}\"\n",
                json_escape(&entry.anchor)
            ));
            json.push_str("  }");
            if i + 1 < self.entries.len() {
                json.push(',');
            }
            json.push('\n');
        }
        json.push(']');
        json
    }

    /// Returns the number of entries in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Documentation color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocTheme {
    /// Light background theme.
    Light,
    /// Dark background theme.
    Dark,
    /// Follow system preference via `prefers-color-scheme`.
    Auto,
}

impl DocTheme {
    /// Generates a CSS string for this theme.
    pub fn generate_css(&self) -> String {
        match self {
            Self::Light => light_theme_css().to_string(),
            Self::Dark => dark_theme_css().to_string(),
            Self::Auto => format!(
                "{}\n@media (prefers-color-scheme: dark) {{\n{}\n}}",
                light_theme_css(),
                dark_theme_css()
            ),
        }
    }
}

/// Navigation breadcrumb trail (e.g., `std > math > abs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbTrail {
    /// Ordered path segments (module > submodule > item).
    segments: Vec<BreadcrumbSegment>,
}

/// A single segment in a breadcrumb trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    /// Display label.
    pub label: String,
    /// URL or anchor link.
    pub href: String,
}

impl BreadcrumbTrail {
    /// Creates an empty breadcrumb trail.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Creates a breadcrumb trail from a qualified name like `std::math::abs`.
    pub fn from_qualified_name(name: &str) -> Self {
        let parts: Vec<&str> = name.split("::").collect();
        let mut segments = Vec::new();
        let mut path = String::new();

        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                path.push_str("::");
            }
            path.push_str(part);
            segments.push(BreadcrumbSegment {
                label: part.to_string(),
                href: format!("#{}", slug(&path)),
            });
        }

        Self { segments }
    }

    /// Renders the breadcrumb trail as HTML.
    pub fn to_html(&self) -> String {
        let parts: Vec<String> = self
            .segments
            .iter()
            .map(|s| {
                format!(
                    "<a href=\"{}\">{}</a>",
                    html_escape(&s.href),
                    html_escape(&s.label)
                )
            })
            .collect();
        format!("<nav class=\"breadcrumb\">{}</nav>", parts.join(" &gt; "))
    }

    /// Returns the number of segments.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Returns `true` if the trail is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl Default for BreadcrumbTrail {
    fn default() -> Self {
        Self::new()
    }
}

/// A deprecation notice for a doc item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationBanner {
    /// Version since which the item is deprecated.
    pub since_version: String,
    /// Suggested replacement item.
    pub replacement: String,
    /// Additional deprecation message.
    pub message: String,
}

impl DeprecationBanner {
    /// Creates a new deprecation banner.
    pub fn new(
        since_version: impl Into<String>,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            since_version: since_version.into(),
            replacement: replacement.into(),
            message: message.into(),
        }
    }

    /// Renders the banner as an HTML warning box.
    pub fn to_html(&self) -> String {
        let mut html =
            String::from("<div class=\"deprecation-banner\" style=\"background:#fff3cd;");
        html.push_str("padding:1em;border:1px solid #ffc107;border-radius:4px;\">\n");
        html.push_str(&format!(
            "<strong>Deprecated since {}</strong>",
            html_escape(&self.since_version)
        ));
        if !self.replacement.is_empty() {
            html.push_str(&format!(
                " - Use <code>{}</code> instead.",
                html_escape(&self.replacement)
            ));
        }
        if !self.message.is_empty() {
            html.push_str(&format!(" {}", html_escape(&self.message)));
        }
        html.push_str("\n</div>\n");
        html
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. PlaygroundCompiler — Browser playground support
// ═══════════════════════════════════════════════════════════════════════

/// Execution mode for the playground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunMode {
    /// Interpret the code and show output.
    Interpret,
    /// Format the code and return formatted version.
    Format,
    /// Type-check only, no execution.
    Check,
    /// Parse and return AST dump.
    Ast,
}

impl fmt::Display for RunMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interpret => write!(f, "interpret"),
            Self::Format => write!(f, "format"),
            Self::Check => write!(f, "check"),
            Self::Ast => write!(f, "ast"),
        }
    }
}

/// Configuration for the playground sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaygroundConfig {
    /// Maximum execution time in milliseconds.
    pub timeout_ms: u64,
    /// Maximum number of output lines.
    pub max_output_lines: usize,
    /// Maximum memory in megabytes.
    pub max_memory_mb: u64,
    /// Allowed language features.
    pub allowed_features: Vec<String>,
}

impl Default for PlaygroundConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            max_output_lines: 200,
            max_memory_mb: 64,
            allowed_features: vec![
                "std".to_string(),
                "math".to_string(),
                "collections".to_string(),
            ],
        }
    }
}

/// A request to execute code in the playground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaygroundRequest {
    /// Fajar Lang source code to execute.
    pub source_code: String,
    /// How to process the code.
    pub run_mode: RunMode,
}

impl PlaygroundRequest {
    /// Creates a new playground request.
    pub fn new(source_code: impl Into<String>, run_mode: RunMode) -> Self {
        Self {
            source_code: source_code.into(),
            run_mode,
        }
    }
}

/// The result of executing code in the playground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaygroundResponse {
    /// Standard output from execution.
    pub output: String,
    /// Error messages, if any.
    pub errors: Vec<String>,
    /// Execution time in milliseconds.
    pub execution_time_ms: u64,
    /// Approximate memory used in bytes.
    pub memory_used_bytes: u64,
}

impl PlaygroundResponse {
    /// Returns `true` if execution completed without errors.
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

/// A sandboxed execution environment for the playground.
#[derive(Debug, Clone)]
pub struct PlaygroundSandbox {
    /// Sandbox configuration.
    config: PlaygroundConfig,
}

impl PlaygroundSandbox {
    /// Creates a new sandbox with the given configuration.
    pub fn new(config: PlaygroundConfig) -> Self {
        Self { config }
    }

    /// Creates a sandbox with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(PlaygroundConfig::default())
    }

    /// Executes a playground request within the sandbox limits.
    ///
    /// This performs validation and simulated execution. In production,
    /// this would delegate to an actual interpreter with resource limits.
    pub fn execute(
        &self,
        request: &PlaygroundRequest,
    ) -> Result<PlaygroundResponse, DocumentationError> {
        if request.source_code.is_empty() {
            return Ok(PlaygroundResponse {
                output: String::new(),
                errors: vec!["empty source code".to_string()],
                execution_time_ms: 0,
                memory_used_bytes: 0,
            });
        }

        self.validate_source(&request.source_code)?;

        match request.run_mode {
            RunMode::Interpret => self.simulate_interpret(&request.source_code),
            RunMode::Format => Ok(self.simulate_format(&request.source_code)),
            RunMode::Check => Ok(self.simulate_check(&request.source_code)),
            RunMode::Ast => Ok(self.simulate_ast(&request.source_code)),
        }
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &PlaygroundConfig {
        &self.config
    }

    /// Validates source code against sandbox restrictions.
    fn validate_source(&self, source: &str) -> Result<(), DocumentationError> {
        let line_count = source.lines().count();
        if line_count > self.config.max_output_lines * 10 {
            return Err(DocumentationError::PlaygroundMemoryExceeded {
                max_mb: self.config.max_memory_mb,
            });
        }
        Ok(())
    }

    /// Simulates interpreting source code.
    fn simulate_interpret(&self, source: &str) -> Result<PlaygroundResponse, DocumentationError> {
        let line_count = source.lines().count() as u64;
        let simulated_time = line_count.saturating_mul(2);
        let simulated_mem = source.len() as u64 * 10;

        if simulated_time > self.config.timeout_ms {
            return Err(DocumentationError::PlaygroundTimeout {
                timeout_ms: self.config.timeout_ms,
            });
        }

        Ok(PlaygroundResponse {
            output: format!("// Executed {} lines successfully", line_count),
            errors: Vec::new(),
            execution_time_ms: simulated_time,
            memory_used_bytes: simulated_mem,
        })
    }

    /// Simulates formatting source code.
    fn simulate_format(&self, source: &str) -> PlaygroundResponse {
        PlaygroundResponse {
            output: source.to_string(),
            errors: Vec::new(),
            execution_time_ms: 1,
            memory_used_bytes: source.len() as u64 * 2,
        }
    }

    /// Simulates type checking source code.
    fn simulate_check(&self, source: &str) -> PlaygroundResponse {
        let line_count = source.lines().count();
        PlaygroundResponse {
            output: format!("// Type check passed ({line_count} lines)"),
            errors: Vec::new(),
            execution_time_ms: 1,
            memory_used_bytes: source.len() as u64 * 3,
        }
    }

    /// Simulates AST dump of source code.
    fn simulate_ast(&self, source: &str) -> PlaygroundResponse {
        let line_count = source.lines().count();
        PlaygroundResponse {
            output: format!("Program {{ statements: [{line_count} items] }}"),
            errors: Vec::new(),
            execution_time_ms: 1,
            memory_used_bytes: source.len() as u64 * 5,
        }
    }
}

/// Encodes/decodes source code as URL-safe base64 fragments.
#[derive(Debug, Clone)]
pub struct ShareEncoder;

impl ShareEncoder {
    /// Encodes source code into a URL-safe base64 string.
    pub fn encode(source: &str) -> String {
        // Simple URL-safe base64 encoding without padding
        let bytes = source.as_bytes();
        let mut result = String::new();
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

        let mut i = 0;
        while i + 2 < bytes.len() {
            let b0 = bytes[i] as u32;
            let b1 = bytes[i + 1] as u32;
            let b2 = bytes[i + 2] as u32;
            let triple = (b0 << 16) | (b1 << 8) | b2;

            result.push(alphabet[((triple >> 18) & 0x3f) as usize] as char);
            result.push(alphabet[((triple >> 12) & 0x3f) as usize] as char);
            result.push(alphabet[((triple >> 6) & 0x3f) as usize] as char);
            result.push(alphabet[(triple & 0x3f) as usize] as char);
            i += 3;
        }

        let remaining = bytes.len() - i;
        if remaining == 2 {
            let b0 = bytes[i] as u32;
            let b1 = bytes[i + 1] as u32;
            let triple = (b0 << 16) | (b1 << 8);
            result.push(alphabet[((triple >> 18) & 0x3f) as usize] as char);
            result.push(alphabet[((triple >> 12) & 0x3f) as usize] as char);
            result.push(alphabet[((triple >> 6) & 0x3f) as usize] as char);
        } else if remaining == 1 {
            let b0 = bytes[i] as u32;
            let triple = b0 << 16;
            result.push(alphabet[((triple >> 18) & 0x3f) as usize] as char);
            result.push(alphabet[((triple >> 12) & 0x3f) as usize] as char);
        }

        result
    }

    /// Decodes a URL-safe base64 string back to source code.
    ///
    /// Returns `None` if the input is not valid base64.
    pub fn decode(encoded: &str) -> Option<String> {
        let mut bytes = Vec::new();
        let chars: Vec<u8> = encoded.bytes().collect();

        let decode_char = |c: u8| -> Option<u32> {
            match c {
                b'A'..=b'Z' => Some((c - b'A') as u32),
                b'a'..=b'z' => Some((c - b'a' + 26) as u32),
                b'0'..=b'9' => Some((c - b'0' + 52) as u32),
                b'-' => Some(62),
                b'_' => Some(63),
                _ => None,
            }
        };

        let mut i = 0;
        while i + 3 < chars.len() {
            let a = decode_char(chars[i])?;
            let b = decode_char(chars[i + 1])?;
            let c = decode_char(chars[i + 2])?;
            let d = decode_char(chars[i + 3])?;
            let triple = (a << 18) | (b << 12) | (c << 6) | d;
            bytes.push(((triple >> 16) & 0xff) as u8);
            bytes.push(((triple >> 8) & 0xff) as u8);
            bytes.push((triple & 0xff) as u8);
            i += 4;
        }

        let remaining = chars.len() - i;
        if remaining == 3 {
            let a = decode_char(chars[i])?;
            let b = decode_char(chars[i + 1])?;
            let c = decode_char(chars[i + 2])?;
            let triple = (a << 18) | (b << 12) | (c << 6);
            bytes.push(((triple >> 16) & 0xff) as u8);
            bytes.push(((triple >> 8) & 0xff) as u8);
        } else if remaining == 2 {
            let a = decode_char(chars[i])?;
            let b = decode_char(chars[i + 1])?;
            let triple = (a << 18) | (b << 12);
            bytes.push(((triple >> 16) & 0xff) as u8);
        }

        String::from_utf8(bytes).ok()
    }

    /// Creates a full playground URL from source code.
    pub fn to_url(base_url: &str, source: &str) -> String {
        let encoded = Self::encode(source);
        format!("{base_url}?code={encoded}")
    }
}

/// A curated library of runnable example programs.
#[derive(Debug, Clone)]
pub struct ExampleLibrary {
    /// Examples organized by category.
    examples: Vec<PlaygroundExample>,
}

/// A single example in the playground library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaygroundExample {
    /// Example title.
    pub title: String,
    /// Category tag.
    pub category: String,
    /// Source code.
    pub source_code: String,
    /// Description of what the example demonstrates.
    pub description: String,
}

impl ExampleLibrary {
    /// Creates an empty example library.
    pub fn new() -> Self {
        Self {
            examples: Vec::new(),
        }
    }

    /// Creates a library pre-populated with standard examples.
    pub fn with_defaults() -> Self {
        let mut lib = Self::new();
        for example in default_basic_examples() {
            lib.add_example(example);
        }
        for example in default_advanced_examples() {
            lib.add_example(example);
        }
        lib
    }

    /// Adds an example to the library.
    pub fn add_example(&mut self, example: PlaygroundExample) {
        self.examples.push(example);
    }

    /// Returns all examples.
    pub fn examples(&self) -> &[PlaygroundExample] {
        &self.examples
    }

    /// Returns examples in a given category.
    pub fn by_category(&self, category: &str) -> Vec<&PlaygroundExample> {
        self.examples
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Returns all unique category names.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.examples.iter().map(|e| e.category.as_str()).collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }

    /// Returns the number of examples.
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// Returns `true` if the library is empty.
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }
}

impl Default for ExampleLibrary {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the basic default playground examples.
fn default_basic_examples() -> Vec<PlaygroundExample> {
    vec![
        PlaygroundExample {
            title: "Hello World".to_string(),
            category: "basics".to_string(),
            source_code: "fn main() {\n    println(\"Hello, Fajar Lang!\")\n}".to_string(),
            description: "A minimal Fajar Lang program.".to_string(),
        },
        PlaygroundExample {
            title: "Fibonacci".to_string(),
            category: "basics".to_string(),
            source_code: concat!(
                "fn fib(n: i64) -> i64 {\n",
                "    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }\n",
                "}\n\nfn main() {\n    println(fib(10))\n}"
            )
            .to_string(),
            description: "Recursive Fibonacci computation.".to_string(),
        },
        PlaygroundExample {
            title: "Pattern Matching".to_string(),
            category: "control-flow".to_string(),
            source_code: concat!(
                "enum Shape { Circle(f64), Rect(f64, f64) }\n\n",
                "fn area(s: Shape) -> f64 {\n",
                "    match s {\n        Circle(r) => 3.14159 * r * r,\n",
                "        Rect(w, h) => w * h,\n    }\n}"
            )
            .to_string(),
            description: "Using match expressions with enum variants.".to_string(),
        },
    ]
}

/// Returns the advanced default playground examples.
fn default_advanced_examples() -> Vec<PlaygroundExample> {
    vec![
        PlaygroundExample {
            title: "Tensor Operations".to_string(),
            category: "ml".to_string(),
            source_code: concat!(
                "@device\nfn inference() {\n",
                "    let x = zeros(3, 4)\n    let w = randn(4, 2)\n",
                "    let y = matmul(x, w)\n    let out = relu(y)\n",
                "    println(out)\n}"
            )
            .to_string(),
            description: "Basic tensor operations in @device context.".to_string(),
        },
        PlaygroundExample {
            title: "Async Channels".to_string(),
            category: "concurrency".to_string(),
            source_code: concat!(
                "fn main() {\n    let (tx, rx) = channel()\n",
                "    spawn(fn() { tx.send(42) })\n",
                "    let val = rx.recv()\n    println(val)\n}"
            )
            .to_string(),
            description: "Message passing between threads with channels.".to_string(),
        },
        PlaygroundExample {
            title: "Effects and Error Handling".to_string(),
            category: "effects".to_string(),
            source_code: concat!(
                "fn divide(a: f64, b: f64) -> Result<f64, str> {\n",
                "    if b == 0.0 { Err(\"division by zero\") }\n",
                "    else { Ok(a / b) }\n}\n\n",
                "fn main() {\n    let result = divide(10.0, 3.0)\n",
                "    match result {\n        Ok(v) => println(v),\n",
                "        Err(e) => eprintln(e),\n    }\n}"
            )
            .to_string(),
            description: "Result-based error handling with pattern matching.".to_string(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// 5. DocValidator — Documentation quality checker
// ═══════════════════════════════════════════════════════════════════════

/// Kinds of documentation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocIssueKind {
    /// The item has no doc comment at all.
    Missing,
    /// The item has an empty doc comment.
    Empty,
    /// The item has no code examples.
    NoExamples,
    /// A cross-reference link is broken.
    BrokenLink,
    /// The documentation refers to an outdated API version.
    OutdatedSince,
}

impl fmt::Display for DocIssueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "missing documentation"),
            Self::Empty => write!(f, "empty documentation"),
            Self::NoExamples => write!(f, "no code examples"),
            Self::BrokenLink => write!(f, "broken cross-reference link"),
            Self::OutdatedSince => write!(f, "outdated version reference"),
        }
    }
}

/// A single documentation quality issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocIssue {
    /// Name of the item with the issue.
    pub item_name: String,
    /// What kind of issue was found.
    pub issue_kind: DocIssueKind,
    /// Suggestion for how to fix it.
    pub suggestion: String,
}

impl DocIssue {
    /// Creates a new doc issue.
    pub fn new(
        item_name: impl Into<String>,
        issue_kind: DocIssueKind,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            item_name: item_name.into(),
            issue_kind,
            suggestion: suggestion.into(),
        }
    }
}

/// Documentation coverage statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct DocCoverage {
    /// Total number of documentable items.
    pub total_items: usize,
    /// Number of items with documentation.
    pub documented: usize,
    /// Number of items without documentation.
    pub undocumented: usize,
    /// Coverage percentage (0.0 to 100.0).
    pub coverage_pct: f64,
}

/// A module-level coverage report.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageReport {
    /// Per-module coverage data.
    pub modules: Vec<ModuleCoverage>,
    /// Overall coverage across all modules.
    pub overall: DocCoverage,
}

/// Coverage data for a single module.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCoverage {
    /// Module name.
    pub module_name: String,
    /// Coverage data for this module.
    pub coverage: DocCoverage,
}

/// Validates documentation quality for a set of doc items.
#[derive(Debug, Clone)]
pub struct DocValidator {
    /// Known item names for cross-reference checking.
    known_names: std::collections::HashSet<String>,
}

impl DocValidator {
    /// Creates a new validator with the given known names.
    pub fn new(known_names: &[String]) -> Self {
        Self {
            known_names: known_names.iter().cloned().collect(),
        }
    }

    /// Creates a validator from a list of doc items.
    pub fn from_items(items: &[DocItem]) -> Self {
        let names: Vec<String> = items.iter().map(|i| i.name.clone()).collect();
        Self::new(&names)
    }

    /// Validates all items and returns any issues found.
    pub fn validate(&self, items: &[DocItem]) -> Vec<DocIssue> {
        let mut issues = Vec::new();
        for item in items {
            self.check_item(item, &mut issues);
        }
        issues
    }

    /// Checks a single item for documentation issues.
    fn check_item(&self, item: &DocItem, issues: &mut Vec<DocIssue>) {
        if item.doc_comment.is_empty() {
            issues.push(DocIssue::new(
                &item.name,
                DocIssueKind::Missing,
                format!("Add a doc comment to `{}`", item.name),
            ));
            return;
        }

        if item.doc_comment.trim().is_empty() {
            issues.push(DocIssue::new(
                &item.name,
                DocIssueKind::Empty,
                format!("Add content to the doc comment for `{}`", item.name),
            ));
        }

        self.check_cross_refs(item, issues);
    }

    /// Checks for broken cross-references in doc comments.
    fn check_cross_refs(&self, item: &DocItem, issues: &mut Vec<DocIssue>) {
        let text = &item.doc_comment;
        let mut remaining = text.as_str();

        while let Some(start) = remaining.find('[') {
            let after = &remaining[start + 1..];
            if let Some(end) = after.find(']') {
                let ref_name = &after[..end];
                if !ref_name.is_empty()
                    && !ref_name.contains(' ')
                    && !self.known_names.contains(ref_name)
                {
                    issues.push(DocIssue::new(
                        &item.name,
                        DocIssueKind::BrokenLink,
                        format!("Reference [{ref_name}] could not be resolved"),
                    ));
                }
                remaining = &after[end + 1..];
            } else {
                break;
            }
        }
    }

    /// Computes documentation coverage for a set of items.
    pub fn coverage(&self, items: &[DocItem]) -> DocCoverage {
        let total = items.len();
        let documented = items
            .iter()
            .filter(|i| !i.doc_comment.trim().is_empty())
            .count();
        let undocumented = total - documented;
        let pct = if total > 0 {
            (documented as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        DocCoverage {
            total_items: total,
            documented,
            undocumented,
            coverage_pct: pct,
        }
    }

    /// Computes a coverage report grouped by module.
    pub fn coverage_report(&self, items: &[DocItem]) -> CoverageReport {
        let mut by_module: HashMap<String, Vec<&DocItem>> = HashMap::new();
        for item in items {
            let module = extract_module_path(&item.name);
            by_module.entry(module).or_default().push(item);
        }

        let mut modules: Vec<ModuleCoverage> = by_module
            .iter()
            .map(|(name, mod_items)| {
                let total = mod_items.len();
                let documented = mod_items
                    .iter()
                    .filter(|i| !i.doc_comment.trim().is_empty())
                    .count();
                let undocumented = total - documented;
                let pct = if total > 0 {
                    (documented as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                ModuleCoverage {
                    module_name: name.clone(),
                    coverage: DocCoverage {
                        total_items: total,
                        documented,
                        undocumented,
                        coverage_pct: pct,
                    },
                }
            })
            .collect();

        modules.sort_by(|a, b| a.module_name.cmp(&b.module_name));

        let overall = self.coverage(items);
        CoverageReport { modules, overall }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. SiteGenerator — Static site generation
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the documentation site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteConfig {
    /// Site title.
    pub title: String,
    /// Project version string.
    pub version: String,
    /// Base URL for links.
    pub base_url: String,
    /// Visual theme.
    pub theme: DocTheme,
    /// Google Analytics tracking ID (optional).
    pub analytics_id: Option<String>,
}

impl SiteConfig {
    /// Creates a new site configuration.
    pub fn new(
        title: impl Into<String>,
        version: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            base_url: base_url.into(),
            theme: DocTheme::Auto,
            analytics_id: None,
        }
    }
}

/// Table of contents entry for a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// Heading text.
    pub text: String,
    /// Heading level (1-6).
    pub level: u8,
    /// Anchor link.
    pub anchor: String,
}

/// A single page in the documentation site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// URL path for this page (e.g., `/api/std/math.html`).
    pub path: String,
    /// Page title.
    pub title: String,
    /// Full HTML content.
    pub content_html: String,
    /// Breadcrumb navigation.
    pub breadcrumbs: BreadcrumbTrail,
    /// Table of contents extracted from headings.
    pub toc: Vec<TocEntry>,
}

impl Page {
    /// Creates a new page.
    pub fn new(
        path: impl Into<String>,
        title: impl Into<String>,
        content_html: impl Into<String>,
    ) -> Self {
        let content = content_html.into();
        let toc = extract_toc_from_html(&content);
        Self {
            path: path.into(),
            title: title.into(),
            content_html: content,
            breadcrumbs: BreadcrumbTrail::new(),
            toc,
        }
    }

    /// Sets the breadcrumb trail for this page.
    pub fn with_breadcrumbs(mut self, breadcrumbs: BreadcrumbTrail) -> Self {
        self.breadcrumbs = breadcrumbs;
        self
    }
}

/// A sidebar navigation entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarEntry {
    /// Display label.
    pub label: String,
    /// Link target.
    pub href: String,
    /// Child entries.
    pub children: Vec<SidebarEntry>,
}

impl SidebarEntry {
    /// Creates a new sidebar entry.
    pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
            children: Vec::new(),
        }
    }

    /// Adds a child entry.
    pub fn add_child(&mut self, child: SidebarEntry) {
        self.children.push(child);
    }
}

/// Builds a static documentation site from pages.
#[derive(Debug, Clone)]
pub struct SiteBuilder {
    /// Site configuration.
    config: SiteConfig,
    /// Collected pages.
    pages: Vec<Page>,
    /// Sidebar navigation.
    sidebar: Vec<SidebarEntry>,
    /// Available versions for the version selector.
    versions: Vec<String>,
}

impl SiteBuilder {
    /// Creates a new site builder.
    pub fn new(config: SiteConfig) -> Self {
        let current = config.version.clone();
        Self {
            config,
            pages: Vec::new(),
            sidebar: Vec::new(),
            versions: vec![current],
        }
    }

    /// Adds a page to the site.
    pub fn add_page(&mut self, page: Page) {
        self.pages.push(page);
    }

    /// Adds a sidebar navigation entry.
    pub fn add_sidebar_entry(&mut self, entry: SidebarEntry) {
        self.sidebar.push(entry);
    }

    /// Adds a version to the version selector.
    pub fn add_version(&mut self, version: impl Into<String>) {
        self.versions.push(version.into());
    }

    /// Returns the number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Builds the site, returning all generated pages with full HTML.
    pub fn build(&self) -> Result<Vec<Page>, DocumentationError> {
        if self.pages.is_empty() {
            return Err(DocumentationError::SiteGenerationError {
                reason: "no pages to build".to_string(),
            });
        }

        let sidebar_html = self.render_sidebar();
        let version_selector = self.render_version_selector();
        let css = self.config.theme.generate_css();

        let mut output = Vec::with_capacity(self.pages.len());
        for page in &self.pages {
            let full_html = self.wrap_page(page, &sidebar_html, &version_selector, &css);
            output.push(Page {
                path: page.path.clone(),
                title: page.title.clone(),
                content_html: full_html,
                breadcrumbs: page.breadcrumbs.clone(),
                toc: page.toc.clone(),
            });
        }

        Ok(output)
    }

    /// Wraps a page in the full site layout.
    fn wrap_page(
        &self,
        page: &Page,
        sidebar_html: &str,
        version_selector: &str,
        css: &str,
    ) -> String {
        let analytics = self.render_analytics();
        let breadcrumbs = page.breadcrumbs.to_html();
        let toc_html = render_page_toc(&page.toc);

        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("  <meta charset=\"utf-8\">\n");
        html.push_str("  <meta name=\"viewport\" ");
        html.push_str("content=\"width=device-width, initial-scale=1\">\n");
        html.push_str(&format!(
            "  <title>{} - {}</title>\n",
            html_escape(&page.title),
            html_escape(&self.config.title)
        ));
        html.push_str(&format!("  <style>\n{css}\n  </style>\n"));
        html.push_str(&render_print_css());
        html.push_str(&analytics);
        html.push_str("</head>\n<body>\n");
        html.push_str("<div class=\"site-layout\">\n");
        html.push_str(&format!(
            "<header>\n<h1>{}</h1>\n{version_selector}\n</header>\n",
            html_escape(&self.config.title)
        ));
        html.push_str(&format!("<nav class=\"sidebar\">\n{sidebar_html}</nav>\n"));
        html.push_str("<main>\n");
        html.push_str(&breadcrumbs);
        html.push_str(&toc_html);
        html.push_str(&page.content_html);
        html.push_str("\n</main>\n");
        html.push_str("</div>\n</body>\n</html>\n");
        html
    }

    /// Renders the sidebar navigation.
    fn render_sidebar(&self) -> String {
        let mut html = String::from("<ul>\n");
        for entry in &self.sidebar {
            render_sidebar_entry(entry, &mut html);
        }
        html.push_str("</ul>\n");
        html
    }

    /// Renders the version selector dropdown.
    fn render_version_selector(&self) -> String {
        let mut html = String::from("<select class=\"version-selector\">\n");
        for v in &self.versions {
            let selected = if v == &self.config.version {
                " selected"
            } else {
                ""
            };
            html.push_str(&format!(
                "  <option value=\"{}\"{selected}>{}</option>\n",
                html_escape(v),
                html_escape(v)
            ));
        }
        html.push_str("</select>\n");
        html
    }

    /// Renders analytics snippet if configured.
    fn render_analytics(&self) -> String {
        match &self.config.analytics_id {
            Some(id) => format!(
                concat!(
                    "  <script async ",
                    "src=\"https://www.googletagmanager.com/gtag/js?id={}\">",
                    "</script>\n"
                ),
                html_escape(id)
            ),
            None => String::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Internal Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Converts a string to a URL-safe slug.
fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Escapes special HTML characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Escapes a string for JSON embedding.
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Extracts the first sentence from a doc comment.
fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(pos) = trimmed.find(". ") {
        trimmed[..pos + 1].to_string()
    } else if let Some(pos) = trimmed.find(".\n") {
        trimmed[..pos + 1].to_string()
    } else if trimmed.ends_with('.') {
        trimmed.to_string()
    } else {
        let line = trimmed.lines().next().unwrap_or(trimmed);
        line.to_string()
    }
}

/// Extracts the module path from a fully qualified name.
fn extract_module_path(name: &str) -> String {
    if let Some(pos) = name.rfind("::") {
        name[..pos].to_string()
    } else {
        "root".to_string()
    }
}

/// Returns the light theme CSS.
fn light_theme_css() -> &'static str {
    concat!(
        "body { font-family: system-ui, sans-serif; color: #333; ",
        "background: #fff; line-height: 1.6; }\n",
        "a { color: #0366d6; text-decoration: none; }\n",
        "a:hover { text-decoration: underline; }\n",
        "pre { background: #f6f8fa; padding: 1em; border-radius: 6px; ",
        "overflow-x: auto; }\n",
        "code { font-family: 'SFMono-Regular', monospace; font-size: 0.9em; }\n",
        ".sidebar { background: #f0f0f0; padding: 1em; }\n",
        ".site-layout { display: grid; grid-template-columns: 250px 1fr; gap: 2em; }\n",
        "header { grid-column: 1 / -1; border-bottom: 1px solid #eee; ",
        "padding-bottom: 1em; }\n",
        ".breadcrumb { color: #666; margin-bottom: 1em; }\n",
        ".version-selector { padding: 0.3em; }\n"
    )
}

/// Returns the dark theme CSS.
fn dark_theme_css() -> &'static str {
    concat!(
        "body { color: #c9d1d9; background: #0d1117; }\n",
        "a { color: #58a6ff; }\n",
        "pre { background: #161b22; }\n",
        ".sidebar { background: #161b22; }\n",
        "header { border-bottom-color: #30363d; }\n",
        ".breadcrumb { color: #8b949e; }\n"
    )
}

/// Renders a print-friendly stylesheet.
fn render_print_css() -> String {
    concat!(
        "  <style media=\"print\">\n",
        "    .sidebar, .version-selector, nav { display: none; }\n",
        "    body { color: #000; background: #fff; }\n",
        "    a { color: #000; text-decoration: underline; }\n",
        "    pre { border: 1px solid #ccc; }\n",
        "  </style>\n"
    )
    .to_string()
}

/// Renders a sidebar entry recursively.
fn render_sidebar_entry(entry: &SidebarEntry, html: &mut String) {
    html.push_str(&format!(
        "<li><a href=\"{}\">{}</a>",
        html_escape(&entry.href),
        html_escape(&entry.label)
    ));
    if !entry.children.is_empty() {
        html.push_str("\n<ul>\n");
        for child in &entry.children {
            render_sidebar_entry(child, html);
        }
        html.push_str("</ul>\n");
    }
    html.push_str("</li>\n");
}

/// Extracts a table of contents from HTML heading tags.
fn extract_toc_from_html(html: &str) -> Vec<TocEntry> {
    let mut toc = Vec::new();
    let mut remaining = html;

    while let Some(pos) = remaining.find("<h") {
        let after = &remaining[pos + 2..];
        if let Some(level_char) = after.chars().next() {
            if let Some(level) = level_char.to_digit(10) {
                if (1..=6).contains(&level) {
                    if let Some(close) = after.find('>') {
                        let content_start = &after[close + 1..];
                        if let Some(end_tag) = content_start.find("</h") {
                            let text = &content_start[..end_tag];
                            let clean_text = strip_html_tags(text);
                            toc.push(TocEntry {
                                text: clean_text.clone(),
                                level: level as u8,
                                anchor: slug(&clean_text),
                            });
                        }
                    }
                }
            }
        }
        remaining = &remaining[pos + 3..];
    }

    toc
}

/// Strips HTML tags from a string (simple implementation).
fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
}

/// Renders a page-level table of contents.
fn render_page_toc(toc: &[TocEntry]) -> String {
    if toc.is_empty() {
        return String::new();
    }
    let mut html = String::from("<nav class=\"page-toc\">\n<ul>\n");
    for entry in toc {
        let indent = "  ".repeat(entry.level as usize);
        html.push_str(&format!(
            "{indent}<li><a href=\"#{}\">{}</a></li>\n",
            html_escape(&entry.anchor),
            html_escape(&entry.text)
        ));
    }
    html.push_str("</ul>\n</nav>\n");
    html
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 17: ReferenceGenerator tests ──────────────────────────

    #[test]
    fn s17_1_reference_section_all_variants() {
        let sections = ReferenceSection::all();
        assert_eq!(sections.len(), 10);
        assert_eq!(sections[0], ReferenceSection::Types);
        assert_eq!(sections[9], ReferenceSection::Generics);
    }

    #[test]
    fn s17_2_reference_section_anchors() {
        assert_eq!(ReferenceSection::Types.anchor(), "types");
        assert_eq!(ReferenceSection::Concurrency.anchor(), "concurrency");
        assert_eq!(ReferenceSection::Ownership.anchor(), "ownership");
    }

    #[test]
    fn s17_3_reference_entry_creation_and_see_also() {
        let mut entry = ReferenceEntry::new(
            "Integer Types",
            ReferenceSection::Types,
            "Fajar Lang provides signed integer types: i8, i16, i32, i64, i128.",
        );
        entry.add_example(CodeExample::new(
            "Basic integers",
            "let x: i32 = 42",
            "42",
            "Declaring a 32-bit integer.",
        ));
        entry.add_see_also("Float Types");
        assert_eq!(entry.examples.len(), 1);
        assert_eq!(entry.see_also, vec!["Float Types"]);
        assert_eq!(entry.anchor(), "integer-types");
    }

    #[test]
    fn s17_4_reference_manual_add_and_query() {
        let mut manual = ReferenceManual::new();
        manual.add_entry(ReferenceEntry::new(
            "Booleans",
            ReferenceSection::Types,
            "The `bool` type has values `true` and `false`.",
        ));
        manual.add_entry(ReferenceEntry::new(
            "If Expressions",
            ReferenceSection::Expressions,
            "Use `if` for conditional logic.",
        ));
        assert_eq!(manual.entry_count(), 2);
        let type_entries = manual.entries_for_section(ReferenceSection::Types);
        assert_eq!(type_entries.len(), 1);
        assert_eq!(type_entries[0].title, "Booleans");
    }

    #[test]
    fn s17_5_reference_manual_generate_markdown_with_toc() {
        let mut manual = ReferenceManual::new();
        manual.add_entry(ReferenceEntry::new(
            "Variables",
            ReferenceSection::Statements,
            "Use `let` to declare variables.",
        ));
        manual.add_entry(ReferenceEntry::new(
            "Loops",
            ReferenceSection::Statements,
            "Use `while` or `for` for iteration.",
        ));
        let md = manual.generate_markdown();
        assert!(md.contains("# Fajar Lang Reference Manual"));
        assert!(md.contains("## Table of Contents"));
        assert!(md.contains("- [Statements](#statements)"));
        assert!(md.contains("### Variables"));
        assert!(md.contains("### Loops"));
        assert!(md.contains("## Index"));
    }

    #[test]
    fn s17_6_reference_manual_generate_html() {
        let mut manual = ReferenceManual::new();
        manual.add_entry(ReferenceEntry::new(
            "Structs",
            ReferenceSection::Types,
            "Define composite types with `struct`.",
        ));
        let html = manual.generate_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Fajar Lang Reference Manual"));
        assert!(html.contains("id=\"types\""));
        assert!(html.contains("id=\"structs\""));
    }

    #[test]
    fn s17_7_reference_cross_ref_resolution() {
        let mut manual = ReferenceManual::new();
        manual.add_entry(ReferenceEntry::new(
            "Option Type",
            ReferenceSection::Types,
            "An optional value.",
        ));
        manual.add_entry(ReferenceEntry::new(
            "Pattern Matching",
            ReferenceSection::Expressions,
            "Use match to destructure [Option Type] values.",
        ));
        let md = manual.generate_markdown();
        // The cross-ref [Option Type] should resolve to a link
        assert!(md.contains("[Option Type](#option-type)"));
    }

    #[test]
    fn s17_8_reference_manual_index_sorted() {
        let mut manual = ReferenceManual::new();
        manual.add_entry(ReferenceEntry::new(
            "Closures",
            ReferenceSection::Expressions,
            "Anonymous functions.",
        ));
        manual.add_entry(ReferenceEntry::new(
            "Arrays",
            ReferenceSection::Types,
            "Fixed-size collections.",
        ));
        let md = manual.generate_markdown();
        let index_pos = md.find("## Index").expect("index section");
        let index_section = &md[index_pos..];
        let arrays_pos = index_section.find("Arrays").expect("arrays in index");
        let closures_pos = index_section.find("Closures").expect("closures in index");
        assert!(arrays_pos < closures_pos, "index should be alphabetical");
    }

    #[test]
    fn s17_9_code_example_struct() {
        let ex = CodeExample::new(
            "Hello World",
            "println(\"hello\")",
            "hello",
            "Prints a greeting.",
        );
        assert_eq!(ex.title, "Hello World");
        assert_eq!(ex.code, "println(\"hello\")");
        assert_eq!(ex.expected_output, "hello");
    }

    #[test]
    fn s17_10_reference_manual_empty_section_skipped() {
        let mut manual = ReferenceManual::new();
        manual.add_entry(ReferenceEntry::new(
            "Channels",
            ReferenceSection::Concurrency,
            "Message passing between threads.",
        ));
        let md = manual.generate_markdown();
        // Should not contain sections with no entries
        assert!(!md.contains("## Types"));
        assert!(md.contains("## Concurrency"));
    }

    // ── Sprint 18: TutorialBuilder tests ─────────────────────────────

    #[test]
    fn s18_1_tutorial_creation() {
        let mut tut = Tutorial::new(1, "Getting Started", Difficulty::Beginner);
        assert_eq!(tut.number, 1);
        assert_eq!(tut.title, "Getting Started");
        assert_eq!(tut.difficulty, Difficulty::Beginner);
        tut.add_section(TutorialSection::new(
            "Installation",
            "Download and install Fajar Lang.",
        ));
        assert_eq!(tut.sections.len(), 1);
    }

    #[test]
    fn s18_2_difficulty_ordering() {
        assert!(Difficulty::Beginner < Difficulty::Intermediate);
        assert!(Difficulty::Intermediate < Difficulty::Advanced);
        assert_eq!(format!("{}", Difficulty::Advanced), "Advanced");
    }

    #[test]
    fn s18_3_tutorial_section_with_code_and_exercises() {
        let mut section = TutorialSection::new("Variables", "Learn about variable declarations.");
        section.add_code_block(CodeBlock::fj("let x = 42", "Declaring an integer"));
        section.add_exercise(Exercise::new(
            "Declare a mutable boolean variable set to true.",
            "Use `let mut`",
            "let mut flag: bool = true",
        ));
        assert_eq!(section.code_blocks.len(), 1);
        assert!(section.code_blocks[0].runnable);
        assert_eq!(section.exercises.len(), 1);
    }

    #[test]
    fn s18_4_tutorial_exercise_count() {
        let mut tut = Tutorial::new(2, "Control Flow", Difficulty::Beginner);
        let mut s1 = TutorialSection::new("If", "Conditional logic.");
        s1.add_exercise(Exercise::new("Write an if", "", "if true { 1 }"));
        s1.add_exercise(Exercise::new(
            "Write if-else",
            "",
            "if false { 0 } else { 1 }",
        ));
        let mut s2 = TutorialSection::new("While", "Loops.");
        s2.add_exercise(Exercise::new("Write a loop", "", "while true { break }"));
        tut.add_section(s1);
        tut.add_section(s2);
        assert_eq!(tut.exercise_count(), 3);
    }

    #[test]
    fn s18_5_tutorial_series_add_and_order() {
        let mut series = TutorialSeries::new();
        series
            .add_tutorial(Tutorial::new(2, "Functions", Difficulty::Beginner))
            .unwrap();
        series
            .add_tutorial(Tutorial::new(1, "Basics", Difficulty::Beginner))
            .unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series.tutorials()[0].number, 1);
        assert_eq!(series.tutorials()[1].number, 2);
    }

    #[test]
    fn s18_6_tutorial_series_reject_duplicate_number() {
        let mut series = TutorialSeries::new();
        series
            .add_tutorial(Tutorial::new(1, "Basics", Difficulty::Beginner))
            .unwrap();
        let result = series.add_tutorial(Tutorial::new(1, "Dupe", Difficulty::Beginner));
        assert!(result.is_err());
    }

    #[test]
    fn s18_7_tutorial_series_reject_zero_number() {
        let mut series = TutorialSeries::new();
        let result = series.add_tutorial(Tutorial::new(0, "Bad", Difficulty::Beginner));
        assert!(result.is_err());
        match result {
            Err(DocumentationError::InvalidTutorialNumber { number, .. }) => {
                assert_eq!(number, 0);
            }
            other => panic!("expected InvalidTutorialNumber, got {other:?}"),
        }
    }

    #[test]
    fn s18_8_tutorial_series_generate_markdown() {
        let mut series = TutorialSeries::new();
        let mut tut = Tutorial::new(1, "Hello World", Difficulty::Beginner);
        let mut section = TutorialSection::new("First Program", "Write your first program.");
        section.add_code_block(CodeBlock::fj(
            "fn main() { println(\"hello\") }",
            "Hello world",
        ));
        tut.add_section(section);
        series.add_tutorial(tut).unwrap();

        let md = series.generate_markdown();
        assert!(md.contains("# Fajar Lang Tutorial Series"));
        assert!(md.contains("## Tutorial 1: Hello World"));
        assert!(md.contains("*Difficulty: Beginner*"));
        assert!(md.contains("```fj"));
    }

    #[test]
    fn s18_9_tutorial_series_navigation_links() {
        let mut series = TutorialSeries::new();
        series
            .add_tutorial(Tutorial::new(1, "Basics", Difficulty::Beginner))
            .unwrap();
        series
            .add_tutorial(Tutorial::new(2, "Functions", Difficulty::Beginner))
            .unwrap();
        series
            .add_tutorial(Tutorial::new(3, "Structs", Difficulty::Intermediate))
            .unwrap();

        let md = series.generate_markdown();
        // Tutorial 2 should have both prev and next links
        assert!(md.contains("Previous: Tutorial 1"));
        assert!(md.contains("Next: Tutorial 3"));
    }

    #[test]
    fn s18_10_code_block_factory() {
        let block = CodeBlock::fj("let x = 1", "Simple variable");
        assert_eq!(block.language, "fj");
        assert!(block.runnable);
        assert_eq!(block.caption, "Simple variable");

        let non_runnable = CodeBlock::new("$ fj run hello.fj", "shell", "Running a program", false);
        assert!(!non_runnable.runnable);
        assert_eq!(non_runnable.language, "shell");
    }

    // ── Sprint 19: DocEnhancer + PlaygroundCompiler tests ────────────

    #[test]
    fn s19_1_doc_item_kind_display() {
        assert_eq!(format!("{}", DocItemKind::Function), "function");
        assert_eq!(format!("{}", DocItemKind::Module), "module");
        assert_eq!(format!("{}", DocItemKind::Method), "method");
        assert_eq!(format!("{}", DocItemKind::Field), "field");
        assert_eq!(format!("{}", DocItemKind::TypeAlias), "type alias");
    }

    #[test]
    fn s19_2_search_index_build_and_query() {
        let items = vec![
            DocItem::new(
                "std::math::abs",
                DocItemKind::Function,
                "fn abs(x: f64) -> f64",
                "Returns absolute value.",
                "stdlib/math.fj",
                10,
            ),
            DocItem::new(
                "std::math::sqrt",
                DocItemKind::Function,
                "fn sqrt(x: f64) -> f64",
                "Returns square root.",
                "stdlib/math.fj",
                20,
            ),
            DocItem::new(
                "std::collections::Array",
                DocItemKind::Struct,
                "struct Array<T>",
                "A dynamic array.",
                "stdlib/collections.fj",
                5,
            ),
        ];

        let index = SearchIndex::from_items(&items);
        assert_eq!(index.len(), 3);

        let results = index.search("abs");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "std::math::abs");

        let results2 = index.search("std");
        assert_eq!(results2.len(), 3);
    }

    #[test]
    fn s19_3_search_index_json_serialization() {
        let items = vec![DocItem::new(
            "std::io::println",
            DocItemKind::Function,
            "fn println(s: str)",
            "Prints a line.",
            "stdlib/io.fj",
            1,
        )];
        let index = SearchIndex::from_items(&items);
        let json = index.to_json();
        assert!(json.contains("\"name\": \"std::io::println\""));
        assert!(json.contains("\"kind\": \"function\""));
        assert!(json.contains("\"module\": \"std::io\""));
    }

    #[test]
    fn s19_4_breadcrumb_from_qualified_name() {
        let trail = BreadcrumbTrail::from_qualified_name("std::math::abs");
        assert_eq!(trail.len(), 3);
        let html = trail.to_html();
        assert!(html.contains("std"));
        assert!(html.contains("math"));
        assert!(html.contains("abs"));
        assert!(html.contains("&gt;"));
    }

    #[test]
    fn s19_5_deprecation_banner_html() {
        let banner = DeprecationBanner::new(
            "0.4.0",
            "new_api_fn",
            "This function will be removed in v1.0.",
        );
        let html = banner.to_html();
        assert!(html.contains("Deprecated since 0.4.0"));
        assert!(html.contains("new_api_fn"));
        assert!(html.contains("removed in v1.0"));
    }

    #[test]
    fn s19_6_cross_ref_struct() {
        let cr = CrossRef::new("Array", "Iterator", CrossRefType::ImplementedBy);
        assert_eq!(cr.from_item, "Array");
        assert_eq!(cr.to_item, "Iterator");
        assert_eq!(cr.ref_type, CrossRefType::ImplementedBy);
        assert_eq!(format!("{}", CrossRefType::Uses), "uses");
    }

    #[test]
    fn s19_7_playground_sandbox_execute_interpret() {
        let sandbox = PlaygroundSandbox::with_defaults();
        let req = PlaygroundRequest::new("let x = 42\nprintln(x)", RunMode::Interpret);
        let resp = sandbox.execute(&req).unwrap();
        assert!(resp.is_success());
        assert!(resp.output.contains("Executed"));
        assert!(resp.execution_time_ms > 0);
    }

    #[test]
    fn s19_8_playground_sandbox_empty_source() {
        let sandbox = PlaygroundSandbox::with_defaults();
        let req = PlaygroundRequest::new("", RunMode::Interpret);
        let resp = sandbox.execute(&req).unwrap();
        assert!(!resp.is_success());
        assert!(resp.errors[0].contains("empty"));
    }

    #[test]
    fn s19_9_share_encoder_roundtrip() {
        let source = "fn main() { println(\"Hello!\") }";
        let encoded = ShareEncoder::encode(source);
        assert!(!encoded.is_empty());
        assert!(!encoded.contains('=')); // no padding
        let decoded = ShareEncoder::decode(&encoded).unwrap();
        assert_eq!(decoded, source);
    }

    #[test]
    fn s19_10_example_library_defaults() {
        let lib = ExampleLibrary::with_defaults();
        assert!(lib.len() >= 6);
        let basics = lib.by_category("basics");
        assert!(basics.len() >= 2);
        let cats = lib.categories();
        assert!(cats.contains(&"basics"));
        assert!(cats.contains(&"ml"));
    }

    // ── Sprint 20: DocValidator + SiteGenerator tests ────────────────

    #[test]
    fn s20_1_doc_validator_missing_docs() {
        let items = vec![
            DocItem::new(
                "good_fn",
                DocItemKind::Function,
                "fn good_fn()",
                "Does something.",
                "src/lib.fj",
                1,
            ),
            DocItem::new(
                "bad_fn",
                DocItemKind::Function,
                "fn bad_fn()",
                "",
                "src/lib.fj",
                5,
            ),
        ];
        let validator = DocValidator::from_items(&items);
        let issues = validator.validate(&items);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].item_name, "bad_fn");
        assert_eq!(issues[0].issue_kind, DocIssueKind::Missing);
    }

    #[test]
    fn s20_2_doc_validator_broken_link() {
        let items = vec![DocItem::new(
            "my_fn",
            DocItemKind::Function,
            "fn my_fn()",
            "See [NonExistent] for details.",
            "src/lib.fj",
            1,
        )];
        let validator = DocValidator::from_items(&items);
        let issues = validator.validate(&items);
        assert!(
            issues
                .iter()
                .any(|i| i.issue_kind == DocIssueKind::BrokenLink)
        );
    }

    #[test]
    fn s20_3_doc_coverage_calculation() {
        let items = vec![
            DocItem::new("a", DocItemKind::Function, "", "Documented.", "a.fj", 1),
            DocItem::new(
                "b",
                DocItemKind::Function,
                "",
                "Also documented.",
                "a.fj",
                2,
            ),
            DocItem::new("c", DocItemKind::Function, "", "", "a.fj", 3),
        ];
        let validator = DocValidator::from_items(&items);
        let coverage = validator.coverage(&items);
        assert_eq!(coverage.total_items, 3);
        assert_eq!(coverage.documented, 2);
        assert_eq!(coverage.undocumented, 1);
        assert!((coverage.coverage_pct - 66.66).abs() < 1.0);
    }

    #[test]
    fn s20_4_coverage_report_by_module() {
        let items = vec![
            DocItem::new(
                "std::math::abs",
                DocItemKind::Function,
                "",
                "Returns absolute value.",
                "math.fj",
                1,
            ),
            DocItem::new(
                "std::math::sqrt",
                DocItemKind::Function,
                "",
                "",
                "math.fj",
                2,
            ),
            DocItem::new(
                "std::io::println",
                DocItemKind::Function,
                "",
                "Prints a line.",
                "io.fj",
                1,
            ),
        ];
        let validator = DocValidator::from_items(&items);
        let report = validator.coverage_report(&items);
        assert_eq!(report.modules.len(), 2);
        assert_eq!(report.overall.total_items, 3);
        assert_eq!(report.overall.documented, 2);
    }

    #[test]
    fn s20_5_site_config_creation() {
        let config = SiteConfig::new("Fajar Lang Docs", "0.5.0", "https://docs.fajarlang.dev");
        assert_eq!(config.title, "Fajar Lang Docs");
        assert_eq!(config.version, "0.5.0");
        assert_eq!(config.theme, DocTheme::Auto);
        assert!(config.analytics_id.is_none());
    }

    #[test]
    fn s20_6_site_builder_add_pages() {
        let config = SiteConfig::new("Docs", "1.0.0", "/");
        let mut builder = SiteBuilder::new(config);
        builder.add_page(Page::new("/index.html", "Home", "<h2>Welcome</h2>"));
        builder.add_page(Page::new("/api.html", "API", "<h2>API Reference</h2>"));
        assert_eq!(builder.page_count(), 2);
    }

    #[test]
    fn s20_7_site_builder_build_output() {
        let config = SiteConfig::new("Fajar Docs", "0.5.0", "/docs");
        let mut builder = SiteBuilder::new(config);
        builder.add_page(Page::new(
            "/index.html",
            "Home",
            "<h2>Welcome to Fajar Lang</h2><p>Docs here.</p>",
        ));
        let mut sidebar = SidebarEntry::new("API", "/api.html");
        sidebar.add_child(SidebarEntry::new("std::math", "/api/math.html"));
        builder.add_sidebar_entry(sidebar);
        builder.add_version("0.4.0");

        let pages = builder.build().unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].content_html.contains("<!DOCTYPE html>"));
        assert!(pages[0].content_html.contains("Fajar Docs"));
        assert!(pages[0].content_html.contains("version-selector"));
        assert!(pages[0].content_html.contains("0.5.0"));
        assert!(pages[0].content_html.contains("sidebar"));
    }

    #[test]
    fn s20_8_site_builder_empty_fails() {
        let config = SiteConfig::new("Empty", "1.0.0", "/");
        let builder = SiteBuilder::new(config);
        let result = builder.build();
        assert!(result.is_err());
        match result {
            Err(DocumentationError::SiteGenerationError { reason }) => {
                assert!(reason.contains("no pages"));
            }
            other => panic!("expected SiteGenerationError, got {other:?}"),
        }
    }

    #[test]
    fn s20_9_doc_theme_css_generation() {
        let light = DocTheme::Light.generate_css();
        assert!(light.contains("background: #fff"));

        let dark = DocTheme::Dark.generate_css();
        assert!(dark.contains("background: #0d1117"));

        let auto = DocTheme::Auto.generate_css();
        assert!(auto.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn s20_10_page_toc_extraction() {
        let html = "<h2>Introduction</h2><p>Text.</p><h3>Subsection</h3><p>More.</p>";
        let page = Page::new("/test.html", "Test", html);
        assert_eq!(page.toc.len(), 2);
        assert_eq!(page.toc[0].text, "Introduction");
        assert_eq!(page.toc[0].level, 2);
        assert_eq!(page.toc[1].text, "Subsection");
        assert_eq!(page.toc[1].level, 3);
    }
}
