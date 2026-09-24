# Product decision log

Record decisions here when they affect more than one milestone or change the
reuse boundary. Detailed technical decisions may later move into ADR files.

| ID | Date | Status | Decision | Consequence |
| --- | --- | --- | --- | --- |
| PD-001 | 2026-09-24 | Proposed | Reuse selected `apps/backend` and `apps/kiosk` internals without preserving backward compatibility. | Existing APIs, schemas, events, workflows, routes, and UI contracts may be replaced; all other product surfaces are new builds. |
| PD-002 | 2026-09-24 | Proposed | Use Station, not PC, as the cross-product resource abstraction. | Backend contracts, UX language, Edge, and Hub mappings must converge on capabilities attached to a station. |
| PD-003 | 2026-09-24 | Proposed | Separate kiosk continuity from Edge offline authority. | Kiosk retains local enforcement while Edge owns venue-level offline business workflows and synchronization. |
| PD-004 | 2026-09-24 | Proposed | Treat unlimited plans as fair-use products with enforced record-creation and service limits. | Entitlements, quotas, policy, UX, metrics, and support processes ship together. |
| PD-005 | 2026-09-24 | Proposed | Build separate Arena360 Admin, Owner, Staff, Player Self-Service, and Marketing Website products. | Each surface has independent responsibilities and deployment boundaries while sharing the new domain language and identity platform. |
| PD-006 | 2026-09-24 | Proposed | Existing-data migration is delivered through a separate user-owned tool. | The new system publishes versioned import contracts but does not preserve legacy APIs, schemas, identifiers, or workflows. |

## Entry template

```text
ID:
Date:
Status: Proposed | Accepted | Superseded
Context:
Decision:
Alternatives:
Consequences:
Affected milestones:
Evidence:
```
