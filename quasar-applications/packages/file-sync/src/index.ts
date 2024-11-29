#!/usr/bin/env node

import { QuasarClient, ConnectionOptions } from '@quasar/client'
import yargs from 'yargs'
import { hideBin } from 'yargs/helpers'
import winston from 'winston'
import { DirectoryWatcher, FileChange } from './directory-watcher'
import { FileSync } from './protocol'
import fs from 'fs/promises'
import path from 'path'
import { MessageBuffer } from './message-buffer'

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
    description: 'Type of user (uploader/downloader)',
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

  if (argv['user-type'] !== 'uploader' && argv['user-type'] !== 'downloader') {
    throw new Error('Invalid user-type')
  }
  const isDownloader = argv['user-type'] === 'downloader'
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

  if (isDownloader) {
    logger.debug('Waiting for messages...')
    // Process messages as they come in
    while (client.clientIds.size >= 2) {
      const message = await messageBuffer.next();
      try {
        const syncMessage: FileSync = JSON.parse(message);
        if (syncMessage.type === 'file_sync') {
          const change = syncMessage.change;
          const fullPath = path.join(directory, change.relativePath);
          
          switch (change.type) {
            case 'add':
            case 'change':
              await fs.mkdir(path.dirname(fullPath), { recursive: true });
              if (change.content) {
                await fs.writeFile(fullPath, Buffer.from(change.content, 'base64'));
                logger.info(`Wrote file: ${change.relativePath}`);
              }
              break;
            case 'unlink':
              await fs.unlink(fullPath);
              logger.info(`Deleted file: ${change.relativePath}`);
              break;
          }
        }
      } catch (error) {
        logger.error('Error processing file change:', error);
      }
    }
  } else {
    logger.debug('Watching directory for changes...')
    // Set up directory watcher for uploader
    const watcher = new DirectoryWatcher(
      directory,
      async (change: FileChange) => {
        try {
          if (change.type === 'add' || change.type === 'change') {
            // Read and encode file content
            const content = await fs.readFile(change.path)
            change.content = content.toString('base64')
          }
          
          const message: FileSync = {
            type: 'file_sync',
            change
          }
          
          client.sendData(JSON.stringify(message))
          logger.info(`Sent ${change.type} for: ${change.relativePath}`)
        } catch (error) {
          logger.error('Error sending file change:', error)
        }
      },
      logger
    )


    while (client.clientIds.size >= 2) {
        await new Promise((resolve) => setTimeout(resolve, 1000))
        logger.debug('Waiting for other client to disconnect...')
    }

  }
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
