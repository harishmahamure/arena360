import { configure } from '@testing-library/react';

// Form and chart rendering can exceed the default one-second wait on shared CI hosts.
configure({ asyncUtilTimeout: 5_000 });
