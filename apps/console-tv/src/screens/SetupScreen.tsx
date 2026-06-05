import { deviceSubTypeOptions, deviceTypeOptions } from '@gaming-cafe/contracts';
import { useEffect, useState } from 'react';
import {
  ActivityIndicator,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from 'react-native';
import { useConsole } from '../context/ConsoleProvider';
import { collectFingerprint, type FingerprintPayload } from '../lib/native/ConsoleNative';

type Step = 'credentials' | 'otp' | 'device';

const CONSOLE_TYPES = deviceTypeOptions.filter((o) =>
  ['PS5', 'PS4', 'CONSOLE'].includes(o.value),
);
const TV_SUBTYPES = deviceSubTypeOptions.filter((o) =>
  ['PREMIUM_TV_CONSOLES', 'STANDARD_TV_CONSOLES'].includes(o.value),
);

export function SetupScreen() {
  const { requestAdminOtp, verifyRegistrationOtp, provisionDevice, adminAuthenticated, error } =
    useConsole();

  const [step, setStep] = useState<Step>('credentials');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [otp, setOtp] = useState('');
  const [sessionOtpId, setSessionOtpId] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [deviceType, setDeviceType] = useState(CONSOLE_TYPES[0]?.value ?? 'PS5');
  const [deviceSubType, setDeviceSubType] = useState(
    TV_SUBTYPES[0]?.value ?? 'PREMIUM_TV_CONSOLES',
  );
  const [location, setLocation] = useState('');
  const [fingerprint, setFingerprint] = useState<FingerprintPayload | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    collectFingerprint()
      .then(setFingerprint)
      .catch(() => setFingerprint(null));
  }, []);

  async function onCredentials() {
    setBusy(true);
    try {
      const transactionId = await requestAdminOtp(username, password);
      setSessionOtpId(transactionId);
      setStep('otp');
    } catch {
      // error in context
    } finally {
      setBusy(false);
    }
  }

  async function onOtp() {
    if (!sessionOtpId) return;
    setBusy(true);
    try {
      await verifyRegistrationOtp(otp.trim(), sessionOtpId);
      setStep('device');
    } catch {
      // error in context
    } finally {
      setBusy(false);
    }
  }

  async function onProvision() {
    setBusy(true);
    try {
      await provisionDevice({
        name: name.trim(),
        deviceType,
        deviceSubType,
        location: location.trim(),
      });
    } catch {
      // error in context
    } finally {
      setBusy(false);
    }
  }

  return (
    <ScrollView contentContainerStyle={styles.scroll} keyboardShouldPersistTaps="handled">
      <Text style={styles.title}>Set up this console TV</Text>
      <Text style={styles.subtitle}>Sign in with an administrator account to register this station.</Text>
      {error ? <Text style={styles.error}>{error}</Text> : null}

      {step === 'credentials' && (
        <View style={styles.form}>
          <Text style={styles.label}>Admin username</Text>
          <TextInput
            style={styles.input}
            value={username}
            onChangeText={setUsername}
            autoCapitalize="none"
            autoCorrect={false}
          />
          <Text style={styles.label}>Password</Text>
          <TextInput
            style={styles.input}
            value={password}
            onChangeText={setPassword}
            secureTextEntry
          />
          <TouchableOpacity style={styles.button} onPress={onCredentials} disabled={busy}>
            {busy ? <ActivityIndicator color="#fff" /> : <Text style={styles.buttonText}>Continue</Text>}
          </TouchableOpacity>
        </View>
      )}

      {step === 'otp' && (
        <View style={styles.form}>
          <Text style={styles.label}>Authenticator code</Text>
          <TextInput
            style={styles.input}
            value={otp}
            onChangeText={setOtp}
            keyboardType="number-pad"
            maxLength={8}
          />
          <TouchableOpacity style={styles.button} onPress={onOtp} disabled={busy}>
            {busy ? <ActivityIndicator color="#fff" /> : <Text style={styles.buttonText}>Verify</Text>}
          </TouchableOpacity>
        </View>
      )}

      {step === 'device' && adminAuthenticated && (
        <View style={styles.form}>
          <Text style={styles.label}>Station name</Text>
          <TextInput style={styles.input} value={name} onChangeText={setName} />
          <Text style={styles.label}>Device type</Text>
          <View style={styles.row}>
            {CONSOLE_TYPES.map((opt) => (
              <TouchableOpacity
                key={opt.value}
                style={[styles.chip, deviceType === opt.value && styles.chipActive]}
                onPress={() => setDeviceType(opt.value)}
              >
                <Text style={styles.chipText}>{opt.label}</Text>
              </TouchableOpacity>
            ))}
          </View>
          <Text style={styles.label}>Console tier</Text>
          <View style={styles.row}>
            {TV_SUBTYPES.map((opt) => (
              <TouchableOpacity
                key={opt.value}
                style={[styles.chip, deviceSubType === opt.value && styles.chipActive]}
                onPress={() => setDeviceSubType(opt.value)}
              >
                <Text style={styles.chipText}>{opt.label}</Text>
              </TouchableOpacity>
            ))}
          </View>
          <Text style={styles.label}>Location (optional)</Text>
          <TextInput style={styles.input} value={location} onChangeText={setLocation} />
          {fingerprint ? (
            <Text style={styles.meta}>
              Device: {fingerprint.manufacturer ?? 'Android'} {fingerprint.model ?? ''}
            </Text>
          ) : null}
          <TouchableOpacity style={styles.button} onPress={onProvision} disabled={busy || !name.trim()}>
            {busy ? (
              <ActivityIndicator color="#fff" />
            ) : (
              <Text style={styles.buttonText}>Register station</Text>
            )}
          </TouchableOpacity>
        </View>
      )}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: {
    flexGrow: 1,
    padding: 32,
    backgroundColor: '#0f172a',
  },
  title: {
    color: '#f8fafc',
    fontSize: 28,
    fontWeight: '700',
    marginBottom: 8,
  },
  subtitle: {
    color: '#94a3b8',
    fontSize: 16,
    marginBottom: 24,
  },
  error: {
    color: '#f87171',
    marginBottom: 16,
  },
  form: {
    gap: 8,
  },
  label: {
    color: '#cbd5e1',
    fontSize: 14,
    marginTop: 12,
  },
  input: {
    backgroundColor: '#1e293b',
    color: '#f8fafc',
    borderRadius: 8,
    paddingHorizontal: 16,
    paddingVertical: 12,
    fontSize: 16,
  },
  button: {
    backgroundColor: '#2563eb',
    borderRadius: 8,
    paddingVertical: 14,
    alignItems: 'center',
    marginTop: 24,
  },
  buttonText: {
    color: '#fff',
    fontSize: 16,
    fontWeight: '600',
  },
  row: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 8,
    marginTop: 4,
  },
  chip: {
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 8,
    backgroundColor: '#1e293b',
  },
  chipActive: {
    backgroundColor: '#2563eb',
  },
  chipText: {
    color: '#f8fafc',
    fontSize: 14,
  },
  meta: {
    color: '#64748b',
    fontSize: 12,
    marginTop: 8,
  },
});
