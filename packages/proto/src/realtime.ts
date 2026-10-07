import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import { ClientFrameSchema, ServerFrameSchema } from './gen/arena360/v1/realtime_pb';

export type ClientRealtimeFrame =
  | { type: 'Subscribe'; channels: string[] }
  | { type: 'Unsubscribe'; channels: string[] }
  | { type: 'Ack'; msg_id: number }
  | { type: 'Publish'; channel: string; payload: Record<string, unknown> }
  | { type: 'Ping' };

export interface DecodedServerFrame {
  type: string;
  msg_id?: number;
  channel?: string;
  event_type?: string;
  payload?: Record<string, unknown>;
  ts?: string;
  user_id?: string;
  roles?: string[];
  channels?: string[];
  code?: string;
  message?: string;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function encodeClientFrame(frame: ClientRealtimeFrame): Uint8Array {
  switch (frame.type) {
    case 'Subscribe':
      return toBinary(
        ClientFrameSchema,
        create(ClientFrameSchema, {
          frame: { case: 'subscribe', value: { channels: frame.channels } },
        }),
      );
    case 'Unsubscribe':
      return toBinary(
        ClientFrameSchema,
        create(ClientFrameSchema, {
          frame: { case: 'unsubscribe', value: { channels: frame.channels } },
        }),
      );
    case 'Ack':
      return toBinary(
        ClientFrameSchema,
        create(ClientFrameSchema, {
          frame: { case: 'ack', value: { msgId: BigInt(frame.msg_id) } },
        }),
      );
    case 'Publish':
      return toBinary(
        ClientFrameSchema,
        create(ClientFrameSchema, {
          frame: {
            case: 'publish',
            value: {
              channel: frame.channel,
              payloadJson: encoder.encode(JSON.stringify(frame.payload)),
            },
          },
        }),
      );
    case 'Ping':
      return toBinary(
        ClientFrameSchema,
        create(ClientFrameSchema, { frame: { case: 'ping', value: {} } }),
      );
  }
}

export function decodeServerFrame(bytes: Uint8Array): DecodedServerFrame {
  const frame = fromBinary(ServerFrameSchema, bytes).frame;
  switch (frame.case) {
    case 'welcome':
      return { type: 'Welcome', user_id: frame.value.userId, roles: frame.value.roles };
    case 'subscribed':
      return { type: 'Subscribed', channels: frame.value.channels };
    case 'unsubscribed':
      return { type: 'Unsubscribed', channels: frame.value.channels };
    case 'event':
      return {
        type: 'Event',
        msg_id: Number(frame.value.msgId),
        channel: frame.value.channel,
        event_type: frame.value.eventType,
        payload: JSON.parse(decoder.decode(frame.value.payloadJson)) as Record<string, unknown>,
        ts: new Date(Number(frame.value.timestampMs)).toISOString(),
      };
    case 'error':
      return { type: 'Error', code: frame.value.code, message: frame.value.message };
    case 'pong':
      return { type: 'Pong' };
    default:
      throw new Error('Missing protobuf server frame');
  }
}

export function decodeClientFrame(bytes: Uint8Array): ClientRealtimeFrame {
  const frame = fromBinary(ClientFrameSchema, bytes).frame;
  switch (frame.case) {
    case 'subscribe':
      return { type: 'Subscribe', channels: frame.value.channels };
    case 'unsubscribe':
      return { type: 'Unsubscribe', channels: frame.value.channels };
    case 'ack':
      return { type: 'Ack', msg_id: Number(frame.value.msgId) };
    case 'publish':
      return {
        type: 'Publish',
        channel: frame.value.channel,
        payload: JSON.parse(decoder.decode(frame.value.payloadJson)) as Record<string, unknown>,
      };
    case 'ping':
      return { type: 'Ping' };
    default:
      throw new Error('Missing protobuf client frame');
  }
}

export function encodeServerFrame(frame: DecodedServerFrame): Uint8Array {
  switch (frame.type) {
    case 'Welcome':
      return toBinary(
        ServerFrameSchema,
        create(ServerFrameSchema, {
          frame: {
            case: 'welcome',
            value: { userId: frame.user_id ?? '', roles: frame.roles ?? [] },
          },
        }),
      );
    case 'Subscribed':
      return toBinary(
        ServerFrameSchema,
        create(ServerFrameSchema, {
          frame: { case: 'subscribed', value: { channels: frame.channels ?? [] } },
        }),
      );
    case 'Unsubscribed':
      return toBinary(
        ServerFrameSchema,
        create(ServerFrameSchema, {
          frame: { case: 'unsubscribed', value: { channels: frame.channels ?? [] } },
        }),
      );
    case 'Event':
      return toBinary(
        ServerFrameSchema,
        create(ServerFrameSchema, {
          frame: {
            case: 'event',
            value: {
              msgId: BigInt(frame.msg_id ?? 0),
              channel: frame.channel ?? '',
              eventType: frame.event_type ?? '',
              payloadJson: encoder.encode(JSON.stringify(frame.payload ?? {})),
              timestampMs: BigInt(frame.ts ? new Date(frame.ts).getTime() : Date.now()),
            },
          },
        }),
      );
    case 'Error':
      return toBinary(
        ServerFrameSchema,
        create(ServerFrameSchema, {
          frame: { case: 'error', value: { code: frame.code ?? '', message: frame.message ?? '' } },
        }),
      );
    case 'Pong':
      return toBinary(
        ServerFrameSchema,
        create(ServerFrameSchema, { frame: { case: 'pong', value: {} } }),
      );
    default:
      throw new Error(`Unsupported server frame: ${frame.type}`);
  }
}
