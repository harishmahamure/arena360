# Arena360 — Complete Product Vision

## 1. Vision

Arena360 is the operating system for modern gaming cafés and gaming venues.

The goal is to manage the entire venue from one platform:

* Gaming PCs
* PlayStation and Xbox stations
* TVs and displays
* Sessions and billing
* Customers and memberships
* Wallets and payments
* POS and food ordering
* Inventory
* Staff and shifts
* Reservations
* Loyalty
* Pricing
* Hardware automation
* Analytics
* Multi-location operations
* Local/on-premise deployments

Arena360 should move gaming cafés away from fragmented tools, manual timers, spreadsheets, switches, remote controls and disconnected POS systems.

The long-term product promise is:

> Run, automate and optimize an entire gaming venue from one platform.

---

# 2. The problem Arena360 solves

Gaming cafés typically operate several independent systems.

A café may have:

* PC billing software
* A separate POS
* Excel for accounting
* WhatsApp for reservations
* Separate customer membership records
* Manual console timers
* TV remotes
* Manual HDMI switching
* Staff-operated session starts
* UPI screenshots
* Manual shift reconciliation
* Manual inventory
* No reliable utilization analytics

This produces operational problems.

Examples:

* Staff forget to end sessions.
* TVs stay powered on unnecessarily.
* Customers occupy stations after time expires.
* Different staff charge different rates.
* Membership discounts are applied incorrectly.
* Cash reconciliation is difficult.
* Owners cannot see true station profitability.
* Low-demand periods remain underutilized.
* Console management requires employee intervention.
* Internet outages interrupt cloud-only systems.
* Owners cannot manage multiple branches centrally.

Arena360 connects these workflows into one system.

---

# 3. Core product philosophy

Arena360 should follow five principles.

## 3.1 Venue-first

Arena360 is not just PC café software.

It should support:

* Gaming PCs
* PS4
* PS5
* Xbox
* Racing simulators
* VR stations
* Arcade machines
* Private gaming rooms
* Streaming stations
* Tournament areas

The underlying concept should be a:

**Resource / Station**

rather than a PC-specific model.

Example:

```text
Station
│
├── PC
├── PS5
├── Xbox
├── VR
├── Simulator
└── Private Room
```

Each station has:

* capability
* pricing
* availability
* hardware controls
* session state
* maintenance state

---

# 4. Arena360 product architecture

Arena360 should eventually consist of six major products.

```text
                         ARENA360

                            │
          ┌─────────────────┼──────────────────┐
          │                 │                  │
       Cloud             Edge               Apps
          │                 │                  │
    Management         Local control      Customer
    Billing            Offline mode        Owner
    Analytics          Automation          Staff
          │
          ▼
        Agent
          │
     Gaming PCs

          +
          
       Arena Hub
          │
     Hardware control
```

The components are:

1. Arena360 Cloud
2. Arena360 Edge
3. Arena360 Agent
4. Arena360 Hub
5. Arena360 Customer App
6. Arena360 Intelligence

---

# 5. Arena360 Cloud

Arena360 Cloud is the control plane.

It manages the overall business.

Core areas:

## Venue management

* Organization
* Locations
* Floors/zones
* Stations
* Devices
* Pricing
* Employees
* customers

Example:

```text
Organization
│
├── Location: Pune
│   ├── PC Zone
│   ├── Console Zone
│   └── VIP Zone
│
├── Location: Mumbai
│
└── Location: Bengaluru
```

---

# 6. Station management

Every gaming resource becomes a station.

Station states could include:

```text
AVAILABLE
RESERVED
STARTING
ACTIVE
PAUSED
ENDING
CLEANUP
MAINTENANCE
OFFLINE
```

Each station should expose:

* current customer
* active session
* remaining time
* price
* hardware status
* network status
* agent version
* booking status

Owners should get a real-time visual floor view.

Example:

```text
PC01   AVAILABLE
PC02   01:42 remaining
PC03   00:14 remaining
PC04   OFFLINE

PS01   RESERVED 8:00 PM
PS02   ACTIVE 00:52
```

---

# 7. Session management

Sessions are at the center of Arena360.

Support:

* prepaid
* postpaid
* open-ended
* package-based
* membership-based
* booking-based
* promotional sessions

Session controls:

* start
* pause
* resume
* extend
* transfer station
* terminate
* add time
* add customer
* multiplayer session
* group session

