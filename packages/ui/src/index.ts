// Layouts

export type { AppBarQuickActions, ShiftBadgeProps } from './lib/components/AppBar';
export { default as AppBar } from './lib/components/AppBar';
export type {
  CrudPageConfig,
  CrudPageHandlers,
  CrudPageProps,
  PageMode,
} from './lib/components/CrudPage';
// CrudPage
export { CrudPage } from './lib/components/CrudPage';
export type { Action, Column } from './lib/components/DataGrid';
export { DataGrid } from './lib/components/DataGrid';
export type {
  DetailPageProps,
  DetailPageSection,
  DetailPageStatus,
} from './lib/components/DetailPage';
// DetailPage
export { DetailPage } from './lib/components/DetailPage';
export type { EmptyStateProps } from './lib/components/EmptyState';
// EmptyState
export { EmptyState } from './lib/components/EmptyState';
export type { ErrorPanelProps } from './lib/components/ErrorPanel';
// ErrorPanel
export { ErrorPanel } from './lib/components/ErrorPanel';
export type {
  FieldConfig,
  FieldType,
  FormBuilderProps,
  FormMode,
  FormSection,
  UseFormBuilderOptions,
} from './lib/components/FormBuilder';
// FormBuilder
export { FormBuilder, useFormBuilder } from './lib/components/FormBuilder';
export type { FormPageProps } from './lib/components/FormPage';
// FormPage
export { FormPage } from './lib/components/FormPage';
// FormSkeleton
export { FormSkeleton } from './lib/components/FormSkeleton';
// Form Components
export * from './lib/components/forms';
export type { GameCardProps } from './lib/components/GameCard';
// GameCard + default icon fallback (ggCircuit-style design primitives)
export { GameCard } from './lib/components/GameCard';
export { GridSkeleton } from './lib/components/GridSkeleton';
export type { FallbackIconKind, IconFallbackProps } from './lib/components/IconFallback';
export { defaultIconKind, IconFallback } from './lib/components/IconFallback';
export type { ListMobileCardProps } from './lib/components/ListMobileCard';
export { ListMobileCard } from './lib/components/ListMobileCard';
export type { ListPagePagination, ListPageProps } from './lib/components/ListPage';
// ListPage
export { ListPage } from './lib/components/ListPage';
export type { ListViewPageProps } from './lib/components/ListViewPage';
/** @deprecated Use ListPage instead */
export { ListViewPage } from './lib/components/ListViewPage';
export type { BreadcrumbItem, PageHeaderProps } from './lib/components/PageHeader';
export { PageHeader } from './lib/components/PageHeader';
export type { PageShellProps } from './lib/components/PageShell';
export { PageShell } from './lib/components/PageShell';
// Components
export type { NavItem } from './lib/components/Sidebar';
export { default as Sidebar } from './lib/components/Sidebar';
export { default as AuthLayout } from './lib/layouts/AuthLayout';
export { default as DashboardLayout } from './lib/layouts/DashboardLayout';
