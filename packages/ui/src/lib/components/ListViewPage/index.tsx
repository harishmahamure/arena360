'use client';

import { Add, Clear, Search } from '@mui/icons-material';
import { Box, Button, IconButton, InputAdornment, TextField, Typography } from '@mui/material';
import { type Action, type Column, DataGrid } from '../DataGrid';
import { GridSkeleton } from '../GridSkeleton';

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
}

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
}: ListViewPageProps<T>) {
  return (
    <>
      <Typography variant="h4" sx={{ marginBottom: '0' }}>
        {title}
      </Typography>
      <Typography variant="body1" sx={{ marginBottom: '16px' }}>
        {description}
      </Typography>

      <Box
        sx={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '16px',
        }}
      >
        <Box sx={{ width: '100%', maxWidth: 300 }}>
          <TextField
            label="Search"
            variant="outlined"
            size="small"
            fullWidth
            value={inputValue}
            onChange={handleSearch}
            InputProps={{
              startAdornment: (
                <InputAdornment position="start">
                  <Search color="action" fontSize="small" />
                </InputAdornment>
              ),
              endAdornment: inputValue && (
                <InputAdornment position="end">
                  <IconButton size="small" onClick={handleClearSearch} edge="end">
                    <Clear fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            }}
          />
        </Box>
        {onAddClick && (
          <Button variant="contained" color="primary" onClick={onAddClick}>
            <Add />
            {addButtonLabel}
          </Button>
        )}
      </Box>

      {isLoading ? (
        <GridSkeleton />
      ) : (
        <DataGrid<T>
          columns={columns}
          data={data}
          rowKey={(row) => row.id}
          showActionsLabel={true}
          emptyMessage="No items found"
          actions={actions}
          maxHeight="calc(100vh - 280px)"
        />
      )}
    </>
  );
}

export default ListViewPage;
