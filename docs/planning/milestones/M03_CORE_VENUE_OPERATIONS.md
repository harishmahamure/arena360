# M03 — Arena360 Staff and core venue operations

## Outcome

Staff can run the gaming floor from the new Arena360 Staff product while the
new Cloud/Station contract enforces sessions reliably.

## In scope

- Live floor for PC, console, VR, simulator, arcade, and room station types.
- Player lookup/create/update and access status.
- Start, inspect, extend, end, and recover sessions where backend capability is
  approved; clearly mark unsupported pause/transfer/group actions.
- Station maintenance, provisioning status, device health, and kiosk diagnostics.
- Plans, eligible balances, explained selection, and session price/entitlement
  context.
- Shift start/close prerequisites needed to operate these journeys.

## Critical states

- Stale station state, duplicate start, customer already active, kiosk offline,
  delayed cleanup, insufficient entitlement, and staff override with reason.

## Non-goals

- No reservations, general wallet, loyalty, Edge authority, or hardware Hub.
- No hidden emulation of missing backend behavior in the frontend.

## Exit gate

A pilot venue can operate player arrival through station cleanup with Arena360
Staff and Station, with reconciled session/balance data, audit records, and the
new M00/M01 state models proven end to end.
