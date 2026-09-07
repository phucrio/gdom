# GDOM Design Direction

Approved on 2026-09-07 through a 15-question interview in five rounds, with an additional approved requirement for recognizable file-type icons.

This document is the canonical end-user visual and interaction design contract. It does not select frameworks or component architecture. Security and ownership invariants in [AGENTS.md](../AGENTS.md) remain mandatory; engineering and accessibility requirements in [code-convention.md](code-convention.md) continue to apply.

## Experience intent and principles

GDOM should feel modern, approachable, and reassuring. Use balanced information density and restrained visual emphasis. Users should immediately understand which accounts are involved, what is happening, and whether action is required.

- Make source and target identities explicit. Never rely on an arrow or avatar color alone.
- Present ownership status from verified application state. Distinguish processed, succeeded, failed, and skipped items.
- Keep progress visible without covering the current task.
- Use consistent controls, readable hierarchy, and predictable placement.
- Explain consequences accurately. Cancellation must never imply that completed ownership transfers are reversed.
- Keep assets local and preserve all account, credential, and mutation boundaries.

## Layout, navigation, and responsive behavior

Retain the top header with Home and Jobs tabs and account switching. Use a full-width main content area with 24 px desktop padding.

Place persistent migration progress in a dedicated layout region that reserves space. Expanding progress details must not cover file rows or controls. Keep the expanded area bounded so the main content remains usable in short windows.

| Available width | Behavior |
|---|---|
| 900 px and above | Desktop arrangement with 24 px outer padding and aligned job metadata. |
| 640–899 px | Stack job metadata and wrap toolbars; preserve navigation and account access. |
| Below 640 px | Single-column arrangement with 16 px outer padding and wrapping actions. |

Keep essential names, account identities, status, and actions available at narrow widths. Confine necessary horizontal scrolling to data tables. Avoid competing page and panel scrollbars where one scroll region suffices. Long identifiers must wrap or expose their full value through an accessible interaction, not a hover-only tooltip.

## Semantic palette

Dark mode first, cool neutrals, and one indigo interaction accent. OKLCH values are canonical; hexadecimal values are rounded sRGB compatibility fallbacks. These are approved design targets, not a claim that current product styles already meet contrast requirements.

| Role | OKLCH | Hex fallback |
|---|---|---|
| Canvas | `oklch(0.208 0.042 265.755)` | `#0F172A` |
| Surface | `oklch(0.279 0.041 260.031)` | `#1E293B` |
| Elevated surface | `oklch(0.372 0.044 257.287)` | `#334155` |
| Control boundary | `oklch(0.554 0.046 257.417)` | `#64748B` |
| Primary text | `oklch(0.984 0.003 247.858)` | `#F8FAFC` |
| Secondary text | `oklch(0.869 0.022 252.894)` | `#CBD5E1` |
| Primary action fill | `oklch(0.457 0.240 277.023)` | `#4338CA` |
| Focus / accent text | `oklch(0.785 0.115 274.713)` | `#A5B4FC` |
| Success | `oklch(0.792 0.209 151.711)` | `#4ADE80` |
| Warning | `oklch(0.828 0.189 84.429)` | `#FBBF24` |
| Error | `oklch(0.704 0.191 22.216)` | `#F87171` |

Use primary text on the primary action fill. Reserve semantic colors for status communication, paired with labels or distinct icons. File-type colors are identification aids and must remain visually distinct from status indicators. Validate actual foreground/background combinations, including hover, focus, selection, and elevated surfaces.

## Typography

- Font stack: `"Segoe UI Variable", "Segoe UI", system-ui, sans-serif`. Do not request remote fonts.
- Balanced modular scale, ratio 1.25: 12.8, 16, 20, 25, and 31.25 px, corresponding to 0.8, 1, 1.25, 1.5625, and 1.953125 rem at a 16 px root.
- Use 16 px for body text and controls, 20 px for section headings, and 25 px for page or dialog titles. Reserve 31.25 px for exceptional introductory headings.
- Use 12.8 px only for supplementary metadata. Essential statuses and actions must remain comfortably readable.
- Body line height: 1.5. Heading line height: 1.25. Regular body weight: 400; headings and emphasis: 600.
- Use tabular numerals for progress counts. Preserve Vietnamese diacritics and support long account and file names.

## Density, geometry, and elevation