Example:

```text
Customer
   ↓
Select station
   ↓
Choose duration
   ↓
Pricing calculated
   ↓
Payment/wallet
   ↓
Session starts
   ↓
Station unlocks
   ↓
Hardware activates
```

---

# 8. Pricing engine

Pricing should become one of Arena360's strongest differentiators.

Instead of storing a single hourly rate, Arena360 should support a pricing rules engine.

Base:

```text
PC Premium
₹60/hour
```

Then rules:

```text
07:00–12:00
× 0.80

18:00–23:00
× 1.10

23:00–07:00
× 1.25
```

Additional dimensions:

* weekday/weekend
* holidays
* station type
* customer membership
* occupancy
* booking source
* duration
* group size
* promotion
* location

Conceptually:

```text
Final Rate =
Base Rate
× Time Rule
× Demand Rule
× Membership Rule
× Promotion Rule
```

Pricing needs transparent explanations.

Example:

```text
Base                ₹60
Morning discount    -₹12
Gold member         -₹5
------------------------
Final               ₹43/hr
```

---

# 9. Membership platform

Memberships should be first-class entities.

Support:

* monthly subscription
* prepaid hours
* recurring hours
* time-window memberships
* device-specific membership
* location-specific membership
* organization-wide membership

Examples:

```text
Gold
₹599/month

10% gaming discount
2 free hours/month
Priority booking
```

Another:

```text
Night Gamer
₹999/month

30 hours
11 PM–7 AM only
```

Another:

```text
Student Morning
₹499/month

30% discount
Mon–Fri
7 AM–12 PM
```

Support:

* expiry
* renewal
* carry-forward rules
* usage limits
* auto-renew
* upgrade/downgrade

---

# 10. Customer wallet

Arena360 should include a prepaid wallet.

Capabilities:

* cash recharge
* UPI recharge
* card recharge
* promotional credits
* refunds
* bonuses
* wallet expiry policies
* transaction history

Separate:

```text
Cash Balance
Promotional Balance
Membership Credits
Loyalty Points
```

Avoid merging everything into one opaque balance.

---

# 11. Loyalty system

Owners should be able to configure loyalty rules.

Example:

```text
₹100 spend
=
10 Arena Points
```

Rewards:

* gaming time
* food
* membership discounts
* coupons
* merchandise

Also support:

* referral bonuses
* birthday offers
* win-back offers
* first-visit bonuses

---

# 12. Customer self-service

Arena360 should reduce dependence on staff.

A customer should eventually be able to:

```text
Scan QR
   ↓
Login
   ↓
Choose station
   ↓
Choose duration
   ↓
Pay
   ↓
Start session
```

Customer app features:

* registration
* OTP login
* wallet
* membership
* bookings
* station availability
* recharge
* QR check-in
* food ordering
* loyalty
* session history
* offers

This enables near-unattended café operation.

---

# 13. Reservations

Reservations should work for:

* PC
* console
* VR
* sim
* private room

Booking flow:

```text
Location
  ↓
Station type
  ↓
Date/time
  ↓
Seat
  ↓
Duration
  ↓
Deposit/payment
  ↓
Reservation
```

Features:

* partial deposits
* cancellation rules
* no-show policy
* booking grace period
* auto-release
* QR check-in

---

# 14. Arena360 POS

POS should be tightly integrated with gaming sessions.

Products:

* snacks
* beverages
* meals
* peripherals
* merchandise

Example:

```text
Session
₹180

Coke
₹40

Sandwich
₹120

----------------
Total
₹340
```

Support:

* cash
* UPI
* card
* wallet
* split payments

---

# 15. Kitchen and inventory

Later versions should support:

* KOT
* kitchen display
* recipe ingredients
* stock deduction
* low-stock alerts
* purchase entries
* wastage
* vendors
* stock transfers

Example:

```text
Chicken Burger
│
├── Bun
├── Patty
├── Cheese
└── Sauce
```

Selling one burger automatically updates stock.

---

# 16. Staff management

Support granular RBAC.

Roles could include:

```text
Owner
Manager
Cashier
Floor Staff
Technician
Kitchen Staff
Accountant
```

Permissions should exist at action level.

Examples:

* session override
* refund
* discount
* wallet adjustment
* cash drawer
* pricing edits
* membership changes
* inventory adjustment

---

# 17. Shift management

