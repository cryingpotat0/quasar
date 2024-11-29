#!/usr/bin/env node

import { QuasarClient, ConnectionOptions } from '@quasar/client'
import yargs from 'yargs'
import { hideBin } from 'yargs/helpers'
import winston from 'winston'

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

async function main() {
  const client = new QuasarClient({
    url: argv.url,
    connectionOptions: connectionOptions,
    debug: argv.debug,
    onClose: () => console.log('Disconnected from QuasarClient'),
    onError: (error) => console.error('Error:', error),
    logger,
    receiveData: (message: string) => console.log('Received:', message)
  })

  if (argv['user-type'] !== 'leader' && argv['user-type'] !== 'follower') {
    throw new Error('Invalid user-type')
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

  await new Promise((resolve) => setTimeout(resolve, 1000))
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
