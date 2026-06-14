'use client';

import { Box, Button, Card, CardContent, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import type { Action, Column } from './DataGrid';

export interface ListMobileCardProps<T extends Record<string, unknown>> {
  row: T;
  columns: Column<T>[];
  actions?: Action<T>[];
}

export function ListMobileCard<T extends Record<string, unknown>>({
  row,
  columns,
  actions = [],
}: ListMobileCardProps<T>) {
  const visibleActions = actions.filter((action) => action.show === undefined || action.show(row));

  return (
    <Card variant="outlined">
      <CardContent>
        <Stack spacing={1.5}>
          {columns.map((column, index) => {
            const value = row[column.id];
            const content: ReactNode = column.format
              ? column.format(value, row)
              : (value as ReactNode);

            return (
              <Box key={column.key ?? `${String(column.id)}-${index}`}>
                <Typography variant="caption" color="text.secondary" display="block" gutterBottom>
                  {column.label}
                </Typography>
                <Box>{content}</Box>
              </Box>
            );
          })}

          {visibleActions.length > 0 && (
            <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap sx={{ pt: 0.5 }}>
              {visibleActions.map((action) => {
                const isDisabled = action.disabled?.(row);
                return (
                  <Button
                    key={action.label}
                    variant="outlined"
                    size="small"
                    color={action.color === 'default' ? 'inherit' : action.color}
                    startIcon={action.icon}
                    disabled={isDisabled}
                    onClick={() => action.onClick(row)}
                    sx={{ minHeight: 44 }}
                  >
                    {action.label}
                  </Button>
                );
              })}
            </Stack>
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}