Every cashier/staff shift should have reconciliation.

Example:

```text
Opening cash    ₹5,000

Cash sales      ₹8,450
UPI            ₹12,200
Card            ₹3,000

Refunds           ₹500

Expected cash   ₹12,950
Actual cash     ₹12,900

Variance           -₹50
```

Provide audit trails for discrepancies.

---

# 18. Arena360 Agent

The PC Agent should be lightweight and reliable.

Responsibilities:

* machine registration
* lock screen
* unlock
* session enforcement
* remaining-time alerts
* logout
* restart
* shutdown
* heartbeat
* application status
* hardware health
* remote commands
* version updates

Future capabilities:

* game launcher
* process restrictions
* application usage statistics
* patch management
* game update coordination

---

# 19. Arena360 Edge

Arena360 Edge is strategically important.

It runs locally inside the café.

Responsibilities:

* local session state
* local device communication
* hardware automation
* offline operation
* event queue
* cloud synchronization

Architecture:

```text
Arena Cloud
     │
     │
Internet
     │
     ▼
 Arena Edge
     │
 ┌───┼────────┐
 │   │        │
PCs TVs   Consoles
```

If the internet goes down:

```text
Cloud connection ❌

Arena Edge ✅

Sessions continue
Billing continues
Hardware control continues
Events queue locally
```

When connectivity returns:

```text
Queued Events
     ↓
Cloud synchronization
```

This is a strong reliability advantage.

---

# 20. Arena Hub

Arena Hub is the physical automation product.

Possible components:

* ESP32
* IR transmitter
* IR receiver
* HDMI-CEC
* relay
* Bluetooth
* Wi-Fi/Ethernet

It can control:

* television power
* input switching
* display settings
* lighting
* selected console-related operations

Example:

```text
PS5 session starts

      ↓

Arena Edge

      ↓

Arena Hub

      ↓

TV ON

      ↓

HDMI 2

      ↓

Session ACTIVE
```

When the session finishes:

```text
5 minute warning

      ↓

Session ends

      ↓

Station locks

      ↓

TV OFF

      ↓

AVAILABLE
```

This should become one of Arena360's core differentiators.

---

# 21. Venue automation

Eventually create a generic automation engine.

Example:

```text
Trigger:
SessionStarted

Conditions:
station.type = PS5

Actions:
TV.PowerOn
TV.Input = HDMI2
Light.Scene = Gaming
```

Another:

```text
Trigger:
SessionEnding

Before:
5 minutes

Actions:
Display.Warning
Send.Notification
```

Another:

```text
Trigger:
LocationClosed

Actions:
Shutdown.AllPCs
TV.PowerOffAll
TurnOffGamingLights
```

Long term, this becomes:

**IFTTT for gaming venues.**

---

# 22. Reporting

Reporting is table-stakes and should not be hidden behind expensive plans.

Reports should include:

* daily revenue
* monthly revenue
* session revenue
* POS revenue
* membership revenue
* payment method
* customer visits
* wallet transactions
* station utilization
* revenue per station
* staff sales
* shift reconciliation
* refunds
* discounts
* bookings

---

# 23. Arena360 Intelligence

Intelligence is different from reporting.

Reporting says:

> PS5 occupancy was 92%.

Intelligence says:

> PS5 occupancy remained above 90% between 7 PM and 11 PM for the last four Fridays.

Then:

> Consider increasing Friday evening PS5 pricing.

Potential capabilities:

* demand forecasting
* pricing recommendations
* device profitability
* customer retention
* churn risk
* customer LTV
* membership performance
* underused station detection
* peak period identification
* staffing recommendations

---

# 24. Dynamic pricing

Eventually Arena360 should become a revenue-management platform.

Example:

```text
Current

PC Rate
₹40/hour

Morning occupancy
22%
```

Arena360 could recommend:

```text
Suggested

07:00–12:00
₹32/hour
```

And:

```text
PS5

19:00–23:00

Occupancy
94%

Current
₹200/hour

Suggested
₹220/hour
```

Initially these should be recommendations.

Owners approve changes.

Do not immediately let an AI automatically manipulate prices without configurable limits.

---

# 25. Multi-location platform

Arena360 must support chains from the data model from the beginning.

Structure:

```text
Organization
│
├── Location
│   ├── stations
│   ├── employees
│   ├── inventory
│   └── pricing
│
├── Location
│
└── Location
```

