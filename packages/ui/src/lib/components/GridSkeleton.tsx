import { Box, Skeleton } from '@mui/material';

const HEADER_COLUMNS = [
  { id: 'name', width: 150 },
  { id: 'status', width: 100 },
  { id: 'type', width: 100 },
  { id: 'col-4', width: 80 },
  { id: 'col-5', width: 80 },
  { id: 'col-6', width: 80 },
  { id: 'actions', width: 120 },
] as const;

const SKELETON_ROWS = ['row-1', 'row-2', 'row-3', 'row-4', 'row-5'] as const;

export const GridSkeleton = () => (
  <>
    <Box
      sx={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: '16px',
      }}
    >
      <Skeleton variant="rounded" width={300} height={40} />
      <Skeleton variant="rounded" width={140} height={36} />
    </Box>
    <Box
      sx={{
        border: '1px solid',
        borderColor: 'divider',
        borderRadius: 1,
        overflow: 'hidden',
      }}
    >
      {/* Table Header */}
      <Box
        sx={{
          display: 'flex',
          bgcolor: 'action.hover',
          py: 1.5,
          px: 2,
          gap: 2,
        }}
      >
        {HEADER_COLUMNS.map((column) => (
          <Skeleton key={column.id} variant="text" width={column.width} height={24} />
        ))}
      </Box>
      {/* Table Rows */}
      {SKELETON_ROWS.map((rowId) => (
        <Box
          key={rowId}
          sx={{
            display: 'flex',
            py: 1.5,
            px: 2,
            gap: 2,
            borderTop: '1px solid',
            borderColor: 'divider',
          }}
        >
          <Skeleton variant="text" width={150} height={24} />
          <Skeleton variant="text" width={100} height={24} />
          <Skeleton variant="text" width={100} height={24} />
          <Skeleton variant="text" width={80} height={24} />
          <Skeleton variant="text" width={80} height={24} />
          <Skeleton variant="rounded" width={70} height={24} />
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Skeleton variant="circular" width={32} height={32} />
            <Skeleton variant="circular" width={32} height={32} />
            <Skeleton variant="circular" width={32} height={32} />
          </Box>
        </Box>
      ))}
    </Box>
  </>
);
