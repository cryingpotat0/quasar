#!/usr/bin/env node

import { QuasarClient, ConnectionOptions } from '@quasar/client';
import yargs from 'yargs';
import { hideBin } from 'yargs/helpers';
import repl from 'node:repl';

const argv = yargs(hideBin(process.argv))
  .option('url', {
    type: 'string',
    description: 'URL for the QuasarClient',
    demandOption: true
  })
  .option('code', {
    type: 'string',
    description: 'Connection code (if provided, will use "code" connection type)'
  })
  .option('debug', {
    type: 'boolean',
    description: 'Enable debug mode',
    default: false
  })
  .parseSync();

const connectionOptions: ConnectionOptions = argv.code
  ? { connectionType: 'code', code: argv.code }
  : { connectionType: 'new_channel' };

const client = new QuasarClient({
  url: argv.url,
  connectionOptions: connectionOptions,
  debug: argv.debug,
  onClose: () => console.log('Disconnected from QuasarClient'),
  onError: (error) => console.error('Error:', error),
  receiveData: (message: string) => console.log('Received:', message)
});

console.log('Connecting to QuasarClient...');

client.connect().then(() => {
  console.log('Connected. Enter commands or press Ctrl+C to exit.');
  
  const replServer = repl.start({
    prompt: 'QuasarClient> ',
    eval: (cmd, _context, _filename, _callback) => {
    if (cmd.includes('generateCode')) {
      console.log('Generating code...');
      client.generateCode().then((code) => {
        console.log('Generated code:', code);
      });
    } else
      if (cmd.trim()) {
        client.sendData(cmd.trim())
      } 
    }
  });

  replServer.on('exit', () => {
    console.log('Disconnecting from QuasarClient...');
    client.disconnect().then(() => {
      console.log('Disconnected. Goodbye!');
      process.exit(0);
    });
  });

  // Add the client object to the REPL context
  replServer.context['client'] = client;

}).catch((error: any) => {
  console.error('Failed to connect:', error);
  process.exit(1);
});