Central controls:

* global pricing templates
* branch-specific pricing
* shared customers
* central wallet
* shared memberships
* consolidated reporting
* regional managers
* centralized promotions

---

# 26. SaaS deployment

Default model:

## Arena360 Cloud

Arena360 operates:

* infrastructure
* PostgreSQL
* Redis
* monitoring
* backups
* updates
* security
* availability

Customer simply subscribes.

Suggested positioning:

### Business

₹999/month

### Automation

₹1,999/month

### Intelligence

₹3,499/month

Devices:

**Unlimited within fair-use policies.**

---

# 27. Arena360 Local / On-Prem

Some customers will want local infrastructure.

Offer:

## Arena360 Local

Customer owns and operates:

* server
* networking
* OS
* storage
* UPS
* database infrastructure
* backups

Arena360 provides:

* software
* installation package
* license
* documentation
* update packages

---

# 28. Perpetual licensing

Arena360 Local can offer a perpetual license.

Important terms:

* licensed to one legal entity
* internal business use only
* non-transferable
* non-sublicensable
* cannot resell
* cannot host for third parties
* source code not included

License scope should include locations.

Example:

```text
ABC Gaming Pvt Ltd

Licensed Locations:

Pune
Mumbai
```

Unlimited devices can exist within those licensed locations.

---

# 29. Perpetual license pricing

Indicative model:

### Arena360 Local Business

₹75,000–₹1,00,000

per location.

### Arena360 Local Automation

₹1,25,000–₹1,75,000

per location.

### Enterprise On-Prem

Custom.

Pricing should generally represent roughly several years of equivalent SaaS value.

---

# 30. Perpetual maintenance

Perpetual does not mean unlimited support forever.

Year one:

Included.

Year two onward:

Approximately:

**15–20% of license value/year**

for:

* updates
* security fixes
* compatibility
* support
* upgrade assistance

If maintenance expires:

```text
Current version
continues working
```

But:

```text
New versions
Support
Major upgrades
```

are not included.

---

# 31. Local installation services

On-prem deployment should have a setup charge.

Example:

### Standard Deployment

₹15,000–₹30,000 one-time.

May include:

* OS validation
* Docker installation
* PostgreSQL
* Redis
* reverse proxy
* TLS
* Arena360 installation
* initial backup setup
* staff training
* station onboarding

---

# 32. Managed on-prem

Offer another option:

## Arena360 Managed Local

Customer owns hardware.

Arena360 manages:

* OS/application maintenance
* Docker
* PostgreSQL
* backups
* monitoring
* updates
* certificates
* Arena services

Possible pricing:

₹5,000–₹10,000/month.

This can become highly profitable for larger operators.

---

# 33. Hardware business

Arena360 should make money from optional hardware.

Potential products:

### Arena Hub

Hardware controller.

### Arena Hub Pro

More ports/features.

### Arena Edge Appliance

Preconfigured local server.

For example:

```text
Mini PC
+
Arena Edge preinstalled
+
Remote management
```

Operators who don't want to build their own edge server can simply buy an appliance.

---

# 34. Installation partner ecosystem

Arena360 should not employ installers in every city.

Create:

## Arena360 Certified Partners

Partners can provide:

* local sales
* PC deployment
* Arena Hub installation
* networking
* TV automation
* L1 support

Arena360 provides:

* software
* licenses
* training
* marketing assets
* L2/L3 support

---

# 35. Partner economics

Possible reseller structure:

### Referral Partner

20% first-year commission.

### Sales Partner

20–25% recurring.

### Certified Partner

25–30% recurring.

Plus:

* hardware margin
* installation income
* migration income

This creates strong incentives.

---

# 36. Customer billing should remain centralized

Preferred:

```text
Customer
   ↓
Arena360
   ↓
Partner commission
```

Rather than:

```text
Customer
   ↓
Partner
   ↓
Arena360
```

Arena360 should retain:

* customer ownership
* billing relationship
* renewal data
* product usage data

---

# 37. Revenue model

Long term Arena360 could have multiple revenue streams.

```text
                     ARENA360

                         │
       ┌─────────────────┼──────────────────┐
       │                 │                  │
      SaaS            Hardware          Services
       │                 │                  │
 Subscriptions       Arena Hub          Installation
                     Edge Appliance     Migration
                                        Training

                         │
                         ▼
                      Support

                         │
                         ▼
                    Enterprise

                         │
                         ▼
                     Payments
```

