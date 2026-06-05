import { StyleSheet, Text, View } from 'react-native';

export function SessionActiveBadge() {
  return (
    <View style={styles.container} accessibilityLiveRegion="polite">
      <Text style={styles.label}>Session active</Text>
      <Text style={styles.hint}>Countdown unavailable — waiting for session data</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    alignItems: 'center',
    paddingVertical: 20,
    paddingHorizontal: 32,
    backgroundColor: 'rgba(0,0,0,0.72)',
    borderRadius: 12,
  },
  label: {
    color: '#4ade80',
    fontSize: 32,
    fontWeight: '700',
  },
  hint: {
    color: '#94a3b8',
    fontSize: 14,
    marginTop: 8,
    textAlign: 'center',
  },
});
