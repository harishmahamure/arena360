import { StyleSheet, Text, View } from 'react-native';
import { useConsole } from '../context/ConsoleProvider';

export function IdleScreen() {
  const { deviceName, wsConnected } = useConsole();

  return (
    <View style={styles.container}>
      <Text style={styles.brand}>Arena360 Console</Text>
      <Text style={styles.station}>{deviceName ?? 'Console station'}</Text>
      <Text style={styles.status}>Waiting for session</Text>
      <Text style={styles.ws}>{wsConnected ? 'Connected' : 'Reconnecting…'}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#0f172a',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 32,
  },
  brand: {
    color: '#64748b',
    fontSize: 18,
    letterSpacing: 2,
    textTransform: 'uppercase',
  },
  station: {
    color: '#f8fafc',
    fontSize: 36,
    fontWeight: '700',
    marginTop: 12,
    textAlign: 'center',
  },
  status: {
    color: '#94a3b8',
    fontSize: 20,
    marginTop: 24,
  },
  ws: {
    color: '#475569',
    fontSize: 14,
    marginTop: 48,
  },
});
