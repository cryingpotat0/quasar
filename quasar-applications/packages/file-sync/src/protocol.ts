import { Type, Static } from '@sinclair/typebox'

export const FileChange = Type.Object({
  type: Type.Union([
    Type.Literal('add'),
    Type.Literal('change'),
    Type.Literal('unlink')
  ]),
  path: Type.String(),
  relativePath: Type.String(),
  content: Type.Optional(Type.String()) // Base64 encoded for binary files
})

export type FileChange = Static<typeof FileChange>

export const FileSync = Type.Object({
  type: Type.Literal('file_sync'),
  change: Type.Ref(FileChange)
})

export type FileSync = Static<typeof FileSync> 