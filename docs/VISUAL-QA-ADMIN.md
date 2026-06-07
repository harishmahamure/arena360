# Admin UI visual QA checklist

Manual regression pass for the admin modernization track (M5–M6). No Playwright snapshots in scope per ADR-0005.

**Breakpoints:** 375px (mobile), 768px (tablet), 1280px (desktop)  
**Browsers:** Chrome and Safari minimum

## Auth

- [ ] Login — staff/admin toggle, dark shell

## Staff loop (375px critical)

- [ ] Staff dashboard — shift hero, quick actions 44px, ending-soon strip
- [ ] Sessions list — card layout, no horizontal scroll
- [ ] Session detail — sticky mobile actions, hero countdown
- [ ] POS — product grid, sticky cart, payment tiles, helper text on store/player/search
- [ ] Plan purchase — POS-style plan grid, sticky summary, payment tiles (matches product POS)
- [ ] Config form sample — Product or Plan create shows gray helper text under every field

## Owner / admin

- [ ] Admin dashboard — date toggle, stat grid, skeleton
- [ ] Sidebar — single accordion open, section labels
- [ ] List pages — ListPage pagination, filters preserved

## Shell

- [ ] AppBar title, shift chip, quick actions
- [ ] Shift handover dialog — reconciliation card, TOTP field

---

**Sign-off:** __________ **Date:** __________
