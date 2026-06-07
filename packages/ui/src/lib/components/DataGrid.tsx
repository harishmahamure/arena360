'use client';

import {
  alpha,
  Box,
  IconButton,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import type { ReactNode } from 'react';

export interface Column<T = Record<string, unknown>> {
  id: keyof T;
  /** React key when multiple columns share the same `id` */
  key?: string;
  label: string;
  align?: 'left' | 'center' | 'right';
  minWidth?: number;
  format?: (value: unknown, row: T) => ReactNode;
  hideOnMobile?: boolean;
  hideOnTablet?: boolean;
}

export interface Action<T = Record<string, unknown>> {
  icon: ReactNode;
  label: string;
  onClick: (row: T) => void;
  color?: 'default' | 'primary' | 'secondary' | 'error' | 'info' | 'success' | 'warning';
  disabled?: (row: T) => boolean;
  show?: (row: T) => boolean;
}

interface DataGridProps<T = Record<string, unknown>> {
  columns: Column<T>[];
  data: T[];
  actions?: Action<T>[];
  rowKey?: string | ((row: T) => string | number);
  stickyHeader?: boolean;
  showActionsLabel?: boolean;
  onRowClick?: (row: T) => void;
  emptyMessage?: string;
  maxHeight?: string | number;
  renderMobileCard?: (row: T, rowActions: Action<T>[]) => ReactNode;
}

// const ROW_HEIGHT = 52; // Approximate height of each row (py: 2 = 32px + content ~20px)
// const HEADER_HEIGHT = 56; // Approximate height of header row
// const MIN_ROWS = 10;

export function DataGrid<T extends Record<string, unknown>>({
  columns,
  data,
  actions = [],
  rowKey = 'id',
  stickyHeader = true,
  showActionsLabel = true,
  onRowClick,
  emptyMessage = 'No data available',
  maxHeight,
  renderMobileCard,
}: DataGridProps<T>) {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));
  const isTablet = useMediaQuery(theme.breakpoints.down('md'));

  const getRowKey = (row: T, index: number): string | number => {
    if (typeof rowKey === 'function') {
      return rowKey(row);
    }
    const key = row[rowKey];
    if (typeof key === 'string' || typeof key === 'number') {
      return key;
    }
    return index;
  };

  const visibleColumns = columns.filter((column) => {
    if (isMobile && column.hideOnMobile) return false;
    if (isTablet && column.hideOnTablet) return false;
    return true;
  });

  const hasActions = actions.length > 0;

  const getColumnKey = (column: Column<T>, index: number) =>
    column.key ?? `${String(column.id)}-${index}`;

  const headerCellSx = {
    backgroundColor: alpha(theme.palette.text.primary, 0.04),
    fontWeight: 600,
    fontSize: '0.8125rem',
    color: theme.palette.text.secondary,
    borderBottom: `1px solid ${theme.palette.divider}`,
    py: 2,
    px: isMobile ? 1.5 : 2,
  };

  const paperSx = {
    borderRadius: 1,
    border: `1px solid ${theme.palette.divider}`,
    boxShadow: theme.shadows[2],
    overflow: maxHeight ? 'auto' : 'hidden',
    transition: 'all 0.3s ease-in-out',
    maxHeight: maxHeight,
  };

  if (isMobile && renderMobileCard) {
    if (data.length === 0) {
      return (
        <Paper sx={{ ...paperSx, py: 8, textAlign: 'center' }}>
          <Typography variant="body1" color="text.secondary">
            {emptyMessage}
          </Typography>
        </Paper>
      );
    }

    return (
      <Stack spacing={2} sx={maxHeight ? { maxHeight, overflow: 'auto' } : undefined}>
        {data.map((row, index) => (
          <Box key={getRowKey(row, index)}>{renderMobileCard(row, actions)}</Box>
        ))}
      </Stack>
    );
  }

  return (
    <TableContainer component={Paper} sx={paperSx}>
      <Table
        stickyHeader={stickyHeader}
        sx={{ tableLayout: 'auto', width: '100%' }}
        aria-label="data grid table"
      >
        <TableHead>
          <TableRow>
            {visibleColumns.map((column, columnIndex) => (
              <TableCell
                key={getColumnKey(column, columnIndex)}
                align={column.align || 'left'}
                sx={{
                  minWidth: column.minWidth,
                  ...headerCellSx,
                }}
              >
                {column.label}
              </TableCell>
            ))}
            {hasActions && (
              <TableCell
                align="center"
                sx={{
                  minWidth: actions.length * 50,
                  ...headerCellSx,
                }}
              >
                {showActionsLabel ? 'Actions' : ''}
              </TableCell>
            )}
          </TableRow>
        </TableHead>
        <TableBody>
          {data.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={visibleColumns.length + (hasActions ? 1 : 0)}
                align="center"
                sx={{ py: 8 }}
              >
                <Typography variant="body1" color="text.secondary">
                  {emptyMessage}
                </Typography>
              </TableCell>
            </TableRow>
          ) : (
            data.map((row, index) => (
              <TableRow
                key={getRowKey(row, index)}
                onClick={() => onRowClick?.(row)}
                sx={{
                  cursor: onRowClick ? 'pointer' : 'default',
                  transition: 'all 0.2s ease-in-out',
                  '&:hover': {
                    backgroundColor: alpha(theme.palette.primary.main, 0.05),
                  },
                  '&:last-child td, &:last-child th': {
                    border: 0,
                  },
                }}
              >
                {visibleColumns.map((column, columnIndex) => {
                  const value = row[column.id];
                  return (
                    <TableCell
                      key={getColumnKey(column, columnIndex)}
                      align={column.align || 'left'}
                      sx={{
                        py: 2,
                        px: isMobile ? 1.5 : 2,
                        fontSize: '0.875rem',
                        color: theme.palette.text.primary,
                      }}
                    >
                      {column.format ? column.format(value, row) : (value as ReactNode)}
                    </TableCell>
                  );
                })}
                {hasActions && (
                  <TableCell
                    align="center"
                    sx={{
                      py: 1,
                      px: isMobile ? 1 : 2,
                    }}
                  >
                    <Box
                      sx={{
                        display: 'flex',
                        gap: 0.5,
                        justifyContent: 'center',
                        alignItems: 'center',
                      }}
                    >
                      {actions.map((action) => {
                        const shouldShow = action.show === undefined || action.show(row);
                        const isDisabled = action.disabled?.(row);

                        if (!shouldShow) return null;

                        return (
                          <Tooltip key={action.label} title={action.label} arrow>
                            <span>
                              <IconButton
                                color={action.color || 'default'}
                                size="small"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  action.onClick(row);
                                }}
                                disabled={isDisabled}
                                sx={{
                                  transition: 'background-color 0.2s ease-in-out',
                                }}
                              >
                                {action.icon}
                              </IconButton>
                            </span>
                          </Tooltip>
                        );
                      })}
                    </Box>
                  </TableCell>
                )}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </TableContainer>
  );
}
