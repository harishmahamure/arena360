import { GridViewRounded } from '@mui/icons-material';
import { Box } from '@mui/material';
import { useBranding } from '../services/config/branding';

/** Venue logo and name when white-labelled; the Arena360 wordmark otherwise. */
export function BrandMark({ subtitle }: { subtitle?: string }) {
  const brand = useBranding();
  if (!brand.logoUrl) {
    return (
      <>
        <span className="brand-mark">
          <GridViewRounded />
        </span>
        <span>
          arena<span className="brand-number">360</span>
          {subtitle && <small>{subtitle}</small>}
        </span>
      </>
    );
  }
  return (
    <>
      <Box
        component="img"
        src={brand.logoUrl}
        alt=""
        sx={{ width: 36, height: 36, objectFit: 'contain', borderRadius: 1, flexShrink: 0 }}
      />
      <span style={{ minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis' }}>
        {brand.name}
        {subtitle && <small>{subtitle}</small>}
      </span>
    </>
  );
}
