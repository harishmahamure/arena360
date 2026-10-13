import { ApiError } from '@gaming-cafe/utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { completeOnboarding, getOnboarding, saveOnboarding } from './onboarding';

const fetchMock = vi.fn();
const encoder = new TextEncoder();

function varint(value: number): number[] {
  const bytes: number[] = [];
  while (value > 127) {
    bytes.push((value & 127) | 128);
    value >>>= 7;
  }
  return [...bytes, value];
}
function frame(flag: number, bytes: Uint8Array): Uint8Array {
  const output = new Uint8Array(5 + bytes.length);
  output[0] = flag;
  new DataView(output.buffer).setUint32(1, bytes.length);
  output.set(bytes, 5);
  return output;
}

// A real gRPC-web ProxyResponse: protobuf status_code/body_json, then success trailers.
function response(status: number, envelope: unknown): Response {
  const json = encoder.encode(JSON.stringify(envelope));
  const message = frame(
    0,
    new Uint8Array([8, ...varint(status), 18, ...varint(json.length), ...json]),
  );
  const trailers = frame(128, encoder.encode('grpc-status: 0\r\n'));
  return new Response(new Uint8Array([...message, ...trailers]), {
    headers: { 'content-type': 'application/grpc-web+proto' },
  });
}

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal('fetch', fetchMock);
  fetchMock.mockImplementation(async () =>
    response(200, {
      success: true,
      data: { progress: { revision: 1 }, readiness: { ready: false } },
    }),
  );
});
afterEach(() => vi.unstubAllGlobals());

it('uses the configuration gRPC service for setup reads, saves, and completion', async () => {
  await expect(getOnboarding()).resolves.toMatchObject({ progress: { revision: 1 } });
  await saveOnboarding({ currentStep: 'stations', expectedRevision: 8, reviewSettings: true });
  await completeOnboarding(9);
  expect(fetchMock).toHaveBeenCalledTimes(3);
  for (const [url] of fetchMock.mock.calls) {
    expect(String(url)).toContain('/arena360.v1.ConfigurationService/Invoke');
  }
});

it('preserves server readiness and revision failures through the transport', async () => {
  fetchMock.mockImplementation(async () =>
    response(409, { message: 'ONBOARDING_REVISION_CONFLICT', details: { currentRevision: 10 } }),
  );
  await expect(
    saveOnboarding({ currentStep: 'review', expectedRevision: 9 }),
  ).rejects.toMatchObject({
    message: 'ONBOARDING_REVISION_CONFLICT',
    statusCode: 409,
    details: { currentRevision: 10 },
  });
  await expect(completeOnboarding(9)).rejects.toBeInstanceOf(ApiError);
});
