'use client';

import { Add, Clear, Search } from '@mui/icons-material';
import { Box, Button, IconButton, InputAdornment, TextField } from '@mui/material';
import type { ChangeEvent, ReactNode } from 'react';
import type { Action, Column } from '../DataGrid';
import { DataGrid } from '../DataGrid';
import { EmptyState } from '../EmptyState';
import { GridSkeleton } from '../GridSkeleton';

export interface ListPageContentProps<T extends { id: string | number }> {
  data: T[];
  columns: Column<T>[];
  actions?: Action<T>[];
  isLoading: boolean;
  showSearch?: boolean;
  searchValue?: string;
  onSearchChange?: (event: ChangeEvent<HTMLInputElement>) => void;
  onSearchClear?: () => void;
  onAddClick?: () => void;
  addButtonLabel?: string;
  emptyMessage?: string;
  emptyDescription?: string;
  emptyActionLabel?: string;
  onEmptyAction?: () => void;
  maxHeight?: string;
  mobileCardRender?: (row: T, rowActions: Action<T>[]) => ReactNode;
}

export function ListPageContent<T extends { id: string | number }>({
  data,
  columns,
  actions,
  isLoading,
  showSearch = false,
  searchValue = '',
  onSearchChange,
  onSearchClear,
  onAddClick,
  addButtonLabel = 'Add Item',
  emptyMessage = 'No items found',
  emptyDescription,
  emptyActionLabel,
  onEmptyAction,
  maxHeight = 'calc(100vh - 280px)',
  mobileCardRender,
}: ListPageContentProps<T>) {
  const showToolbarRow = showSearch || onAddClick;
  const resolvedActionLabel = emptyActionLabel ?? (onAddClick ? addButtonLabel : undefined);
  const resolvedOnAction = onEmptyAction ?? onAddClick;

  return (
    <>
      {showToolbarRow && (
        <Box
          sx={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            mb: 2,
          }}
        >
          {showSearch ? (
            <Box sx={{ width: '100%', maxWidth: 300 }}>
              <TextField
                label="Search"
                variant="outlined"
                size="small"
                fullWidth
                value={searchValue}
                onChange={onSearchChange}
                InputProps={{
                  startAdornment: (
                    <InputAdornment position="start">
                      <Search color="action" fontSize="small" />
                    </InputAdornment>
                  ),
                  endAdornment: searchValue ? (
                    <InputAdornment position="end">
                      <IconButton size="small" onClick={onSearchClear} edge="end">
                        <Clear fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  ) : undefined,
                }}
              />
            </Box>
          ) : (
            <Box />
          )}
          {onAddClick && (
            <Button variant="contained" color="primary" onClick={onAddClick}>
              <Add />
              {addButtonLabel}
            </Button>
          )}
        </Box>
      )}

      {isLoading ? (
        <GridSkeleton />
      ) : data.length === 0 ? (
        <EmptyState
          title={emptyMessage}
          description={emptyDescription}
          actionLabel={resolvedActionLabel}
          onAction={resolvedOnAction}
        />
      ) : (
        <DataGrid<T>
          columns={columns}
          data={data}
          rowKey={(row) => row.id}
          showActionsLabel
          emptyMessage={emptyMessage}
          actions={actions}
          maxHeight={maxHeight}
          renderMobileCard={mobileCardRender}
        />
      )}
    </>
  );
}
