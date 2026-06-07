import {
  ErrorOutline as ErrorIcon,
  ExpandLess as ExpandLessIcon,
  ExpandMore as ExpandMoreIcon,
  Refresh as RefreshIcon,
} from '@mui/icons-material';
import {
  Alert,
  AlertTitle,
  Box,
  Button,
  Collapse,
  Container,
  Paper,
  Typography,
} from '@mui/material';
import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
  showDetails: boolean;
}

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
      showDetails: false,
    };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    // Update state so the next render will show the fallback UI
    return { hasError: true, error };
  }

  override componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({
      error,
      errorInfo,
    });

    // You can also log the error to an error reporting service here
    // logErrorToService(error, errorInfo);
  }

  handleReset = () => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null,
      showDetails: false,
    });
  };

  handleReload = () => {
    window.location.reload();
  };

  toggleDetails = () => {
    this.setState((prevState) => ({
      showDetails: !prevState.showDetails,
    }));
  };

  override render() {
    if (this.state.hasError) {
      return (
        <Box
          sx={{
            minHeight: '100vh',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            bgcolor: 'background.default',
            py: 4,
          }}
        >
          <Container maxWidth="md">
            <Paper
              elevation={3}
              sx={{
                p: 4,
                borderRadius: 2,
                textAlign: 'center',
              }}
            >
              <Box
                sx={{
                  display: 'flex',
                  justifyContent: 'center',
                  mb: 3,
                }}
              >
                <ErrorIcon
                  sx={{
                    fontSize: 80,
                    color: 'error.main',
                  }}
                />
              </Box>

              <Typography variant="h4" component="h1" gutterBottom color="error">
                Something went wrong
              </Typography>

              <Typography variant="body1" color="text.secondary" sx={{ mb: 3 }}>
                We are sorry for the inconvenience. An unexpected error has occurred. Try refreshing
                the page or contact support if the problem persists.
              </Typography>

              <Alert severity="error" sx={{ mb: 3, textAlign: 'left' }}>
                <AlertTitle>Error Details</AlertTitle>
                <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>
                  {this.state.error?.message || 'Unknown error'}
                </Typography>
              </Alert>

              <Box sx={{ mb: 3 }}>
                <Button
                  variant="text"
                  size="small"
                  onClick={this.toggleDetails}
                  endIcon={this.state.showDetails ? <ExpandLessIcon /> : <ExpandMoreIcon />}
                >
                  {this.state.showDetails ? 'Hide Technical Details' : 'Show Technical Details'}
                </Button>

                <Collapse in={this.state.showDetails}>
                  <Paper
                    variant="outlined"
                    sx={{
                      mt: 2,
                      p: 2,
                      bgcolor: 'grey.50',
                      maxHeight: 300,
                      overflow: 'auto',
                      textAlign: 'left',
                    }}
                  >
                    <Typography
                      variant="caption"
                      component="pre"
                      sx={{
                        fontFamily: 'monospace',
                        whiteSpace: 'pre-wrap',
                        wordBreak: 'break-word',
                        fontSize: '0.75rem',
                      }}
                    >
                      {this.state.error?.stack}
                      {'\n\n'}
                      {this.state.errorInfo?.componentStack}
                    </Typography>
                  </Paper>
                </Collapse>
              </Box>

              <Box
                sx={{
                  display: 'flex',
                  gap: 2,
                  justifyContent: 'center',
                  flexWrap: 'wrap',
                }}
              >
                <Button
                  variant="contained"
                  color="primary"
                  startIcon={<RefreshIcon />}
                  onClick={this.handleReload}
                  size="large"
                >
                  Reload Page
                </Button>
                <Button
                  variant="outlined"
                  color="secondary"
                  onClick={this.handleReset}
                  size="large"
                >
                  Try Again
                </Button>
              </Box>

              <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 3 }}>
                If the problem persists, please contact support.
              </Typography>
            </Paper>
          </Container>
        </Box>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
