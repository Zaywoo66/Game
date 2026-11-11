# M3 Countries and rulers
Docs: SIM_RULES.md, DATA_FORMATS.md

## Tasks
- Country state: treasury, income/expenses, taxes, capital
- Character: stats ADM/DIP/MIL, traits (as modifiers), age, death, heir, dynasty
- Succession rules (basic), regency for minors
- Monthly economy tick; basic UI panels (country, ruler)

## Acceptance
- 100+ years of autoplay on the test region without invalid state (invariant checks)
- Ruler death triggers succession deterministically

## AI slice (required)
Docs: AI.md
- **AI L0**: AI countries manage taxes, spending, debt and handle succession/regency choices
- Profile data format loaded (AI profiles) with at least 2 archetypes
- Reason codes emitted for every AI command; debug ring buffer exists
- Acceptance add-on: AI-only autoplay of 50 years keeps all countries solvent or explainably bankrupt, no invalid states
