export const analyticsDashboards = [
  {
    id: 'executive',
    title: 'Executive Overview',
    action: 'See whether the business is improving.',
    coverage: 'Measured',
  },
  {
    id: 'locations',
    title: 'Location Performance',
    action: 'Compare station areas and identify capacity imbalances.',
    coverage: 'Partial',
  },
  {
    id: 'capacity',
    title: 'Busy Hours & Capacity',
    action: 'Plan staffing and capacity around observed demand.',
    coverage: 'Estimated capacity',
  },
  {
    id: 'opportunity',
    title: 'Revenue Opportunity',
    action: 'Explore how much unused capacity could earn.',
    coverage: 'Scenario',
  },
  {
    id: 'pricing',
    title: 'Dynamic Pricing',
    action: 'Test pricing ideas before changing live prices.',
    coverage: 'Scenario',
  },
  {
    id: 'retention',
    title: 'Customer Retention',
    action: 'Understand repeat visits and identify customers to win back.',
    coverage: 'Measured',
  },
  {
    id: 'memberships',
    title: 'Membership Analytics',
    action: 'Grow prepaid plan repurchases and watch expiry.',
    coverage: 'Plan proxies',
  },
  {
    id: 'stations',
    title: 'Station Performance',
    action: 'Compare station use and prioritize equipment reviews.',
    coverage: 'Measured + allocation',
  },
  {
    id: 'pos',
    title: 'F&B / POS',
    action: 'Grow secondary spend and identify popular products.',
    coverage: 'Measured',
  },
  {
    id: 'staff',
    title: 'Staff / Operations',
    action: 'Review sales productivity and shift coverage.',
    coverage: 'Partial',
  },
  {
    id: 'forecast',
    title: 'Forecast / Recommendations',
    action: 'Plan the next week using observed weekday patterns.',
    coverage: 'Baseline estimate',
  },
] as const;
export type AnalyticsDashboard = (typeof analyticsDashboards)[number]['id'];