| Property | Contract |
|---|---|
| Spacing scale | 4, 8, 12, 16, 24, 32, 48 px |
| Small control radius | 4 px |
| Button and input radius | 6 px |
| Panel and dialog radius | 8 px |
| Typical control height | At least 36 px; grow with text and zoom |
| File row height | Approximately 44 px; grow when text wraps |
| Dialog padding | 24 px, adapting to narrow layouts |
| Base elevation | No shadow; subtle tonal separation |
| Menu elevation | `0 8px 24px #00000033` |
| Dialog elevation | `0 16px 48px #00000066` |

Use balanced density: compact file rows, with more breathing room in dialogs and job summaries. Apply subtle layered surfaces rather than shadows on every card. Controls that need a visible boundary must maintain sufficient contrast.

## Screen-specific requirements

| Surface | Required improvement |
|---|---|
| Drive browser | Consistent file-type icons, aligned columns, styled Refresh control, clear selection and keyboard focus. |
| Jobs | Separate source and target identities from status and counts. Wrap long content without pushing actions off-screen. |
| History details | Structured label/value layout, readable timestamps, 24 px padding, and one clearly placed Close action. |
| Migration progress | Reserved space, explicit source/target labels, readable outcome counts, consistent Pause/Resume and Cancel controls. |
| Account menu | Clear active-account indicator, readable identities, consistent actions, and a separated destructive action. |

## Iconography and imagery

Use a consistent outline icon family, normally 20 px with a consistent stroke. Keep imagery minimal, with small illustrations only where they help explain an empty state. Avoid decorative photography.

File-type icons must be recognizable at 20 px and distinguish common formats rather than showing the same generic document for every file.

| Category | Required coverage |
|---|---|
| PDF | `.pdf` |
| Archives | `.zip`, `.rar`, `.7z`, `.tar`, `.gz`, including compound archive names such as `.tar.gz` |
| Ebooks | `.epub`, `.mobi` |
| Documents | `.doc`, `.docx`, `.odt`, `.txt` |
| Markdown | `.md`, `.markdown` |
| Spreadsheets | `.xls`, `.xlsx`, `.ods`, `.csv` |
| Presentations | `.ppt`, `.pptx`, `.odp` |
| Images | Common raster and vector formats, including PNG, JPEG, GIF, WebP, SVG, and HEIC |
| Audio | Common formats, including MP3, WAV, FLAC, and M4A |
| Video | Common formats, including MP4, WebM, MOV, and MKV |
| Source code | Recognizable code icons for common programming, markup, and configuration files |
| Google-native items | Distinct Google Docs, Sheets, and Slides icons |
| Navigation items | Folders and shortcuts, distinguishable from regular files |
| Unknown formats | A consistent generic file icon |

Prefer specific MIME types. Fall back to case-insensitive extensions when metadata is generic or unavailable. Maintain a consistent family with restrained type colors and recognizable format marks where helpful. Color alone must not identify a format. Keep filenames and extensions readable; avoid duplicate screen-reader announcements when an icon repeats adjacent information.

Bundle assets locally. Do not send file metadata to external icon services. Reuse suitable existing assets where available and retain applicable asset license notices.

## Motion

Use snappy, restrained transitions of 100–150 ms with `ease-out`. Avoid movement on every progress update. Respect reduced-motion preferences by removing nonessential movement and using immediate state changes where appropriate.

## Feedback and state matrix

Use inline, persistent feedback with readable badges and supported inline recovery actions. Use dialogs for decisions requiring confirmation, rather than routine progress updates. User-requested history details may remain a read-only dialog.

| State | Presentation and behavior |
|---|---|
| Loading / scanning | Labeled activity indicator. Use indeterminate progress when the total is unknown. |
| Running | Persistent progress, separate outcome counts, and available controls. |
| Paused | Explicit label and supported resume action. |
| Rate-limited | Explicit label, reason and retry timing when known; never invent a countdown. |
| Completed | Persistent outcome summary; distinguish completion with failures. |
| Failed / disconnected | Inline explanation with a supported recovery action; preserve context. |
| Empty | Clear explanation and one relevant next action. |
| Invalid input | Inline field error associated with the input; retain entered values. |
| Action pending | Visible pending state; prevent duplicate submission without losing context. |
| Confirmation required | Focused dialog explaining the consequence and identifying affected accounts or job. |
| Cancelled | Explicit outcome with completed transfers retained in the summary; no implication of rollback. |

Display only actions supported by current application state. Announce meaningful progress milestones and state transitions accessibly without repeatedly interrupting screen readers.

## Accessibility requirements

