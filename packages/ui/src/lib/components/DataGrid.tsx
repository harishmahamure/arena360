'use client';

import {
  alpha,
  Box,
  IconButton,
  Paper,
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

  return (
    <TableContainer
      component={Paper}
      sx={{
        borderRadius: 1,
        border: `1px solid ${theme.palette.divider}`,
        boxShadow: theme.shadows[2],
        overflow: maxHeight ? 'auto' : 'hidden',
        transition: 'all 0.3s ease-in-out',
        maxHeight: maxHeight,
      }}
    >
      <Table stickyHeader={stickyHeader} aria-label="data grid table">
        <TableHead>
          <TableRow>
            {visibleColumns.map((column) => (
              <TableCell
                key={column.id as string}
                align={column.align || 'left'}
                sx={{
                  minWidth: column.minWidth,
                  backgroundColor: theme.palette.grey[50],
                  fontWeight: 600,
                  fontSize: '0.875rem',
                  color: theme.palette.text.primary,
                  textTransform: 'uppercase',
                  letterSpacing: '0.05em',
                  borderBottom: `2px solid ${theme.palette.primary.main}`,
                  py: 2,
                  px: isMobile ? 1.5 : 2,
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
                  backgroundColor: theme.palette.grey[50],
                  fontWeight: 600,
                  fontSize: '0.875rem',
                  color: theme.palette.text.primary,
                  textTransform: 'uppercase',
                  letterSpacing: '0.05em',
                  borderBottom: `2px solid ${theme.palette.primary.main}`,
                  py: 2,
                  px: isMobile ? 1.5 : 2,
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
                    transform: onRowClick ? 'scale(1.002)' : 'none',
                  },
                  '&:last-child td, &:last-child th': {
                    border: 0,
                  },
                }}
              >
                {visibleColumns.map((column) => {
                  const value = row[column.id];
                  return (
                    <TableCell
                      key={column.id as string}
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
                                  transition: 'all 0.2s ease-in-out',
                                  '&:hover': {
                                    transform: 'scale(1.1)',
                                  },
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
