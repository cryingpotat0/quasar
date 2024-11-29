import { Type, type Static } from '@sinclair/typebox'

export const PROTOCOL_VERSION = 1;

// Outgoing messages
export const GenerateCodeOutgoingMessage = Type.Object({
    type: Type.Literal('generate_code'),
});
export type GenerateCodeOutgoingMessage = Static<typeof GenerateCodeOutgoingMessage>;

export const DataOutgoingMessage = Type.Object({
    type: Type.Literal('data'),
    content: Type.String(),
});
export type DataOutgoingMessage = Static<typeof DataOutgoingMessage>;

export const OutgoingMessage = Type.Union([
    GenerateCodeOutgoingMessage,
    DataOutgoingMessage,
]);
export type OutgoingMessage = Static<typeof OutgoingMessage>;

// Incoming messages
export const GeneratedCodeIncomingMessage = Type.Object({
    type: Type.Literal('generated_code'),
    code: Type.String(),
});
export type GeneratedCodeIncomingMessage = Static<typeof GeneratedCodeIncomingMessage>;

export const DataIncomingMessage = Type.Object({
    type: Type.Literal('data'),
    content: Type.String(),
});
export type DataIncomingMessage = Static<typeof DataIncomingMessage>;

export const ConnectionInfoIncomingMessage = Type.Object({
    type: Type.Literal('connection_info'),
    id: Type.String(),
    channel_uuid: Type.String(),
    client_ids: Type.Array(Type.String()),
    protocol_version: Type.Number(),
});
export type ConnectionInfoIncomingMessage = Static<typeof ConnectionInfoIncomingMessage>;

export const ClientConnectedIncomingMessage = Type.Object({
    type: Type.Literal('client_connected'),
    id: Type.String(),
});
export type ClientConnectedIncomingMessage = Static<typeof ClientConnectedIncomingMessage>;

export const ClientDisconnectedIncomingMessage = Type.Object({
    type: Type.Literal('client_disconnected'),
    id: Type.String(),
});
export type ClientDisconnectedIncomingMessage = Static<typeof ClientDisconnectedIncomingMessage>;

export const IncomingMessage = Type.Union([
    GeneratedCodeIncomingMessage,
    DataIncomingMessage,
    ConnectionInfoIncomingMessage,
    ClientConnectedIncomingMessage,
    ClientDisconnectedIncomingMessage,
]);
export type IncomingMessage = Static<typeof IncomingMessage>;
