import { Box, Skeleton } from '@mui/material';

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
        {[150, 100, 100, 80, 80, 80, 120].map((width, i) => (
          <Skeleton key={i} variant="text" width={width} height={24} />
        ))}
      </Box>
      {/* Table Rows */}
      {Array.from({ length: 5 }).map((_, rowIndex) => (
        <Box
          key={rowIndex}
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
