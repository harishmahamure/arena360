import { AdvancedFileUpload, MultiImageUpload } from '@gaming-cafe/ui';
import { toastUtils as showToast } from '@gaming-cafe/utils';
import { Image as ImageIcon, InsertDriveFile } from '@mui/icons-material';
import {
  Box,
  Container,
  Divider,
  FormControl,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useFileUpload } from '../../../../src/hooks/useFileUpload';
import type { FileCategory } from '../../../../src/services/files/types';

const MediaUploadPage = () => {
  const [activeTab, setActiveTab] = useState(0);
  const [category, setCategory] = useState<FileCategory>('product');
  const [visibility, setVisibility] = useState<'public' | 'private' | 'admin'>('public');
  const [description, setDescription] = useState('');
  const [tags, setTags] = useState('');
  const [relatedEntityType, setRelatedEntityType] = useState('');
  const [relatedEntityId, setRelatedEntityId] = useState('');

  const { uploadFile } = useFileUpload();

  const handleUpload = async (
    file: File,
    _onProgress?: (progress: number) => void,
  ): Promise<{ id: string } | undefined> => {
    const tagArray = tags
      .split(',')
      .map((tag) => tag.trim())
      .filter((tag) => tag.length > 0);

    try {
      const fileRecord = await uploadFile(file, {
        category,
        visibility,
        description: description || undefined,
        tags: tagArray.length > 0 ? tagArray : undefined,
        relatedEntityType: relatedEntityType || undefined,
        relatedEntityId: relatedEntityId || undefined,
        maxSizeMB: 50,
        allowedTypes: activeTab === 1 ? ['image/*'] : undefined,
      });
      return fileRecord?.id ? { id: fileRecord.id } : undefined;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Upload failed';
      showToast.error(errorMessage);
      throw error;
    }
  };

  return (
    <Container maxWidth="xl" sx={{ py: 4 }}>
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" gutterBottom>
          Media Upload & Management
        </Typography>
        <Typography variant="body1" color="text.secondary">
          Upload and manage images, videos, documents, and other files
        </Typography>
      </Box>

      <Paper sx={{ mb: 4 }}>
        <Tabs
          value={activeTab}
          onChange={(_, newValue) => setActiveTab(newValue)}
          sx={{ borderBottom: 1, borderColor: 'divider' }}
        >
          <Tab icon={<InsertDriveFile />} label="All Files" iconPosition="start" />
          <Tab icon={<ImageIcon />} label="Images Only" iconPosition="start" />
        </Tabs>

        <Box sx={{ p: 3 }}>
          {/* Upload Configuration */}
          <Typography variant="h6" gutterBottom>
            Upload Settings
          </Typography>

          <Box
            sx={{
              display: 'grid',
              gridTemplateColumns: { xs: '1fr', md: 'repeat(3, 1fr)' },
              gap: 2,
              mb: 3,
            }}
          >
            <FormControl fullWidth size="small">
              <InputLabel>Category</InputLabel>
              <Select
                value={category}
                label="Category"
                onChange={(e) => setCategory(e.target.value as FileCategory)}
              >
                <MenuItem value="profile">Profile</MenuItem>
                <MenuItem value="product">Product</MenuItem>
                <MenuItem value="game">Game</MenuItem>
                <MenuItem value="document">Document</MenuItem>
                <MenuItem value="receipt">Receipt</MenuItem>
                <MenuItem value="invoice">Invoice</MenuItem>
                <MenuItem value="temp">Temporary</MenuItem>
                <MenuItem value="other">Other</MenuItem>
              </Select>
            </FormControl>

            <FormControl fullWidth size="small">
              <InputLabel>Visibility</InputLabel>
              <Select
                value={visibility}
                label="Visibility"
                onChange={(e) => setVisibility(e.target.value as 'public' | 'private' | 'admin')}
              >
                <MenuItem value="public">Public</MenuItem>
                <MenuItem value="private">Private</MenuItem>
                <MenuItem value="admin">Admin Only</MenuItem>
              </Select>
            </FormControl>

            <TextField
              label="Description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              size="small"
              fullWidth
            />
          </Box>

          <Box
            sx={{
              display: 'grid',
              gridTemplateColumns: { xs: '1fr', md: 'repeat(3, 1fr)' },
              gap: 2,
              mb: 3,
            }}
          >
            <TextField
              label="Tags (comma separated)"
              value={tags}
              onChange={(e) => setTags(e.target.value)}
              placeholder="featured, banner, main"
              size="small"
              fullWidth
            />

            <TextField
              label="Related Entity Type (optional)"
              value={relatedEntityType}
              onChange={(e) => setRelatedEntityType(e.target.value)}
              placeholder="product, user, game"
              size="small"
              fullWidth
            />

            <TextField
              label="Related Entity ID (optional)"
              value={relatedEntityId}
              onChange={(e) => setRelatedEntityId(e.target.value)}
              placeholder="UUID"
              size="small"
              fullWidth
            />
          </Box>

          <Divider sx={{ my: 3 }} />

          {/* Upload Component */}
          <Typography variant="h6" gutterBottom>
            Select Files
          </Typography>

          {activeTab === 0 ? (
            <AdvancedFileUpload
              multiple
              maxSizeMB={50}
              maxFiles={20}
              autoUpload
              showDropZone
              dropZoneHeight={250}
              showProgress
              helperText="Drag & drop files here or click to browse. Max 50MB per file."
              onUpload={handleUpload}
            />
          ) : (
            <MultiImageUpload
              maxSizeMB={50}
              maxImages={20}
              autoUpload
              previewHeight={200}
              columns={4}
              helperText="Upload images only. Max 50MB per image."
              onUpload={handleUpload}
            />
          )}
        </Box>
      </Paper>
    </Container>
  );
};

export default MediaUploadPage;
