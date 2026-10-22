# 0005: Templates Are Data Over a Closed Set of Modules

Status: accepted

Date: 2026-10-22

## Context

Every workspace is organized the same way: twelve sections in one order, one ranking rule, one set of health checks and one set of assistant suggestions. A team that keeps runbooks and incidents does not need the same front door as a team that keeps code, or one that keeps contracts and policies.

Letting each team change the product's behaviour with code, scripts or free-form rules would put rights, ranking and stored evidence in the hands of whoever writes the rule.

## Decision

A workspace template is a JSON file that names modules RepoMemo already has and gives them an order, an emphasis or a weight. It is checked against closed vocabularies (sections, artifact types, health detectors, assistant capabilities, pulse cards, workspace icons); an unknown key or value makes the file invalid.

- Templates are built into the binary from `crates/templates/definitions/`, with an optional operator folder of the same format. No new dependency.
- A template never reads or changes a right, the AI policy, a quota or what leaves the server. Choosing one needs a workspace administrator.
- No template id appears in code: consumers read fields. A checker enforces it.
- `general` reproduces today's behaviour exactly, and every existing workspace stays on it until an administrator chooses.
- Organization-defined templates are not part of this decision.

## Consequences

- Adding a template is a data change, with a checker and a guide, not a code change.
- Ranking boosts are bounded (0.8–1.3 each, 1.5 together), so the best match for the words typed still comes first.
- The format grows only with a release that also reads the new key.
- A template cannot add a module; a new kind of behaviour needs code and a release as before.
