'use client';

import {
  Add as AddIcon,
  Clear as ClearIcon,
  Search as SearchIcon,
  Visibility as VisibilityIcon,
} from '@mui/icons-material';
import { Box, Button, IconButton, InputAdornment, TextField, Typography } from '@mui/material';
import { type ReactNode, useCallback, useMemo, useState } from 'react';
import type { DefaultValues, FieldValues } from 'react-hook-form';
import type * as yup from 'yup';

import { type Action, type Column, DataGrid } from './DataGrid';
import { type FieldConfig, FormBuilder, type FormMode } from './FormBuilder';

// ============================================================================
// Types & Interfaces
// ============================================================================

export type PageMode = 'list' | FormMode;

export interface CrudPageConfig<
  TData extends Record<string, unknown>,
  TFormData extends FieldValues = Omit<TData, 'id'>,
> {
  /** Page title displayed at the top */
  title: string;
  /** Page description/subtitle */
  description?: string;
  /** Entity name for labels (e.g., "Product", "User") */
  entityName: string;
  /** Column configuration for the data grid */
  columns: Column<TData>[];
  /** Form field configuration */
  fields: FieldConfig<TFormData>[];
  /** Yup validation schema for the form */
  schema?: yup.AnyObjectSchema;
  /** Default values for new records */
  defaultValues?: DefaultValues<TFormData>;
  /** Key field for row identification */
  rowKey?: string | ((row: TData) => string | number);
  /** Field to display in edit/view mode titles */
  displayField?: keyof TData;
  /** Custom actions for the data grid */
  customActions?: Action<TData>[];
  /** Enable built-in view action */
  enableView?: boolean;
  /** Enable built-in edit action */
  enableEdit?: boolean;
  /** Enable built-in delete action */
  enableDelete?: boolean;
  /** Enable search functionality */
  enableSearch?: boolean;
  /** Fields to search in */
  searchFields?: (keyof TData)[];
  /** Custom add button label */
  addButtonLabel?: string;
  /** Custom add button icon */
  addButtonIcon?: ReactNode;
  /** Show actions column label */
  showActionsLabel?: boolean;
  /** Empty state message */
  emptyMessage?: string;
  /** Form container props */
  formContainerProps?: Record<string, unknown>;
  /** Form max width */
  formMaxWidth?: number | string;
}

export interface CrudPageHandlers<
  TData extends Record<string, unknown>,
  TFormData extends FieldValues = Omit<TData, 'id'>,
> {
  /** Handler for creating a new record */
  onCreate?: (data: TFormData) => Promise<void> | void;
  /** Handler for updating a record */
  onUpdate?: (
    id: string | number,
    data: TFormData,
    dirtyFields: Partial<Record<keyof TFormData, boolean>>,
  ) => Promise<void> | void;
  /** Handler for deleting a record */
  onDelete?: (row: TData) => Promise<void> | void;
  /** Handler for viewing a record (custom behavior) */
  onView?: (row: TData) => void;
  /** Handler for row click in the grid */
  onRowClick?: (row: TData) => void;
  /** Handler for search */
  onSearch?: (query: string) => void;
  /** Handler for mode change */
  onModeChange?: (mode: PageMode, selectedItem: TData | null) => void;
}

export interface CrudPageProps<
  TData extends Record<string, unknown>,
  TFormData extends FieldValues = Omit<TData, 'id'>,
> {
  /** Configuration for the CRUD page */
  config: CrudPageConfig<TData, TFormData>;
  /** Data to display in the grid */
  data: TData[];
  /** Event handlers */
  handlers?: CrudPageHandlers<TData, TFormData>;
  /** Loading state */
  loading?: boolean;
  /** Current mode (controlled) */
  mode?: PageMode;
  /** Selected item (controlled) */
  selectedItem?: TData | null;
  /** Custom header content */
  headerContent?: ReactNode;
  /** Custom content below the header */
  subHeaderContent?: ReactNode;
  /** Custom content above the data grid */
  beforeGridContent?: ReactNode;
  /** Custom content below the data grid */
  afterGridContent?: ReactNode;
  /** Custom form header */
  formHeader?: (mode: FormMode, item: TData | null) => ReactNode;
  /** Custom form footer */
  formFooter?: (mode: FormMode, item: TData | null) => ReactNode;
  /** Transform data before populating the form */
  transformDataToForm?: (data: TData) => TFormData;
  /** Extract ID from data */
  getItemId?: (data: TData) => string | number;
}

// ============================================================================
// Default Icons
// ============================================================================

const DefaultEditIcon = () => (
  <svg
    width="18"
    height="18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    aria-hidden="true"
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={2}
      d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
    />
  </svg>
);