---

# 38. SaaS revenue

Plans:

### Arena360 Business

Run the venue.

### Arena360 Automation

Automate the venue.

### Arena360 Intelligence

Optimize the venue.

Simple positioning:

> ₹999 — Run your venue.

> ₹1,999 — Automate your venue.

> ₹3,499 — Optimize your venue.

---

# 39. Infrastructure economics

At:

```text
1,000 cafés
×
20 average devices
=
20,000 devices
```

Arena360 should architect to keep cloud cost low.

Target:

```text
Cloud COGS
< ₹100 per café/month
```

Avoid writing every heartbeat into PostgreSQL.

Use:

```text
Device heartbeat
     ↓
Redis/in-memory presence
```

Persist only meaningful events:

```text
Online → Offline
Session started
Session ended
Device error
Version update
```

---

# 40. Scale architecture

A sensible architecture:

```text
                Clients

             Cloudflare
                 │
          API Gateway
                 │
        ┌────────┴────────┐
        │                 │
    REST/API        Realtime Gateway
        │                 │
        └────────┬────────┘
                 │
          Application Layer
                 │
      ┌──────────┼──────────┐
      │          │          │
 PostgreSQL    Redis       Queue
      │
   PgBouncer
```

Technology can remain pragmatic.

No need to over-engineer early.

---

# 41. Data architecture

Core entities:

```text
organizations
locations
users
customers
stations
devices
sessions
pricing_rules
membership_plans
memberships
wallets
wallet_transactions
orders
payments
inventory
employees
shifts
bookings
audit_events
```

Every major entity should be tenant-aware.

Typical keys:

```text
organization_id
location_id
```

---

# 42. Events

Arena360 should gradually become event-driven.

Examples:

```text
SessionStarted
SessionExtended
SessionEnded
PaymentCompleted
MembershipPurchased
WalletRecharged
StationOffline
BookingCreated
OrderPlaced
```

Consumers can handle:

* notifications
* loyalty
* analytics
* automation
* audit logging

---

# 43. Observability

Arena360 itself must be highly observable.

Track:

* API latency
* errors
* DB pool saturation
* event loop lag
* Redis latency
* websocket connections
* device connectivity
* synchronization delays
* payment failures
* Arena Edge health

Customer-facing health:

```text
Cafe Pune

Cloud       Online
Edge        Online
PCs         18/20
TV Hubs     5/5
Last Sync   3 sec ago
```

---

# 44. Security

Security should be built into the product.

Key requirements:

* organization isolation
* location isolation
* RBAC
* audit logs
* encrypted secrets
* TLS
* signed device identities
* signed update packages
* refresh-token rotation
* rate limits
* device revocation

Perpetual licenses should use signed offline license files.

---

# 45. Auditability

Sensitive operations must be auditable.

Examples:

```text
Refund issued
Discount overridden
Wallet balance changed
Session manually extended
Price changed
Membership modified
Inventory adjusted
```

Store:

* actor
* time
* reason
* previous value
* new value

---

# 46. Payments

Initially integrate third-party payment gateways.

Flow:

```text
Customer
   ↓
Gateway
   ↓
Cafe merchant account
```

Arena360 tracks the transaction but should avoid unnecessarily becoming the merchant-of-record early.

Later possibilities:

* platform fee
* reservation commission
* integrated payments

---

# 47. API ecosystem

Eventually expose:

## Arena360 API

For:

* franchises
* partners
* ERP
* accounting
* websites
* tournament systems
* third-party apps

Also:

## Webhooks

Examples:

```text
session.started
session.ended
payment.completed
booking.created
customer.created
membership.expired
```

---

# 48. Enterprise features

Enterprise customers may require:

* SSO
* SCIM
* private cloud
* on-prem
* HA
* DR
* SLA
* audit exports
* IP restrictions
* custom retention
* custom integrations
* dedicated support

This should be custom-priced.

---

# 49. Arena360 product tiers

A practical long-term structure:

## Business

Everything required to run the café.

Includes:

* unlimited stations
* sessions
* customers
* wallets
* memberships
* POS
* inventory
* bookings
* employees
* reporting

---

## Automation

Everything in Business.

Plus:

