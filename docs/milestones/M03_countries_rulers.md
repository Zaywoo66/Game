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

## Test scenario

The 1440 scenario loads six playable countries. Unassigned land has neutral
map color. Character life dates and succession candidates are authored
scenario data. A country can continue with a vacant throne if its roster
has no living candidate.

On the first day of each month, a country's taxable development is the sum
of its provinces' development plus the scenario baseline per province.
The game multiplies this by the per-development tax yield and the current
tax rate, applies country and ruler modifiers, and credits the result to
treasury. If planned spending would breach the debt floor, the treasury
pays the affordable amount and records bankruptcy. On a ruler's death,
the nominated living heir takes precedence. The fallback considers the
previous dynasty first, then age and character ID. A minor ruler begins a
regency that ends on reaching the scenario's age of adulthood.

The workspace autoplay test runs all six countries under AI control through
1 January 1541. It checks the treasury floor, fiscal and command errors,
ownership, living rulers and heirs, regency and succession, and compares
two replays. One replay is saved and loaded on 1 January 1490 before both
continue to the final hash comparison.