const DefaultDeleteIcon = () => (
  <svg
    width="18"
    height="18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    aria-hidden="true"
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={2}
      d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
    />
  </svg>
);

// ============================================================================
// CrudPage Component
// ============================================================================

export function CrudPage<
  TData extends Record<string, unknown>,
  TFormData extends FieldValues = Omit<TData, 'id'>,
>({
  config,
  data,
  handlers = {},
  loading = false,
  mode: controlledMode,
  selectedItem: controlledSelectedItem,
  headerContent,
  subHeaderContent,
  beforeGridContent,
  afterGridContent,
  formHeader,
  formFooter,
  transformDataToForm,
  getItemId = (item) => item.id as string | number,
}: CrudPageProps<TData, TFormData>) {
  const {
    title,
    description,
    entityName,
    columns,
    fields,
    schema,
    defaultValues,
    rowKey = 'id',
    displayField = 'name' as keyof TData,
    customActions = [],
    enableView = true,
    enableEdit = true,
    enableDelete = true,
    enableSearch = true,
    searchFields = [],
    addButtonLabel,
    addButtonIcon = <AddIcon sx={{ mr: 1 }} />,
    showActionsLabel = true,
    emptyMessage,
    formContainerProps,
    formMaxWidth = 600,
  } = config;

  const { onCreate, onUpdate, onDelete, onView, onRowClick, onSearch, onModeChange } = handlers;

  // ============================================================================
  // State
  // ============================================================================

  const [internalMode, setInternalMode] = useState<PageMode>('list');
  const [internalSelectedItem, setInternalSelectedItem] = useState<TData | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [formLoading, setFormLoading] = useState(false);

  // Use controlled or internal state
  const mode = controlledMode ?? internalMode;
  const selectedItem = controlledSelectedItem ?? internalSelectedItem;

  // ============================================================================
  // Helpers
  // ============================================================================

  const updateMode = useCallback(
    (newMode: PageMode, item: TData | null = null) => {
      if (controlledMode === undefined) {
        setInternalMode(newMode);
        setInternalSelectedItem(item);
      }
      onModeChange?.(newMode, item);
    },
    [controlledMode, onModeChange],
  );

  const getDisplayValue = useCallback(
    (item: TData | null): string => {
      if (!item) return '';
      const value = item[displayField];
      return typeof value === 'string' ? value : String(value ?? '');
    },
    [displayField],
  );

  // ============================================================================
  // Handlers
  // ============================================================================

  const handleAddNew = useCallback(() => {
    updateMode('add', null);
  }, [updateMode]);

  const handleView = useCallback(
    (row: TData) => {
      if (onView) {
        onView(row);
      } else {
        updateMode('view', row);
      }
    },
    [onView, updateMode],
  );

  const handleEdit = useCallback(
    (row: TData) => {
      updateMode('edit', row);
    },
    [updateMode],
  );

  const handleDelete = useCallback(
    async (row: TData) => {
      if (onDelete) {
        await onDelete(row);
      }
    },
    [onDelete],
  );

  const handleCancel = useCallback(() => {
    updateMode('list', null);
  }, [updateMode]);

  const handleFormSubmit = useCallback(
    async (formData: TFormData, dirtyFields: Partial<Record<keyof TFormData, boolean>>) => {
      setFormLoading(true);

      try {
        if (mode === 'edit' && selectedItem) {
          await onUpdate?.(getItemId(selectedItem), formData, dirtyFields);
        } else if (mode === 'add') {
          await onCreate?.(formData);
        }
        updateMode('list', null);
      } finally {
        setFormLoading(false);
      }
    },
    [mode, selectedItem, onCreate, onUpdate, getItemId, updateMode],
  );

  const handleSearchChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const query = event.target.value;
      setSearchQuery(query);
      onSearch?.(query);
    },
    [onSearch],
  );

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
    onSearch?.('');
  }, [onSearch]);

  // ============================================================================
  // Actions
  // ============================================================================

  const actions: Action<TData>[] = useMemo(() => {
    const builtInActions: Action<TData>[] = [];

    if (enableView) {
      builtInActions.push({
        icon: <VisibilityIcon color="success" />,
        label: 'View',
        onClick: handleView,
        color: 'info',
      });
    }

    if (enableEdit) {
      builtInActions.push({
        icon: <DefaultEditIcon />,
        label: 'Edit',
        onClick: handleEdit,
        color: 'primary',
      });
    }

    if (enableDelete) {
      builtInActions.push({
        icon: <DefaultDeleteIcon />,
        label: 'Delete',
        onClick: handleDelete,
        color: 'error',
      });
    }

    return [...builtInActions, ...customActions];
  }, [enableView, enableEdit, enableDelete, customActions, handleView, handleEdit, handleDelete]);

  // ============================================================================
  // Filtered Data
  // ============================================================================

  const filteredData = useMemo(() => {
    if (!searchQuery || searchFields.length === 0) {
      return data;
    }

    const lowerQuery = searchQuery.toLowerCase();
    return data.filter((item) =>
      searchFields.some((field) => {
        const value = item[field];
        if (value == null) return false;
        return String(value).toLowerCase().includes(lowerQuery);
      }),
    );
  }, [data, searchQuery, searchFields]);

  // ============================================================================
  // Form Values
  // ============================================================================

  const formDefaultValues = useMemo((): DefaultValues<TFormData> => {
    if (selectedItem && (mode === 'edit' || mode === 'view')) {
      if (transformDataToForm) {
        return transformDataToForm(selectedItem) as DefaultValues<TFormData>;
      }
      // Remove 'id' from selectedItem for form
      const { id: _id, ...rest } = selectedItem;
      return rest as DefaultValues<TFormData>;
    }
    return defaultValues ?? ({} as DefaultValues<TFormData>);
  }, [selectedItem, mode, transformDataToForm, defaultValues]);

  // ============================================================================
  // Render
  // ============================================================================

  const renderHeader = () => (
    <>
      {headerContent || (
        <>
          <Typography variant="h4" sx={{ mb: 1, fontWeight: 600 }}>
            {title}
          </Typography>
          {description && (
            <Typography variant="body1" color="text.secondary" sx={{ mb: 1 }}>
              {description}
            </Typography>
          )}
        </>
      )}
      {subHeaderContent}
    </>
  );

  const renderListHeader = () => (
    <Box
      sx={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        mb: 3,
        flexWrap: 'wrap',
        gap: 2,
      }}
    >
      {enableSearch && (
        <Box sx={{ width: '100%', maxWidth: 300 }}>
          <TextField
            label="Search"
            variant="outlined"
            size="small"
            fullWidth
            value={searchQuery}
            onChange={handleSearchChange}
            InputProps={{
              startAdornment: (
                <InputAdornment position="start">
                  <SearchIcon color="action" fontSize="small" />
                </InputAdornment>
              ),
              endAdornment: searchQuery && (
                <InputAdornment position="end">
                  <IconButton size="small" onClick={handleClearSearch} edge="end">
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            }}
          />
        </Box>
      )}
      <Box sx={{ ml: 'auto' }}>
        <Button variant="contained" color="primary" onClick={handleAddNew}>
          {addButtonIcon}
          {addButtonLabel || `Add New ${entityName}`}
        </Button>
      </Box>
    </Box>
  );

  const renderFormTitle = () => {
    const formMode = mode as FormMode;
    const displayValue = getDisplayValue(selectedItem);

    const titles: Record<FormMode, string> = {
      add: `Add New ${entityName}`,
      edit: `Edit ${entityName}${displayValue ? `: ${displayValue}` : ''}`,
      view: `View ${entityName}${displayValue ? `: ${displayValue}` : ''}`,
    };

    return (
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 600 }}>
        {titles[formMode]}
      </Typography>
    );
  };

  const renderForm = () => {
    const formMode = mode as FormMode;

    return (
      <Box>
        {formHeader ? formHeader(formMode, selectedItem) : renderFormTitle()}
        <FormBuilder<TFormData>
          fields={fields}
          schema={schema}
          defaultValues={formDefaultValues}
          mode={formMode}
          onSubmit={handleFormSubmit}
          onCancel={handleCancel}
          loading={loading || formLoading}
          showCancel
          showReset={mode === 'add'}
          requireDirty={mode === 'edit'}
          containerProps={{
            sx: {
              maxWidth: formMaxWidth,
              p: 3,
              bgcolor: 'background.paper',
              borderRadius: 2,
              boxShadow: 1,
              ...formContainerProps,
            },
          }}
        />
        {formFooter?.(formMode, selectedItem)}
      </Box>
    );
  };

  return (
    <Box sx={{ p: 3 }}>
      {renderHeader()}

      {mode === 'list' && (
        <>
          {renderListHeader()}
          {beforeGridContent}
          <DataGrid<TData>
            columns={columns}
            data={filteredData}
            actions={actions}
            rowKey={rowKey}
            onRowClick={onRowClick}
            showActionsLabel={showActionsLabel}
            emptyMessage={emptyMessage || `No ${entityName.toLowerCase()}s found`}
          />
          {afterGridContent}
        </>
      )}

      {(mode === 'add' || mode === 'edit' || mode === 'view') && renderForm()}
    </Box>
  );
}

export default CrudPage;