* Arena Edge
* Arena Hub
* TV automation
* HDMI-CEC
* IR
* advanced self-service
* station lifecycle automation
* local offline operations

---

## Intelligence

Everything in Automation.

Plus:

* profitability
* demand analytics
* customer retention
* LTV
* forecasting
* pricing recommendations
* benchmarking
* advanced dashboards

---

# 50. Product roadmap

## Phase 1 — Core operations

Build:

* organizations
* locations
* customers
* stations
* sessions
* rates
* payments
* basic reports
* PC Agent

Goal:

Replace manual café operations.

---

# Phase 2 — Commercial management

Build:

* membership
* wallet
* loyalty
* POS
* staff
* shifts
* bookings
* inventory

Goal:

Replace multiple business tools.

---

# Phase 3 — Edge

Build:

* Arena Edge
* offline operation
* synchronization
* local device gateway
* remote commands

Goal:

Make Arena360 reliable even without internet.

---

# Phase 4 — Automation

Build:

* Arena Hub
* IR
* HDMI-CEC
* TV lifecycle
* console automation
* automation rules

Goal:

Reduce staff intervention.

---

# Phase 5 — Customer self-service

Build:

* mobile/PWA
* QR login
* booking
* payments
* wallet
* food ordering
* station selection

Goal:

Allow partially unattended venues.

---

# Phase 6 — Intelligence

Build:

* utilization intelligence
* station profitability
* customer cohorts
* retention
* demand forecasting
* pricing recommendations

Goal:

Increase owner profitability.

---

# Phase 7 — Ecosystem

Build:

* partner portal
* APIs
* webhooks
* enterprise integrations
* tournaments
* gamer community
* franchise tools

Goal:

Turn Arena360 into a gaming venue platform ecosystem.

---

# 51. What Arena360 should NOT build first

Do not start by chasing every competitor feature.

Avoid prioritizing:

* social networking
* complex tournaments
* AI chatbot
* game marketplace
* esports team management
* streaming features
* deep recommendation engines

until the operational core is excellent.

The first objective is:

> A café owner should trust Arena360 to run the venue every day.

---

# 52. Core competitive advantage

Arena360 should not compete primarily on:

* cheaper billing
* prettier UI
* more reports

Those can be copied.

The stronger moat is the integration between:

```text
Business software
+
PC control
+
Edge infrastructure
+
Physical automation
+
Customer self-service
+
Pricing intelligence
```

Few gaming café products combine all of these.

---

# 53. Positioning against competitors

Conceptually:

```text
Traditional Cafe Software
        ↓
Billing + PC management
```

```text
Modern competitors
        ↓
Billing + POS + memberships
```

Arena360:

```text
Business Operations
        +
Device Management
        +
Physical Automation
        +
Self Service
        +
Revenue Intelligence
```

---

# 54. Ideal customer profiles

## Small café

10–20 stations.

Needs:

* sessions
* memberships
* billing
* POS

Likely Business customer.

---

## Medium gaming café

20–50 stations.

Needs:

* automation
* bookings
* loyalty
* staff controls
* advanced pricing

Likely Automation customer.

---

## Large gaming arena

50–150+ stations.

Needs:

* Edge
* automation
* analytics
* multiple zones
* APIs

Likely Intelligence customer.

---

## Chain/franchise

Multiple locations.

Needs:

* centralized management
* central customer database
* shared memberships
* enterprise reporting
* HA
* integrations

Enterprise.

---

# 55. On-prem ideal customers

Local/perpetual deployment is for customers who:

* have internal IT staff
* want complete infrastructure control
* prefer capital expenditure
* operate larger chains
* have unreliable internet
* have strict data requirements

Cloud should remain the default.

---

# 56. Partner-led go-to-market

Arena360 should be designed for channel distribution.

Partners:

* gaming PC builders
* computer dealers
* network installers
* CCTV/system integrators
* console dealers
* gaming hardware distributors

Partner pitch:

> You're already selling hardware to gaming cafés. Arena360 lets you add recurring software, hardware and installation revenue.

---

# 57. Sales model

Long-term target could be:

```text
70% Channel

20% Self-service/inbound

10% Direct enterprise
```

Central Arena360 team retains:

* product
* engineering
* cloud
* billing
* L2/L3 support
* enterprise relationships

Partners handle:

* local sales
* onboarding
* installation
* L1 support

---

# 58. Unit economics targets

Target:

