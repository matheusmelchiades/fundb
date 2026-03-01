# FunDB Brand Guide

> The cognitive database for the AI era

---

## Brand Personality

FunDB lives at a deliberate intersection: the name **"Fun"** signals approachability and developer joy, while the product is serious infrastructure — a Rust-built database with causal reasoning. This tension is the brand's defining characteristic.

| Association | Expression |
|---|---|
| **Intelligent** | Cognitive metaphors, neural/synaptic visual language |
| **Unified** | Convergence imagery, five-into-one motifs |
| **Approachable** | Warm colors, rounded forms, clear language |
| **Trustworthy** | Consistent typography, whitespace, Rust/PostgreSQL heritage |
| **Modern** | Gradient treatments, AI-era language |

**"Fun" means:** fun to use, fun to learn, fun to build with — not whimsical or unserious.

Visual reference: Stripe, Linear, Vercel (polished, developer-loved, quietly confident).

---

## Logo

### Primary: Synaptic Convergence

Five colored lines (representing the five data types) converge toward a central amber node (rounded square). This symbolizes unification — five databases becoming one.

| Variant | File | Usage |
|---|---|---|
| Full (light) | `assets/brand/logo-full.svg` | README, docs, light backgrounds |
| Full (dark) | `assets/brand/logo-dark.svg` | Dark backgrounds, presentations |
| Icon only | `assets/brand/logo-icon.svg` | Social avatars, app icon |
| Favicon | `assets/brand/logo-bracket.svg` | Browser tabs, small contexts |

### Wordmark

- **"Fun"** in Cognition Amber `#E8872B`
- **"DB"** in Midnight Indigo `#1A1A2E` (light mode) or Fog `#E5E7EB` (dark mode)
- Font: Inter Bold, letter-spacing: -0.02em

### Clear Space

Maintain a minimum clear space equal to the height of the central node icon on all sides. Do not place other elements within this zone.

### Do Not

- Stretch or distort the logo
- Change the logo colors
- Place the logo on busy/low-contrast backgrounds
- Add effects (drop shadows, outlines, bevels)
- Rotate the logo
- Use the wordmark without the icon in formal contexts

---

## Colors

### Primary

| Name | Hex | RGB | Usage |
|---|---|---|---|
| **Cognition Amber** | `#E8872B` | 232, 135, 43 | Main brand color. Buttons, links, primary actions, logo accent |
| **Deep Amber** | `#C46A15` | 196, 106, 21 | Hover states, pressed buttons. AA-compliant for small text on white (4.6:1) |

### Secondary

| Name | Hex | RGB | Usage |
|---|---|---|---|
| **Midnight Indigo** | `#1A1A2E` | 26, 26, 46 | Dark backgrounds, code blocks, text on light |
| **Slate Blue** | `#3D5A80` | 61, 90, 128 | Secondary text, borders, muted elements |
| **Warm Gray** | `#6B7280` | 107, 114, 128 | Body text, captions, disabled states |

### Data Type Accents

Each data type has a dedicated color used in diagrams, feature cards, and badges:

| Data Type | Name | Hex | RGB |
|---|---|---|---|
| Vectors | **Vector Violet** | `#7C3AED` | 124, 58, 237 |
| Graphs | **Graph Teal** | `#0D9488` | 13, 148, 136 |
| Documents | **Doc Blue** | `#2563EB` | 37, 99, 235 |
| Time-Series | **Time Rose** | `#E11D48` | 225, 29, 72 |
| Agent Memory | **Memory Gold** | `#D97706` | 217, 119, 6 |

### Neutrals

| Name | Hex | Usage |
|---|---|---|
| **Snow** | `#FAFAFA` | Light mode page background |
| **White** | `#FFFFFF` | Card surfaces, content containers |
| **Deep Night** | `#0F0F1A` | Dark mode background, hero sections |
| **Charcoal** | `#1E1E2E` | Dark mode cards, elevated surfaces |
| **Fog** | `#E5E7EB` | Light mode borders, dividers |
| **Dusk** | `#374151` | Dark mode borders |

### Brand Gradient

```css
background: linear-gradient(135deg, #E8872B 0%, #D97706 50%, #C46A15 100%);
```

Used in hero sections, CTA buttons, and the logo glow effect. Should feel warm and luminous.

### Accessibility

| Combination | Ratio | WCAG |
|---|---|---|
| Amber `#E8872B` on White `#FFFFFF` | 3.2:1 | AA large text (18pt+) |
| Deep Amber `#C46A15` on White `#FFFFFF` | 4.6:1 | AA all text |
| Amber `#E8872B` on Midnight `#1A1A2E` | 5.8:1 | AA all text |
| Midnight `#1A1A2E` on Snow `#FAFAFA` | 16:1 | AAA |

**Rule:** Use Deep Amber `#C46A15` instead of Cognition Amber for small text on white backgrounds.

---

## Typography

### Font Stack

| Role | Font | Weights | Fallback |
|---|---|---|---|
| **Headings** | Inter | Bold (700), SemiBold (600) | -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif |
| **Body** | Inter | Regular (400), Medium (500) | Same as above |
| **Code** | JetBrains Mono | Regular (400), Medium (500) | 'SF Mono', 'Fira Code', 'Cascadia Code', monospace |

