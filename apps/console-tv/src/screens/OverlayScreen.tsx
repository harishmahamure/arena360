import { StyleSheet, View } from 'react-native';
import { HudTimer } from '../components/HudTimer';
import { SessionActiveBadge } from '../components/SessionActiveBadge';
import { useConsole } from '../context/ConsoleProvider';

export function OverlayScreen() {
  const { activeSession } = useConsole();
  if (!activeSession) return null;

  const hasTimer =
    activeSession.remainingMinutes != null && activeSession.remainingMinutes >= 0;

  return (
    <View style={styles.container} pointerEvents="none">
      <View style={styles.hud}>
        {hasTimer ? (
          <HudTimer remainingMinutes={activeSession.remainingMinutes as number} />
        ) : (
          <SessionActiveBadge />
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    ...StyleSheet.absoluteFillObject,
    justifyContent: 'flex-start',
    alignItems: 'flex-end',
    paddingTop: 48,
    paddingRight: 48,
  },
  hud: {
    maxWidth: '100%',
  },
});
