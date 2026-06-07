'use client';

import { Pagination } from '@mui/material';
import type { ReactNode } from 'react';
import type { Action, Column } from '../DataGrid';
import { PageHeader } from '../PageHeader';
import { PageShell, type PageShellProps } from '../PageShell';
import { ListPageContent } from './ListPageContent';

export interface ListPagePagination {
  page: number;
  totalPages?: number;
  onPageChange: (page: number) => void;
}

export interface ListPageProps<T extends { id: string | number }> {
  title: string;
  description?: string;
  data: T[];
  columns: Column<T>[];
  actions?: Action<T>[];
  isLoading: boolean;
  /** Filter chips / FilterBar — maps to PageShell `toolbar` */
  filters?: ReactNode;
  showSearch?: boolean;
  searchValue?: string;
  onSearchChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onSearchClear?: () => void;
  onAddClick?: () => void;
  addButtonLabel?: string;
  pagination?: ListPagePagination;
  /** Overrides built-in Pagination when set */
  footer?: ReactNode;
  dense?: boolean;
  emptyMessage?: string;
  emptyDescription?: string;
  emptyActionLabel?: string;
  onEmptyAction?: () => void;
  maxHeight?: string;
  maxWidth?: PageShellProps['maxWidth'];
  mobileCardRender?: (row: T, rowActions: Action<T>[]) => ReactNode;
}

function ListPagePaginationFooter({ page, totalPages, onPageChange }: ListPagePagination) {
  return (
    <Pagination
      count={totalPages}
      page={page}
      shape="rounded"
      hidePrevButton={page === 1}
      hideNextButton={page === totalPages}
      onChange={(_event, value) => onPageChange(value)}
    />
  );
}

export function ListPage<T extends { id: string | number }>({
  title,
  description,
  data,
  columns,
  actions,
  isLoading,
  filters,
  showSearch = false,
  searchValue,
  onSearchChange,
  onSearchClear,
  onAddClick,
  addButtonLabel,
  pagination,
  footer,
  dense = true,
  emptyMessage,
  emptyDescription,
  emptyActionLabel,
  onEmptyAction,
  maxHeight,
  maxWidth,
  mobileCardRender,
}: ListPageProps<T>) {
  const resolvedFooter =
    footer ??
    (pagination ? (
      <ListPagePaginationFooter
        page={pagination.page}
        totalPages={pagination.totalPages}
        onPageChange={pagination.onPageChange}
      />
    ) : undefined);

  return (
    <PageShell dense={dense} maxWidth={maxWidth} toolbar={filters} footer={resolvedFooter}>
      <PageHeader title={title} description={description} />
      <ListPageContent<T>
        data={data}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        showSearch={showSearch}
        searchValue={searchValue}
        onSearchChange={onSearchChange}
        onSearchClear={onSearchClear}
        onAddClick={onAddClick}
        addButtonLabel={addButtonLabel}
        emptyMessage={emptyMessage}
        emptyDescription={emptyDescription}
        emptyActionLabel={emptyActionLabel}
        onEmptyAction={onEmptyAction}
        maxHeight={maxHeight}
        mobileCardRender={mobileCardRender}
      />
    </PageShell>
  );
}

export default ListPage;
