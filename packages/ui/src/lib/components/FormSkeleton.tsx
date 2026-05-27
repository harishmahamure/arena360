import { Box, Grid, Skeleton } from '@mui/material';

export const FormSkeleton = () => (
  <Box>
    {/* Header skeleton */}
    <Box sx={{ mb: 4 }}>
      <Skeleton variant="text" width={200} height={40} sx={{ mb: 1 }} />
      <Skeleton variant="text" width={300} height={20} />
    </Box>

    {/* Form fields skeleton */}
    <Grid container spacing={3}>
      {/* Name & SKU - half width each */}
      <Grid size={{ xs: 12, md: 6 }}>
        <Skeleton variant="text" width={100} height={20} sx={{ mb: 1 }} />
        <Skeleton variant="rounded" height={56} />
      </Grid>
      <Grid size={{ xs: 12, md: 6 }}>
        <Skeleton variant="text" width={50} height={20} sx={{ mb: 1 }} />
        <Skeleton variant="rounded" height={56} />
      </Grid>

      {/* Description - full width */}
      <Grid size={{ xs: 12 }}>
        <Skeleton variant="text" width={100} height={20} sx={{ mb: 1 }} />
        <Skeleton variant="rounded" height={100} />
      </Grid>

      {/* Price, Stock, Category - 1/3 width each */}
      <Grid size={{ xs: 12, md: 4 }}>
        <Skeleton variant="text" width={50} height={20} sx={{ mb: 1 }} />
        <Skeleton variant="rounded" height={56} />
      </Grid>
      <Grid size={{ xs: 12, md: 4 }}>
        <Skeleton variant="text" width={100} height={20} sx={{ mb: 1 }} />
        <Skeleton variant="rounded" height={56} />
      </Grid>
      <Grid size={{ xs: 12, md: 4 }}>
        <Skeleton variant="text" width={80} height={20} sx={{ mb: 1 }} />
        <Skeleton variant="rounded" height={56} />
      </Grid>

      {/* Active switch - full width */}
      <Grid size={{ xs: 12 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <Skeleton variant="rounded" width={50} height={30} />
          <Skeleton variant="text" width={180} height={20} />
        </Box>
      </Grid>

      {/* Action buttons */}
      <Grid size={{ xs: 12 }}>
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 2, mt: 2 }}>
          <Skeleton variant="rounded" width={100} height={40} />
          <Skeleton variant="rounded" width={100} height={40} />
          <Skeleton variant="rounded" width={140} height={40} />
        </Box>
      </Grid>
    </Grid>
  </Box>
);
