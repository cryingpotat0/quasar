import { Type, type Static } from '@sinclair/typebox'

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

export const IncomingMessage = Type.Union([
    GeneratedCodeIncomingMessage,
    DataIncomingMessage,
]);
export type IncomingMessage = Static<typeof IncomingMessage>;
