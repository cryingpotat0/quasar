#!/usr/bin/env node

import { Command } from 'commander';
import { render } from 'ink';
import React from 'react';
import { ChatTUI } from './tui.js';
import { QuasarClientOptions } from '@quasar/client';

export function startChatCLI(): void {
  const program = new Command();

  program
    .name('quasar-chat')
    .description('Quasar Chat Client')
    .version('0.1.0')
    .option('-h, --host <host>', 'Quasar server host', '127.0.0.1')
    .option('-p, --port <port>', 'Quasar server port', '3030')
    .option('-d, --debug', 'Enable debug mode')
    .option('--no-tui', 'Disable TUI mode')
    .action((options) => {
      const clientOptions: QuasarClientOptions = {
        host: options.host,
        port: parseInt(options.port, 10),
        debug: options.debug,
      };

      if (options.tui) {
        render(React.createElement(ChatTUI, clientOptions));
      } else {
        console.log('Non-TUI mode not implemented yet');
        process.exit(1);
      }
    });

  program.parse(process.argv);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  startChatCLI();
}
