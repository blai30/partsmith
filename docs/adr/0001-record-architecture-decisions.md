# 0001: Record architecture decisions

Status: Accepted

## Context

Partsmith is written by AI agents with human approval at gates (technical design section 17). Agents start each task with a fresh context, so decisions that live only in a conversation are lost.

## Decision

Every lasting decision is an ADR in `docs/adr/`. Agents read the ADRs for an area before changing it and add a new ADR when they change a decision.

## Consequences

Decisions are reviewable in pull requests and discoverable by later agents. Superseded ADRs stay in place and point to their replacement.
