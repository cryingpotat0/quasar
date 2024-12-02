#!/usr/bin/env node

import { QuasarClient, ConnectionOptions } from '@quasartc/client'
import yargs from 'yargs'
import { hideBin } from 'yargs/helpers'
import winston from 'winston'
import { DirectoryWatcher, FileChange } from './directory-watcher'
import { FileSync } from './protocol'
import fs from 'fs/promises'
import path from 'path'
import { MessageBuffer } from './message-buffer'
import md5 from 'md5'

// Add new types for the enhanced sync protocol
interface FileSyncMessage {
  type: 'file_sync';
  change: {
    type: 'replace';
    relativePath: string;
    oldHash: string;
    newHash: string;
    content: string; // base64 encoded
  };
}

// Add this near the top with other interfaces
interface HashCache {
  [path: string]: string;
}

const argv = yargs(hideBin(process.argv))
  .option('url', {
    type: 'string',
    description: 'URL for the QuasarClient',
    demandOption: true
  })
  .option('code', {
    type: 'string',
    description:
      'Connection code (if provided, will use "code" connection type)'
  })
  .option('debug', {
    type: 'boolean',
    description: 'Enable debug mode',
    default: false
  })
  .option('user-type', {
    type: 'string',
    description: 'Type of user (leader/follower)',
    demandOption: true
  })
  .option('directory', {
        type: 'string',
        description: 'Directory to sync',
  })
  .parseSync()

const connectionOptions: ConnectionOptions = argv.code
  ? { connectionType: 'code', code: argv.code }
  : { connectionType: 'new_channel' }

const logger = winston.createLogger({
  level: argv.debug ? 'debug' : 'info',
  format: winston.format.simple(),
  transports: [new winston.transports.Console()]
})

const messageBuffer = new MessageBuffer();

// Add this before main()
const fileHashCache: HashCache = {};

async function calculateFileHash(filePath: string): Promise<string> {
  try {
    const content = await fs.readFile(filePath)
    return md5(content)
  } catch (error) {
    // Return empty hash if file doesn't exist
    return md5('')
  }
}

async function main() {
  const client = new QuasarClient({
    url: argv.url,
    connectionOptions: connectionOptions,
    debug: argv.debug,
    onClose: () => console.log('Disconnected from QuasarClient'),
    onError: (error) => console.error('Error:', error),
    logger,
    receiveData: (message: string) => messageBuffer.push(message)
  })

  if (argv['user-type'] !== 'leader' && argv['user-type'] !== 'follower') {
    throw new Error('Invalid user-type - must be leader or follower')
  }
  const isLeader = argv['user-type'] === 'leader'
  const directory = argv.directory || process.cwd()

  logger.debug('Connecting to QuasarClient...')

  await client.connect()

  // We can now build our own file-syncing protocol on top of the datastream.
  // Assume there are only two clients.
  // First, we have to generate a code for the other client if one wasn't provided.
  if (client.clientIds.size === 1) {
    logger.debug('Generating code...')
    const code = await client.generateCode()
    logger.info(code)
  }

  // Wait for the other user to connect.
  // Do this by polling for now, eventually we probably want a way to
  // handle a client connection event.
  while (client.clientIds.size < 2) {
    await new Promise((resolve) => setTimeout(resolve, 1000))
    logger.debug('Waiting for other client to connect...')
  }

  logger.info('Connected!')

  // Set up directory watcher for both leader and follower
  const watcher = new DirectoryWatcher(
    directory,
    async (change: FileChange) => {
      try {

        let content: Buffer | undefined = undefined
        let newHash: string | undefined = undefined

        if (change.type === 'unlink') {
          // For deleted files, use empty string hash and remove from cache
          newHash = md5('')
          content = Buffer.from('')
          delete fileHashCache[change.relativePath]
        } else {
          // For new/modified files, read content and calculate hash
          const fileBuffer = await fs.readFile(change.path)
          content = fileBuffer
          newHash = md5(fileBuffer)
        }

        // Skip if the file hasn't actually changed from our last known state
        if (fileHashCache[change.relativePath] === newHash) {
          logger.debug(`Skipping unchanged file: ${change.relativePath}`)
          return
        }

        const contentBase64 = content.toString('base64')
        const oldHash = fileHashCache[change.relativePath] || md5('')

        const message: FileSyncMessage = {
          type: 'file_sync',
          change: {
            type: 'replace',
            relativePath: change.relativePath,
            oldHash,
            newHash,
            content: contentBase64
          }
        }

        logger.debug(`Sending file change for: ${change.relativePath} from ${oldHash} to ${newHash}`)

        // Update cache before sending (deleted files were already removed)
        if (change.type !== 'unlink') {
          fileHashCache[change.relativePath] = newHash
        }

        client.sendData(JSON.stringify(message))
        logger.info(`Sent file change for: ${change.relativePath}`)
      } catch (error) {
        logger.error('Error sending file change:', error)
      }
    },
    logger
  )

  // Process incoming messages for both leader and follower
  while (client.clientIds.size >= 2) {
    const message = await messageBuffer.next();
    try {
      const syncMessage: FileSyncMessage = JSON.parse(message);
      if (syncMessage.type === 'file_sync') {
        const change = syncMessage.change;
        const fullPath = path.join(directory, change.relativePath);

        if (!isLeader) {
          // Follower always accepts remote changes
          await applyFileChange(fullPath, change.content);
          fileHashCache[change.relativePath] = change.newHash;
          logger.info(`Updated file: ${change.relativePath}`);
        } else {
          // Leader only accepts if hashes match, otherwise sends their version
          const currentHash = await calculateFileHash(fullPath);
          if (currentHash === change.oldHash) {
            await applyFileChange(fullPath, change.content);
            fileHashCache[change.relativePath] = change.newHash;
            logger.info(`Updated file: ${change.relativePath}`);
          } else {
            // Leader sends their current version
            const leaderContent = await fs.readFile(fullPath);
            const leaderHash = md5(leaderContent);
            const message: FileSyncMessage = {
              type: 'file_sync',
              change: {
                type: 'replace',
                relativePath: change.relativePath,
                oldHash: currentHash,
                newHash: leaderHash,
                content: leaderContent.toString('base64')
              }
            };
            fileHashCache[change.relativePath] = leaderHash;
            client.sendData(JSON.stringify(message));
            logger.info(`Leader sent override for: ${change.relativePath}`);
          }
        }
      }
    } catch (error) {
      logger.error('Error processing file change:', error);
    }
  }
}

async function applyFileChange(fullPath: string, contentBase64: string) {
  await fs.mkdir(path.dirname(fullPath), { recursive: true });
  await fs.writeFile(fullPath, Buffer.from(contentBase64, 'base64'));
}

;(async () => {
  try {
    await main()
  } catch (error) {
    console.error(error)
  } finally {
    process.exit(0)
  }
})()
