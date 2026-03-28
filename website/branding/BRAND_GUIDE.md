# Fajar Lang Brand Guide

> Version 1.0 | Last updated: 2026-03-28

This guide defines the visual and verbal identity of Fajar Lang. All community materials, conference slides, merchandise, documentation, and website assets should follow these guidelines.

---

## 1. Name and Wordmark

### Official Names

| Context | Name | Example |
|---------|------|---------|
| Full name | **Fajar Lang** | "Fajar Lang is a systems programming language..." |
| CLI / short form | **fj** | `fj run hello.fj`, "Install fj via cargo" |
| File extension | **.fj** | `main.fj`, `model.fj` |
| Package prefix | **fj-** | `fj-math`, `fj-nn`, `fj-hal` |
| OS projects | **FajarOS** | FajarOS Nova (x86_64), FajarOS Surya (ARM64) |

### Naming Rules

- Always capitalize "Fajar Lang" as two words with initial caps.
- Never write "FajarLang", "fajar-lang" (except in URLs/package names), or "FAJAR LANG".
- The CLI binary is always lowercase `fj`.
- In code contexts, use backticks: `fj`, `Fajar Lang`.
- When referring to the language in Indonesian contexts: "Bahasa Fajar" is acceptable.

### Text Monogram

The primary logo is a text-based monogram:

```
  ╭──────╮
  │  fj  │
  ╰──────╯
```

- **Typeface:** Monospaced (JetBrains Mono, Fira Code, or system monospace)
- **Weight:** Bold (700)
- **Minimum size:** 24px for digital, 12pt for print
- **Clear space:** At least 1x the height of the "f" character on all sides

When a graphical logo is unavailable, use the plain text `fj` in bold monospace as the identifier.

---

## 2. Color Palette

### Primary Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| **Fajar Blue** | `#0078D4` | (0, 120, 212) | Primary brand, links, buttons, headers |
| **Sunset Orange** | `#FF6B35` | (255, 107, 53) | Accents, CTAs, highlights, the "fajar" (dawn) |
| **Dark** | `#1B1B1B` | (27, 27, 27) | Text, code blocks, dark backgrounds |

### Secondary Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| **Success Green** | `#00A86B` | (0, 168, 107) | Pass indicators, positive states, confirmations |
| **Error Red** | `#DC3545` | (220, 53, 69) | Error messages, warnings, destructive actions |

### Neutral Palette

| Name | Hex | Usage |
|------|-----|-------|
| **White** | `#FFFFFF` | Backgrounds, inverted text |
| **Light Gray** | `#F5F5F5` | Code block backgrounds, cards |
| **Mid Gray** | `#6C757D` | Secondary text, borders |
| **Charcoal** | `#343A40` | Body text on light backgrounds |

### Color Usage Rules

- **Fajar Blue** is the dominant brand color. Use it for navigation, primary buttons, and headings.
- **Sunset Orange** is the accent. Use sparingly for emphasis: call-to-action buttons, important badges, or the "dawn" theme in hero sections.
- Never place Sunset Orange text on Fajar Blue backgrounds (insufficient contrast).
- Code snippets always use **Dark** (`#1B1B1B`) background with light text.
- Error messages use **Error Red** text or background, never for decorative purposes.
- Maintain WCAG 2.1 AA contrast ratio (4.5:1 for normal text, 3:1 for large text).

---

## 3. Typography

### Font Stack

Fajar Lang uses a system font stack for performance and consistency:

```css
/* Primary (UI text) */
font-family: 'Inter', system-ui, -apple-system, 'Segoe UI', Roboto,
             'Helvetica Neue', Arial, sans-serif;

/* Code / monospace */
font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code',
             'SF Mono', Menlo, Consolas, monospace;
```

### Type Scale