- Meet WCAG 2.2 Level AA. Target Level AAA body-text contrast of 7:1; enforce at least 4.5:1 for normal text and 3:1 for large text and meaningful control boundaries.
- Provide visible keyboard focus, with a 2 px focus outline and 2 px offset where feasible; verify contrast on every surface.
- Support complete keyboard navigation, meaningful control names, semantic headings, and accessible table relationships.
- Dialogs contain keyboard focus, have accessible names, and return focus to their trigger on close. Closing a dialog must not silently confirm an action.
- Pair status colors with text or distinct icons. Do not rely on color, motion, or hover alone.
- Honor reduced motion and preserve operability at 200% zoom and narrow widths.
- Keep targets at least 24 by 24 CSS px, with typical interactive controls at least 36 px high.
- Keep overlays, progress regions, and sticky elements from obscuring focused controls.

## Do / Do not

| Do | Do not |
|---|---|
| Label source and target explicitly. | Rely on arrows or avatar colors to establish account roles. |
| Reserve layout space for persistent progress. | Float progress over file rows or modal actions. |
| Align labels, values, counts, and actions. | Pack identities, statuses, and counts into one unstructured string. |
| Use shared spacing and control styling. | Mix unstyled native-looking buttons with custom controls. |
| Use recognizable local file-type icons. | Fetch icons using private file metadata. |
| Report verified outcomes and supported recovery. | Imply completion, rollback, or retry capabilities that do not exist. |

## Acceptance checklist

- [ ] Verify Drive browser, Jobs, history details, account menu, and collapsed/expanded progress with synthetic data.
- [ ] Verify representative desktop sizes, a roughly 1100 by 750 px window, short windows, narrow layouts, and 200% zoom.
- [ ] No obscured controls, unintended clipping, duplicate Close buttons, or unstyled controls.
- [ ] Long names, Vietnamese diacritics, and account identities remain readable or accessible in full.
- [ ] Confirm PDF, ZIP, EPUB, RAR, DOCX, Markdown, and the other listed file categories render recognizable icons.
- [ ] Verify uppercase extensions, generic MIME types, unknown formats, folders, Google-native items, and shortcuts.
- [ ] Verify keyboard navigation, dialog focus restoration, focus visibility, and reduced motion.
- [ ] Measure contrast for actual text, control, focus, and semantic-state combinations.
- [ ] Verify loading, empty, pending, running, paused, rate-limited, completed, partial-failure, and disconnected presentations using mocks.
- [ ] Distinguish processed items from successful transfers and preserve accurate cancellation messaging.
- [ ] No live Drive mutations or external transmission of account/file data for visual testing.

These checks are implementation acceptance criteria; approval of this document does not mean they have already passed in the product.

## Retained and changed decisions

No previous `docs/DESIGN.md` existed at approval.

| Decision | Retained or changed direction |
|---|---|
| Theme and navigation | Retain dark mode, indigo accents, top navigation, and account switching. |
| Product personality | Modern and approachable, with balanced emphasis and density. |
| Typography | Humanist Windows system sans-serif with a balanced type scale. |
| Geometry and surfaces | Subtle 4–8 px rounding, layered surfaces, restrained overlay shadows. |
| Spacing and controls | Inconsistent spacing and controls → shared visual rules. |
| Job identities | Dense account strings → structured source and target identities. |
| Progress | Floating overlapping progress → reserved progress space. |
| History details | Loosely formatted modal → readable label/value layout and one Close action. |
| File icons | Generic or inconsistent representations → recognizable local file-type coverage. |
| Motion and feedback | Snappy transitions, inline persistent feedback, and supported recovery. |
| Security | Retain all existing token isolation, account routing, mutation, and privacy boundaries. |

## Implementation handoff

Use `ux-visual-interface` for detailed screen execution and `ux-design-systems` where shared visual rules need coordination. Keep this document as the source of design decisions. Product UI code changes and their runtime verification are a subsequent implementation task.

## Shared primitives

The account menu includes a Google storage summary for the active account: percentage, used/total bytes, and a native meter with the indigo accent. Use existing surface, text, radius, and spacing tokens (12 px padding, 8 px gap). Fetch on menu open and account change; show loading, retryable error, and unavailable-limit states without substituting zero usage. Storage is the account total across Google services, not only Drive files.

Use the existing semantic HTML and these reusable visual primitives across reachable product screens: primary, secondary, ghost, and danger buttons; field controls; status badges; notices; dialogs; file-type icons; account selection rows; job cards; and the reserved migration-progress region. Each primitive has default, hover, focus-visible, disabled, pending, error, and reduced-motion behavior where applicable. Do not introduce parallel one-off variants without updating this contract.
