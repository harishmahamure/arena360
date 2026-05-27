'use client';

import { toastUtils as showToast } from '@gaming-cafe/utils';
import {
  Archive,
  Delete,
  Download,
  Edit,
  Image as ImageIcon,
  InsertDriveFile,
  MoreVert,
  PictureAsPdf,
  Restore,
  VideoFile,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardActions,
  CardContent,
  CardMedia,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  GridLegacy as Grid,
  IconButton,
  Menu,
  MenuItem,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { formatFileSize } from '../../services/files/fileManagement';
import type { FileRecord } from '../../services/files/types';

export interface FileGalleryProps {
  files: FileRecord[];
  loading?: boolean;
  error?: string;
  onDelete?: (fileId: string) => void;
  onArchive?: (fileId: string) => void;
  onRestore?: (fileId: string) => void;
  onUpdate?: (fileId: string, data: { description?: string; tags?: string[] }) => void;
  onDownload?: (fileId: string) => void;
  columns?: number;
  showActions?: boolean;
  enableDownload?: boolean;
  enableEdit?: boolean;
  enableDelete?: boolean;
  enableArchive?: boolean;
}

const getFileIcon = (contentType: string) => {
  if (contentType.startsWith('image/')) return <ImageIcon sx={{ fontSize: 64 }} />;
  if (contentType.startsWith('video/')) return <VideoFile sx={{ fontSize: 64 }} />;
  if (contentType === 'application/pdf') return <PictureAsPdf sx={{ fontSize: 64 }} />;
  return <InsertDriveFile sx={{ fontSize: 64 }} />;
};

const FileGallery: React.FC<FileGalleryProps> = ({
  files = [],
  loading = false,
  error,
  onDelete,
  onArchive,
  onRestore,
  onUpdate,
  onDownload,
  columns = 4,
  showActions = true,
  enableDownload = true,
  enableEdit = true,
  enableDelete = true,
  enableArchive = true,
}) => {
  const [selectedFile, setSelectedFile] = useState<FileRecord | null>(null);
  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [editDescription, setEditDescription] = useState('');
  const [editTags, setEditTags] = useState('');
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const [menuFile, setMenuFile] = useState<FileRecord | null>(null);

  const handleMenuOpen = (event: React.MouseEvent<HTMLElement>, file: FileRecord) => {
    setAnchorEl(event.currentTarget);
    setMenuFile(file);
  };

  const handleMenuClose = () => {
    setAnchorEl(null);
    setMenuFile(null);
  };

  const handleEditOpen = (file: FileRecord) => {
    setSelectedFile(file);
    setEditDescription(file.description || '');
    setEditTags((file.tags || []).join(', '));
    setEditDialogOpen(true);
    handleMenuClose();
  };

  const handleEditSave = () => {
    if (selectedFile && onUpdate) {
      const tags = editTags
        .split(',')
        .map((tag) => tag.trim())
        .filter((tag) => tag.length > 0);

      onUpdate(selectedFile.id, {
        description: editDescription,
        tags,
      });

      setEditDialogOpen(false);
      setSelectedFile(null);
      showToast.success('File updated successfully');
    }
  };

  const handleDeleteOpen = (file: FileRecord) => {
    setSelectedFile(file);
    setDeleteDialogOpen(true);
    handleMenuClose();
  };

  const handleDeleteConfirm = () => {
    if (selectedFile && onDelete) {
      onDelete(selectedFile.id);
      setDeleteDialogOpen(false);
      setSelectedFile(null);
      showToast.success('File deleted successfully');
    }
  };

  const handleDownload = (file: FileRecord) => {
    if (onDownload) {
      onDownload(file.id);
      showToast.info('Downloading file...');
    }
    handleMenuClose();
  };

  const handleArchive = (file: FileRecord) => {
    if (onArchive) {
      onArchive(file.id);
      showToast.success('File archived');
    }
    handleMenuClose();
  };

  const handleRestore = (file: FileRecord) => {
    if (onRestore) {
      onRestore(file.id);
      showToast.success('File restored');
    }
    handleMenuClose();
  };

  if (loading) {
    return (
      <Box
        sx={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          minHeight: 400,
        }}
      >
        <CircularProgress />
      </Box>
    );
  }

  if (error) {
    return (
      <Alert severity="error" sx={{ my: 2 }}>
        {error}
      </Alert>
    );
  }

  if (!files || files.length === 0) {
    return (
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          minHeight: 400,
          bgcolor: 'grey.50',
          borderRadius: 1,
          border: 1,
          borderColor: 'grey.300',
          borderStyle: 'dashed',
        }}
      >
        <InsertDriveFile sx={{ fontSize: 64, color: 'grey.400', mb: 2 }} />
        <Typography variant="h6" color="text.secondary">
          No files found
        </Typography>
      </Box>
    );
  }

  return (
    <>
      <Grid container spacing={2}>
        {files &&
          files.length > 0 &&
          files.map((file) => (
            <Grid item xs={12} sm={6} md={12 / columns} key={file.id}>
              <Card
                sx={{
                  height: '100%',
                  display: 'flex',
                  flexDirection: 'column',
                }}
              >
                {/* File preview */}
                {file.contentType.startsWith('image/') ? (
                  <CardMedia
                    component="img"
                    height="200"
                    image={`${import.meta.env.VITE_API_URL}/storage/download/${file.storageKey}`}
                    alt={file.fileName}
                    sx={{ objectFit: 'cover' }}
                    onError={(e: React.SyntheticEvent<HTMLImageElement>) => {
                      const target = e.currentTarget;
                      target.style.display = 'none';
                      const sibling = target.nextElementSibling as HTMLElement | null;
                      if (sibling) {
                        sibling.style.display = 'flex';
                      }
                    }}
                  />
                ) : (
                  <Box
                    sx={{
                      height: 200,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      bgcolor: 'grey.100',
                    }}
                  >
                    {getFileIcon(file.contentType)}
                  </Box>
                )}

                <CardContent sx={{ flex: 1 }}>
                  <Typography variant="subtitle2" noWrap title={file.fileName}>
                    {file.fileName}
                  </Typography>
                  <Typography variant="caption" color="text.secondary" display="block">
                    {formatFileSize(file.fileSize)}
                  </Typography>

                  {file.description && (
                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ mt: 1, fontSize: '0.8rem' }}
                    >
                      {file.description}
                    </Typography>
                  )}

                  <Box sx={{ mt: 1, display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
                    <Chip label={file.category} size="small" color="primary" />
                    {file.status === 'archived' && (
                      <Chip label="Archived" size="small" color="warning" />
                    )}
                    {file.tags?.map((tag) => (
                      <Chip key={tag} label={tag} size="small" variant="outlined" />
                    ))}
                  </Box>

                  {file.width && file.height && (
                    <Typography
                      variant="caption"
                      color="text.secondary"
                      display="block"
                      sx={{ mt: 0.5 }}
                    >
                      {file.width} × {file.height}
                    </Typography>
                  )}

                  <Typography
                    variant="caption"
                    color="text.secondary"
                    display="block"
                    sx={{ mt: 0.5 }}
                  >
                    {new Date(file.createdAt).toLocaleDateString()}
                  </Typography>
                </CardContent>

                {showActions && (
                  <CardActions sx={{ justifyContent: 'space-between', px: 2, pb: 2 }}>
                    <Box>
                      {enableDownload && onDownload && (
                        <IconButton
                          size="small"
                          onClick={() => handleDownload(file)}
                          title="Download"
                        >
                          <Download fontSize="small" />
                        </IconButton>
                      )}
                    </Box>
                    <IconButton size="small" onClick={(e) => handleMenuOpen(e, file)}>
                      <MoreVert fontSize="small" />
                    </IconButton>
                  </CardActions>
                )}
              </Card>
            </Grid>
          ))}
      </Grid>

      {/* Context Menu */}
      <Menu anchorEl={anchorEl} open={Boolean(anchorEl)} onClose={handleMenuClose}>
        {enableDownload && onDownload && menuFile && (
          <MenuItem onClick={() => handleDownload(menuFile)}>
            <Download fontSize="small" sx={{ mr: 1 }} />
            Download
          </MenuItem>
        )}
        {enableEdit && onUpdate && menuFile && (
          <MenuItem onClick={() => handleEditOpen(menuFile)}>
            <Edit fontSize="small" sx={{ mr: 1 }} />
            Edit
          </MenuItem>
        )}
        {enableArchive && onArchive && menuFile && menuFile.status !== 'archived' && (
          <MenuItem onClick={() => handleArchive(menuFile)}>
            <Archive fontSize="small" sx={{ mr: 1 }} />
            Archive
          </MenuItem>
        )}
        {enableArchive && onRestore && menuFile && menuFile.status === 'archived' && (
          <MenuItem onClick={() => handleRestore(menuFile)}>
            <Restore fontSize="small" sx={{ mr: 1 }} />
            Restore
          </MenuItem>
        )}
        {enableDelete && onDelete && menuFile && (
          <MenuItem onClick={() => handleDeleteOpen(menuFile)}>
            <Delete fontSize="small" sx={{ mr: 1 }} />
            Delete
          </MenuItem>
        )}
      </Menu>

      {/* Edit Dialog */}
      <Dialog
        open={editDialogOpen}
        onClose={() => setEditDialogOpen(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>Edit File</DialogTitle>
        <DialogContent>
          <TextField
            label="Description"
            multiline
            rows={3}
            fullWidth
            value={editDescription}
            onChange={(e) => setEditDescription(e.target.value)}
            sx={{ mt: 2 }}
          />
          <TextField
            label="Tags (comma separated)"
            fullWidth
            value={editTags}
            onChange={(e) => setEditTags(e.target.value)}
            helperText="e.g., featured, banner, main"
            sx={{ mt: 2 }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setEditDialogOpen(false)}>Cancel</Button>
          <Button onClick={handleEditSave} variant="contained">
            Save
          </Button>
        </DialogActions>
      </Dialog>

      {/* Delete Dialog */}
      <Dialog open={deleteDialogOpen} onClose={() => setDeleteDialogOpen(false)}>
        <DialogTitle>Delete File</DialogTitle>
        <DialogContent>
          <Typography>
            Are you sure you want to delete &quot;{selectedFile?.fileName}
            &quot;? This action cannot be undone.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDeleteDialogOpen(false)}>Cancel</Button>
          <Button onClick={handleDeleteConfirm} color="error" variant="contained">
            Delete
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default FileGallery;