| Element | Size | Weight | Line Height |
|---------|------|--------|-------------|
| H1 (page title) | 2.5rem (40px) | 700 | 1.2 |
| H2 (section) | 2rem (32px) | 600 | 1.25 |
| H3 (subsection) | 1.5rem (24px) | 600 | 1.3 |
| Body | 1rem (16px) | 400 | 1.6 |
| Code inline | 0.875rem (14px) | 400 | 1.5 |
| Code block | 0.875rem (14px) | 400 | 1.6 |
| Caption | 0.8125rem (13px) | 400 | 1.5 |

### Typography Rules

- Body text: **Charcoal** (`#343A40`) on white, or **White** on **Dark**.
- Headings: **Dark** (`#1B1B1B`) or **Fajar Blue** (`#0078D4`).
- Code: Always monospaced. Inline code uses a light background tint.
- Never use more than 3 font weights on a single page.

---

## 4. Voice and Tone

### Brand Voice

Fajar Lang's voice is **technical, honest, and approachable**:

- **Technical:** We speak precisely about compiler internals, type systems, and hardware. No hand-waving.
- **Honest:** We document what works and what doesn't. See GAP_ANALYSIS for our approach to transparency.
- **Approachable:** We explain complex concepts without condescension. Everyone starts somewhere.

### Writing Guidelines

- Use active voice: "The compiler enforces isolation" not "Isolation is enforced by the compiler."
- Be specific: "292,000 lines of Rust" not "a large codebase."
- Avoid marketing superlatives: say "fast compilation" not "blazingly fast."
- Technical claims must be verifiable: link to benchmarks, tests, or source code.
- Use "we" for the project, "you" for the reader.

---

## 5. Merchandise Guidelines

### Approved Merchandise Items

- T-shirts (front: `fj` monogram, back: tagline or code snippet)
- Stickers (die-cut `fj` monogram, hexagonal format for laptop grids)
- Mugs (monogram on one side, short code snippet on the other)
- Pins (enamel, `fj` in Fajar Blue on white)

### Merchandise Design Rules

- The `fj` monogram must always be legible at the printed size.
- Use only the defined color palette. No gradients on the monogram.
- Code snippets on merchandise must be valid Fajar Lang syntax.
- Include the URL `fajarlang.dev` on all physical merchandise (small, bottom/back).
- Approved taglines for merchandise:
  - "One Language. ML + OS."
  - "@kernel + @device"
  - "If it compiles, it's safe to deploy."
  - "fn main() { fajar() }"

### Prohibited Uses

- Do not stretch, rotate, or skew the monogram.
- Do not change the monogram's letter spacing.
- Do not place the monogram on busy photographic backgrounds.
- Do not create alternate color versions not in the defined palette.

---

## 6. Do's and Don'ts

### Do

- Use the official color palette consistently across all materials.
- Maintain clear space around the `fj` monogram.
- Use monospaced fonts for all code examples.
- Credit the project as "Fajar Lang" in written references.
- Follow the type scale for consistent hierarchy.
- Link to `fajarlang.dev` as the canonical URL.

### Don't

- Don't use the Fajar Lang name to imply official endorsement without permission.
- Don't modify the monogram (no shadows, outlines, or 3D effects).
- Don't use Sunset Orange as a background color for large areas (it's an accent).
- Don't mix the Fajar Lang brand with other project logos in a way that implies partnership.
- Don't use low-contrast color combinations (e.g., orange text on blue).
- Don't use the brand assets in projects that promote unsafe or malicious software.

---

## 7. Asset Checklist

| Asset | Format | Location |
|-------|--------|----------|
| Monogram (text) | SVG, PNG | `website/branding/logo/` |
| Color palette | CSS variables | `website/branding/colors.css` |
| Font stack | CSS | Defined in this guide |
| Sticker template | SVG | `website/branding/stickers/` |
| Slide template | PDF/PPTX | `community/slides/` |
| Social banner | PNG (1200x630) | `website/branding/social/` |

> **Note:** Graphical assets will be added as the project grows. This guide establishes the rules; asset files will follow.

---

*Brand Guide v1.0 -- Fajar Lang Project*
