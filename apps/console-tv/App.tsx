import { ActivityIndicator, StatusBar, StyleSheet, View } from 'react-native';
import { ConsoleProvider, useConsole } from './src/context/ConsoleProvider';
import { IdleScreen } from './src/screens/IdleScreen';
import { OverlayScreen } from './src/screens/OverlayScreen';
import { SetupScreen } from './src/screens/SetupScreen';

function ConsoleShell() {
  const { phase } = useConsole();

  if (phase === 'loading') {
    return (
      <View style={styles.loading}>
        <ActivityIndicator size="large" color="#2563eb" />
      </View>
    );
  }

  return (
    <View style={styles.root}>
      {phase === 'setup' ? <SetupScreen /> : <IdleScreen />}
      {phase === 'overlay' ? <OverlayScreen /> : null}
    </View>
  );
}

function App(): React.JSX.Element {
  return (
    <ConsoleProvider>
      <StatusBar barStyle="light-content" backgroundColor="#0f172a" />
      <ConsoleShell />
    </ConsoleProvider>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: '#0f172a',
  },
  loading: {
    flex: 1,
    backgroundColor: '#0f172a',
    alignItems: 'center',
    justifyContent: 'center',
  },
});

export default App;