```text
Blended ARPU
₹1,500–₹2,000+
```

Variable cost:

```text
< ₹300–₹350/customer/month
```

Target software gross margin:

```text
75–85%
```

Target CAC payback:

```text
< 6 months
```

Target monthly logo churn:

```text
< 2%
```

Automation attach rate:

```text
> 25–30%
```

---

# 59. North-star metrics

The business shouldn't only track revenue.

Product metrics:

### Locations

Active gaming locations.

### GMV

Gaming/session transactions processed.

### Sessions

Sessions completed.

### Automated sessions

Percentage without staff intervention.

### Utilization

Station utilization.

### Automation attach rate

Locations using Arena Hub/Edge.

### ARPU

Revenue per location.

### Retention

Monthly/annual location retention.

---

# 60. Arena360 long-term moat

The real moat becomes accumulated operational data.

Over time Arena360 learns:

* demand patterns
* price elasticity
* station performance
* customer retention patterns
* membership behavior
* device failure patterns
* food purchasing behavior

Across thousands of venues, this could create benchmarking.

Example:

> Your 20-PC café has 52% utilization.

Arena360 could eventually say:

> Similar cafés in your market average 64%.

Without exposing competitors' private data.

---

# 61. Future marketplace

Long-term Arena360 could support a marketplace.

Examples:

* gaming hardware
* memberships
* tournaments
* food supplies
* local technicians
* game licenses
* accessories

Arena360 becomes the operational network connecting gaming cafés, vendors and customers.

This should be much later.

---

# 62. Potential Arena360 product family

Long term:

```text
Arena360
│
├── Arena Cloud
├── Arena POS
├── Arena Agent
├── Arena Edge
├── Arena Hub
├── Arena Customer
├── Arena Owner
├── Arena Intelligence
├── Arena Local
└── Arena Partner
```

This creates a coherent product ecosystem.

---

# 63. Product mission

Arena360's mission should be:

> Make running a gaming venue as automated, measurable and scalable as running modern cloud software.

The owner shouldn't need to constantly ask:

* Which PCs are occupied?
* Has that customer's time ended?
* Did staff collect payment?
* Why is that TV still running?
* Which station earns the most?
* Which membership works?
* When should I discount?
* Why is morning utilization low?

Arena360 should answer those questions automatically.

---

# 64. Ultimate customer experience

A future Arena360 venue could operate like this:

```text
Customer enters
      ↓
Scans QR
      ↓
Selects PS5-04
      ↓
Chooses 2 hours
      ↓
Pays via UPI
      ↓
Arena360 books station
      ↓
TV turns on
      ↓
HDMI switches
      ↓
Session starts
      ↓
Customer orders food from phone
      ↓
Wallet deducted
      ↓
5-minute warning
      ↓
Session ends
      ↓
TV turns off
      ↓
Station becomes available
      ↓
Owner dashboard updates
```

No employee needed for most of the workflow.

Meanwhile the owner sees:

```text
Revenue today        ₹42,600

Occupancy               72%

Highest utilization     PS5

Lowest utilization      PC Zone C

Peak window             8–11 PM

Membership revenue      ₹8,400

Suggested action:
Reduce weekday morning PC price
```

That is the end-state Arena360 should build toward.

---

# 65. One-sentence product definition

> **Arena360 is a cloud, edge and automation platform that runs gaming venues end-to-end—from sessions, billing, memberships and POS to PCs, consoles, TVs, self-service and revenue intelligence.**

---

# 66. Short positioning statement

> **Run. Automate. Optimize.**

**Run your venue with Arena360 Business.**

**Automate your venue with Arena360 Edge and Hub.**

**Optimize profitability with Arena360 Intelligence.**

---

# 67. Strategic product thesis

Arena360 should not try to become the cheapest gaming café timer.

Basic timers and billing will increasingly become commodities.

Arena360 should move upward:

```text
Timer
  ↓
Management
  ↓
Automation
  ↓
Optimization
  ↓
Operating System
```

That progression is the core strategic vision.

The long-term objective is for a gaming venue owner to think:

> I can change my PCs, consoles, ISP, POS hardware or payment provider.

> But Arena360 is the system my entire venue runs on.

That is when Arena360 becomes difficult to replace.

Keeping record creation rate limits for customers for who are using unlimied plan and mentioning them in policy to prevent. Abuse policy to be added.
