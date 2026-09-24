# M08 — Hub and venue automation

## Outcome

Arena360 can safely automate a narrow console/TV station lifecycle through
Edge, with observable outcomes and manual fallback.

## In scope

- New Arena Hub hardware/firmware boundary and supported capability protocol.
- Secure pairing, device identity, assignment to stations, health, and updates.
- IR/HDMI-CEC/relay capability discovery and command result model.
- Idempotent session-start, warning, end, cleanup, and location-close actions.
- Versioned automation rules with triggers, conditions, actions, simulation,
  publish, disable, and history.
- Installer test mode, safe timeout, retry limits, circuit breaker, and manual
  override.

## Pilot slice

Start with one supported TV plus one console station: power on, select input,
confirm action, warn before end, power off, and recover from each failed step.

## Non-goals

- No broad consumer-device compatibility claim.
- No autonomous AI-created rules or unsafe console control.

## Exit gate

The pilot passes repeated lifecycle and fault-injection tests, wrong-device
actions are prevented, every command is attributable, and staff can always
recover manually.