### Hierarchy

| Level | Style | Size | Line-height |
|---|---|---|---|
| H1 | Inter Bold | 36px | 1.2 |
| H2 | Inter SemiBold | 28px | 1.3 |
| H3 | Inter SemiBold | 22px | 1.4 |
| H4 | Inter Medium | 18px | 1.4 |
| Body | Inter Regular | 16px | 1.6 |
| Body small | Inter Regular | 14px | 1.5 |
| Caption | Inter Regular | 12px | 1.4 |
| Code inline | JetBrains Mono | 14px | — |
| Code block | JetBrains Mono | 14px | 1.6 |

---

## Data Type Icons

Each of the five data types has a dedicated icon used across documentation, diagrams, and marketing materials.

| Data Type | File | Visual | Color |
|---|---|---|---|
| Vectors | `assets/brand/icon-vectors.svg` | Three arrows converging to a point | `#7C3AED` |
| Graphs | `assets/brand/icon-graphs.svg` | Three nodes connected in a triangle | `#0D9488` |
| Documents | `assets/brand/icon-documents.svg` | Stacked rectangles with curly brace | `#2563EB` |
| Time-Series | `assets/brand/icon-timeseries.svg` | Sine wave with data points | `#E11D48` |
| Agent Memory | `assets/brand/icon-memory.svg` | Brain outline with recall arrow | `#D97706` |

These icons appear together frequently in the "5 databases in 1" motif. They should never compete with the primary amber — they are supporting elements.

---

## Visual Language

### Icon Style

Duotone line icons: 2.5px stroke-weight, rounded line caps and joins. Primary stroke in the relevant color, secondary fill areas at 15-20% opacity.

### Core Visual Motifs

1. **Convergence Pattern** — Five lines flowing into one point. The core FunDB visual. Used in backgrounds, section dividers, transitions.
2. **Knowledge Glow** — Subtle radial gradient from amber to transparent, suggesting illuminated data.
3. **Layer Stack** — Overlapping translucent planes representing FunDB's layered architecture.

### Syntax Highlighting (FunQL)

| Token | Color |
|---|---|
| Keywords (SELECT, FROM, WHERE) | `#E8872B` |
| FunDB keywords (UNDERSTAND, REMEMBER, TRACE CAUSALITY) | `#D97706` bold |
| Strings | `#0D9488` |
| Numbers | `#7C3AED` |
| Comments | `#6B7280` |
| Functions | `#2563EB` |
| Operators | `#E11D48` |
| Background | `#1A1A2E` |

---

## README Badges

Use `flat-square` style with brand colors:

```markdown
![License](https://img.shields.io/badge/license-Apache--2.0-1A1A2E?style=flat-square&labelColor=E8872B&logoColor=white)
![Version](https://img.shields.io/badge/v0.1.0-cognitive%20database-1A1A2E?style=flat-square&labelColor=E8872B&logoColor=white)
![Rust](https://img.shields.io/badge/rust-1.85+-1A1A2E?style=flat-square&labelColor=C46A15&logo=rust&logoColor=white)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-wire%20compatible-1A1A2E?style=flat-square&labelColor=3D5A80&logo=postgresql&logoColor=white)
![Tests](https://img.shields.io/badge/tests-407%20passing-1A1A2E?style=flat-square&labelColor=0D9488)
```

---

## Brand Voice

| Dimension | Do | Don't |
|---|---|---|
| **Tone** | Confident, precise, warm | Arrogant, hype-driven, buzzwordy |
| **Technical** | Show real code, real architecture | Hide complexity behind marketing |
| **Comparisons** | "FunDB unifies what you need 5 databases for" | "FunDB kills [competitor]" |
| **Fun** | Express through craft quality and DX | Force jokes, excessive emojis |
| **AI language** | "Cognitive", "semantic", "context-aware" | "Revolutionary AI-powered", "magic" |

---

## Assets Inventory

```
assets/brand/
├── logo-full.svg          # Full logo (light mode)
├── logo-dark.svg          # Full logo (dark mode)
├── logo-icon.svg          # Icon only (social avatar, app icon)
├── logo-bracket.svg       # Favicon mark (F{)
├── icon-vectors.svg       # Vectors data type icon
├── icon-graphs.svg        # Graphs data type icon
├── icon-documents.svg     # Documents data type icon
├── icon-timeseries.svg    # Time-series data type icon
├── icon-memory.svg        # Agent memory data type icon
└── social-preview.svg     # GitHub social preview (1280x640)
```

---

## Color Competitive Map

FunDB's amber occupies unique territory among database brands:

| Database | Primary Color | Territory |
|---|---|---|
| Pinecone | `#1B998B` | Teal green |
| Neo4j | `#008CC1` | Blue |
| MongoDB | `#00ED64` | Green |
| InfluxDB | `#9B2AFF` | Purple |
| Redis | `#DC382D` | Red |
| **FunDB** | **`#E8872B`** | **Amber (unique)** |
