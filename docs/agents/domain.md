# Domain docs

## Layout and reading rules

This repo uses a single-context layout:

- `GLOSSARY.md` at the repo root defines domain vocabulary.
- `docs/adr/` holds architectural decision records.

Before exploring the codebase, read the glossary and ADRs relevant
to the area being explored.

If these files do not exist, proceed silently. Domain-modeling
creates them when terms or decisions are resolved.

## Vocabulary

Use glossary terms in issue titles, proposals, hypotheses, and
test names. If a needed concept is missing, reconsider the term
or note the gap for domain-modeling.

## Decision conflicts

Explicitly flag proposals that contradict an existing ADR.
Identify the ADR and explain why the decision merits reopening.
