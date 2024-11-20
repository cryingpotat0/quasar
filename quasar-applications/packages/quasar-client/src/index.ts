import WebSocket from 'ws';
import winston from 'winston';

export interface QuasarClientOptions {
  host: string;
  port: number;
  debug?: boolean;
  onOpen?: () => void;
  onClose?: () => void;
  onError?: (error: Error) => void;
  onControlMessage?: (message: string) => void;
  onDataMessage?: (message: string) => void;
}

export class QuasarClient {
  private ws: WebSocket;
  private logger: winston.Logger;

  constructor(private options: QuasarClientOptions) {
    this.logger = winston.createLogger({
      level: options.debug ? 'debug' : 'info',
      format: winston.format.simple(),
      transports: [new winston.transports.Console()],
    });

    const url = `ws://${options.host}:${options.port}/ws`;
    this.logger.debug(`Attempting to connect to WebSocket at ${url}`);
    this.ws = new WebSocket(url);

    this.ws.on('open', this.handleOpen.bind(this));
    this.ws.on('close', this.handleClose.bind(this));
    this.ws.on('error', this.handleError.bind(this));
    this.ws.on('message', this.handleMessage.bind(this));
  }

  private handleOpen(): void {
    this.logger.info('Connected to Quasar server');
    this.options.onOpen?.();
  }

  private handleClose(): void {
    this.logger.info('Disconnected from Quasar server');
    this.options.onClose?.();
  }

  private handleError(error: Error): void {
    this.logger.error('WebSocket error:', error);
    this.options.onError?.(error);
  }

  private handleMessage(data: WebSocket.Data): void {
    const message = data.toString();
    this.logger.debug(`Received message: ${message}`);
    if (message.startsWith('CONTROL:')) {
      this.options.onControlMessage?.(message.slice(8));
    } else {
      this.options.onDataMessage?.(message);
    }
  }

  public sendControlMessage(message: string): void {
    this.logger.debug(`Sending control message: ${message}`);
    this.ws.send(`CONTROL:${message}`);
  }

  public sendDataMessage(message: string): void {
    this.logger.debug(`Sending data message: ${message}`);
    this.ws.send(message);
  }

  public close(): void {
    this.logger.info('Closing connection');
    this.ws.close();
  }
}
