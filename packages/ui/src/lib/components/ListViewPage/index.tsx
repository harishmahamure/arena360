'use client';

import { Typography } from '@mui/material';
import type { Action, Column } from '../DataGrid';
import { ListPageContent } from '../ListPage/ListPageContent';

export interface ListViewPageProps<T extends { id: string | number }> {
  title: string;
  description: string;
  data: T[];
  columns: Column<T>[];
  actions: Action<T>[];
  isLoading: boolean;
  inputValue: string;
  handleSearch: (event: React.ChangeEvent<HTMLInputElement>) => void;
  handleClearSearch: () => void;
  onAddClick?: () => void;
  addButtonLabel?: string;
  showSearch?: boolean;
}

/**
 * @deprecated Use `ListPage` from `@gaming-cafe/ui` instead.
 */
export function ListViewPage<T extends { id: string | number }>({
  isLoading,
  title,
  description,
  data,
  columns,
  actions,
  inputValue,
  handleSearch,
  handleClearSearch,
  onAddClick,
  addButtonLabel = 'Add Item',
  showSearch = true,
}: ListViewPageProps<T>) {
  return (
    <>
      <Typography variant="h4" sx={{ marginBottom: '0' }}>
        {title}
      </Typography>
      <Typography variant="body1" sx={{ marginBottom: '16px' }}>
        {description}
      </Typography>

      <ListPageContent<T>
        data={data}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        showSearch={showSearch}
        searchValue={inputValue}
        onSearchChange={handleSearch}
        onSearchClear={handleClearSearch}
        onAddClick={onAddClick}
        addButtonLabel={addButtonLabel}
      />
    </>
  );
}

export default ListViewPage;
